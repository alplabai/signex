//! DigiKey distributor adapter — OAuth2 PKCE scaffold.
//!
//! - Uses the `oauth2` crate (v5) for the authorization-code + PKCE flow.
//! - Refresh token persisted in OS keyring under
//!   `signex-distributor-digikey` (username slot `"refresh"`).
//! - No real auth in tests; the refresh-token → access-token exchange is
//!   mocked with `wiremock`. Live API tests are `#[ignore]`d.
//!
//! Public surface:
//! - [`DigiKeyAuth`] — orchestrates the OAuth2 flow. The interactive
//!   "open browser, redirect, exchange code" handshake belongs in the UI;
//!   the library just exposes:
//!   * [`DigiKeyAuth::start_authorization()`] — produces the auth URL
//!     plus the PKCE verifier the UI needs to keep until callback.
//!   * [`DigiKeyAuth::exchange_code()`] — UI passes the redirected `code`
//!     here, we persist the refresh token in keyring and return the
//!     access token.
//!   * [`DigiKeyAuth::access_token()`] — refresh-token-grant, called by
//!     the adapter on every request.
//! - [`DigiKeyAdapter`] — implements `DistributorAdapter` using the
//!   access token in `Authorization: Bearer …`.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::Utc;
use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
};
use serde::Deserialize;
use url::Url;

use crate::distributor::{
    DistributorAdapter, DistributorError, DistributorPart, DistributorSource, PriceBreak,
};
use crate::distributors::cache::{DEFAULT_TTL, DistributorCache};
use crate::distributors::keyring::{KeyringError, KeyringStore};
use crate::param::ParamMap;

/// DigiKey production endpoints. Tests override these via `with_endpoints`.
pub const DIGIKEY_AUTH_URL: &str = "https://api.digikey.com/v1/oauth2/authorize";
pub const DIGIKEY_TOKEN_URL: &str = "https://api.digikey.com/v1/oauth2/token";
pub const DIGIKEY_API_BASE: &str = "https://api.digikey.com";
const DIGIKEY_PROVIDER_KEY: &str = "digikey";
const DIGIKEY_NAME: &str = "DigiKey";
const DEFAULT_CURRENCY: &str = "USD";
const THROTTLE_INTERVAL: Duration = Duration::from_secs(1);

type ConfiguredClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

// ---------------------------------------------------------------------------
// OAuth2 orchestration.
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum DigiKeyAuthError {
    #[error("oauth2 config: {0}")]
    Config(String),
    #[error("oauth2 request: {0}")]
    Request(String),
    #[error("keyring: {0}")]
    Keyring(String),
    #[error("no refresh token stored")]
    NoRefreshToken,
    #[error("authorization cancelled before the token was stored")]
    Cancelled,
}

impl From<DigiKeyAuthError> for DistributorError {
    fn from(e: DigiKeyAuthError) -> Self {
        match e {
            DigiKeyAuthError::NoRefreshToken => DistributorError::Auth(
                "no DigiKey refresh token in keyring; run authorization flow first".into(),
            ),
            DigiKeyAuthError::Config(m)
            | DigiKeyAuthError::Request(m)
            | DigiKeyAuthError::Keyring(m) => DistributorError::Auth(m),
            DigiKeyAuthError::Cancelled => DistributorError::Auth(e.to_string()),
        }
    }
}

/// Scaffolds the OAuth2 PKCE authorization-code flow.
pub struct DigiKeyAuth {
    client_id: String,
    client: ConfiguredClient,
    keyring: KeyringStore,
    /// Test-only fallback: when set, [`DigiKeyAuth::access_token`] uses this
    /// refresh token instead of reading from the OS keyring. Lets wiremock
    /// integration tests cover the refresh-grant path without depending on
    /// the platform keyring. Production callers leave it `None`.
    fallback_refresh: Option<String>,
    /// Key of this client's entry in [`SESSIONS`]: client id and token URL.
    session_key: String,
}

/// What a DigiKey client remembers between walks, for the life of the
/// process: a `DigiKeyAuth` is rebuilt for every Refresh All Pricing.
#[derive(Default)]
struct Session {
    /// The access token from the last exchange and when to stop using it, so
    /// a walk over many rows exchanges once, not once per request.
    access: Option<(String, Instant)>,
    /// The newest refresh token DigiKey issued. The exchange consumed the
    /// old one, so when persisting the new one to the keyring fails, this is
    /// the only valid token left: later walks use it until the app restarts.
    refresh: Option<String>,
}

static SESSIONS: std::sync::OnceLock<Mutex<std::collections::HashMap<String, Session>>> =
    std::sync::OnceLock::new();

fn sessions() -> std::sync::MutexGuard<'static, std::collections::HashMap<String, Session>> {
    SESSIONS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// How long before its stated expiry a cached access token is replaced.
const ACCESS_TOKEN_MARGIN: Duration = Duration::from_secs(30);
/// Lifetime assumed when the token response states none.
const ACCESS_TOKEN_DEFAULT_LIFETIME: Duration = Duration::from_secs(120);

impl DigiKeyAuth {
    /// Production constructor: real DigiKey endpoints, refresh token in
    /// `signex-distributor-digikey/refresh`.
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        redirect_uri: impl Into<String>,
    ) -> Result<Self, DigiKeyAuthError> {
        Self::with_endpoints(
            client_id,
            client_secret,
            redirect_uri,
            DIGIKEY_AUTH_URL,
            DIGIKEY_TOKEN_URL,
        )
    }

    /// Test constructor: override auth + token URLs (e.g. wiremock).
    pub fn with_endpoints(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        redirect_uri: impl Into<String>,
        auth_url: &str,
        token_url: &str,
    ) -> Result<Self, DigiKeyAuthError> {
        let auth_url =
            AuthUrl::new(auth_url.into()).map_err(|e| DigiKeyAuthError::Config(e.to_string()))?;
        let token_url =
            TokenUrl::new(token_url.into()).map_err(|e| DigiKeyAuthError::Config(e.to_string()))?;
        let redirect_url = RedirectUrl::new(redirect_uri.into())
            .map_err(|e| DigiKeyAuthError::Config(e.to_string()))?;

        let client_id: String = client_id.into();
        let client_id_for_key = client_id.clone();
        let token_url_for_key = token_url.to_string();
        let client = BasicClient::new(ClientId::new(client_id.clone()))
            .set_client_secret(ClientSecret::new(client_secret.into()))
            .set_auth_uri(auth_url)
            .set_token_uri(token_url)
            .set_redirect_uri(redirect_url);

        // MD-17: surface keyring-unavailable errors at construction so
        // callers can show a clear "OS keychain not available — install
        // libsecret on Linux or run with --no-keyring" rather than
        // letting the iced UI panic at a later moment.
        let keyring = KeyringStore::for_provider("digikey", "refresh")
            .map_err(|e| DigiKeyAuthError::Keyring(e.to_string()))?;
        Ok(Self {
            client_id,
            client,
            keyring,
            fallback_refresh: None,
            session_key: format!("{client_id_for_key}@{token_url_for_key}"),
        })
    }

    /// The OAuth client id; DigiKey's product API also wants it as the
    /// `X-DIGIKEY-Client-Id` header on every request.
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Test-only setter: provide an in-memory refresh token. When set,
    /// [`Self::access_token`] uses this instead of the keyring.
    #[doc(hidden)]
    pub fn with_test_refresh_token(mut self, refresh: impl Into<String>) -> Self {
        self.fallback_refresh = Some(refresh.into());
        self
    }

    /// Step 1 of the flow: produce the authorization URL the UI should open
    /// in a browser. Returned tuple: `(url, csrf_token, pkce_verifier)`.
    /// The UI keeps the verifier until the redirect callback fires.
    pub fn start_authorization(&self) -> (Url, CsrfToken, PkceCodeVerifier) {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let (auth_url, csrf_token) = self
            .client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new("readonly".to_string()))
            .set_pkce_challenge(pkce_challenge)
            .url();
        (auth_url, csrf_token, pkce_verifier)
    }

    /// Step 2: exchange the redirected `code` for tokens; persist the
    /// refresh token in keyring and return the access token.
    ///
    /// H3: this call now requires the CSRF state token returned by the
    /// authorization redirect (`returned_state`) to match the one issued by
    /// [`Self::start_authorization`] (`expected_state`). Without this check,
    /// an attacker who can forge a redirect (e.g. via DNS rebinding or a
    /// malicious deep-link) could complete the OAuth exchange with an
    /// attacker-controlled `code` and capture the resulting refresh token
    /// into the victim's keyring.
    ///
    /// We compare with `str::eq`. Constant-time comparison would be cleaner
    /// in principle, but DigiKey's CSRF tokens are 128+ bits of entropy and
    /// the exchange is a one-shot per browser session — the timing
    /// side-channel surface is bounded to a single guess per OAuth flow,
    /// well below the threshold where ConstantTime matters. Document this
    /// inline so future audits don't flag it.
    pub fn exchange_code(
        &self,
        code: &str,
        verifier: PkceCodeVerifier,
        returned_state: &str,
        expected_state: &CsrfToken,
    ) -> Result<String, DigiKeyAuthError> {
        self.exchange_code_guarded(code, verifier, returned_state, expected_state, &|| true)
    }

    /// [`Self::exchange_code`] with a last-moment veto over persisting.
    ///
    /// `may_persist` is asked once the token endpoint has answered and
    /// immediately before the refresh token would be written to the
    /// keyring. Returning `false` yields [`DigiKeyAuthError::Cancelled`]
    /// and writes nothing — the exchange is a network round trip, so a
    /// user can cancel while it is in flight.
    pub fn exchange_code_guarded(
        &self,
        code: &str,
        verifier: PkceCodeVerifier,
        returned_state: &str,
        expected_state: &CsrfToken,
        may_persist: &dyn Fn() -> bool,
    ) -> Result<String, DigiKeyAuthError> {
        self.exchange_code_inner(
            code,
            verifier,
            returned_state,
            expected_state,
            may_persist,
            &|refresh| {
                self.keyring
                    .set_secret(refresh)
                    .map_err(|e| DigiKeyAuthError::Keyring(e.to_string()))
            },
        )
    }

    /// [`Self::exchange_code_guarded`] with the refresh-token sink injected,
    /// so tests can observe persistence without the OS keyring. Compiled only
    /// with the `test-support` feature: a caller with this entry point could
    /// bypass the keyring, so it is not part of the shipped API.
    #[cfg(feature = "test-support")]
    pub fn exchange_code_into(
        &self,
        code: &str,
        verifier: PkceCodeVerifier,
        returned_state: &str,
        expected_state: &CsrfToken,
        may_persist: &dyn Fn() -> bool,
        store_refresh: &dyn Fn(&str) -> Result<(), DigiKeyAuthError>,
    ) -> Result<String, DigiKeyAuthError> {
        self.exchange_code_inner(
            code,
            verifier,
            returned_state,
            expected_state,
            may_persist,
            store_refresh,
        )
    }

    /// Delete the stored refresh token (idempotent). Used to undo a token a
    /// cancelled OAuth flow managed to store before the cancel landed.
    pub fn remove_stored_refresh_token() -> Result<(), DigiKeyAuthError> {
        KeyringStore::for_provider("digikey", "refresh")
            .and_then(|k| k.delete())
            .map_err(|e| DigiKeyAuthError::Keyring(e.to_string()))
    }

    fn exchange_code_inner(
        &self,
        code: &str,
        verifier: PkceCodeVerifier,
        returned_state: &str,
        expected_state: &CsrfToken,
        may_persist: &dyn Fn() -> bool,
        store_refresh: &dyn Fn(&str) -> Result<(), DigiKeyAuthError>,
    ) -> Result<String, DigiKeyAuthError> {
        if returned_state != expected_state.secret() {
            return Err(DigiKeyAuthError::Request(
                "CSRF state mismatch — refusing to exchange OAuth code".into(),
            ));
        }
        let http = build_http_client();
        let token = self
            .client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_pkce_verifier(verifier)
            .request(&http)
            .map_err(|e| DigiKeyAuthError::Request(e.to_string()))?;
        if !may_persist() {
            return Err(DigiKeyAuthError::Cancelled);
        }
        if let Some(refresh) = token.refresh_token() {
            store_refresh(refresh.secret())?;
        }
        Ok(token.access_token().secret().clone())
    }

    /// Step 3 (every API call): use the keyring-stored refresh token to
    /// mint a fresh access token. Honours [`Self::with_test_refresh_token`]
    /// when set (tests).
    pub fn access_token(&self) -> Result<String, DigiKeyAuthError> {
        // One exchange per token lifetime. The registry lock is held across
        // the exchange so two callers cannot both spend the same refresh
        // token.
        let mut all = sessions();
        let session = all.entry(self.session_key.clone()).or_default();
        if let Some((token, usable_until)) = session.access.as_ref()
            && Instant::now() < *usable_until
        {
            return Ok(token.clone());
        }
        let refresh = if let Some(t) = session.refresh.clone() {
            t
        } else if let Some(t) = &self.fallback_refresh {
            t.clone()
        } else {
            match self.keyring.get_secret() {
                Ok(s) => s,
                Err(KeyringError::NotFound) => return Err(DigiKeyAuthError::NoRefreshToken),
                Err(KeyringError::Backend(m)) => return Err(DigiKeyAuthError::Keyring(m)),
            }
        };
        let http = build_http_client();
        let token = self
            .client
            .exchange_refresh_token(&RefreshToken::new(refresh))
            .request(&http)
            .map_err(|e| DigiKeyAuthError::Request(e.to_string()))?;
        // Rotate: DigiKey consumed the old refresh token, so remember the new
        // one for the process first, then persist it once per exchange.
        //
        // L5: a failed keyring write leaves the keyring holding a consumed
        // token; the in-memory copy keeps every later walk working until the
        // app restarts, and the warning says why a restart would need a new
        // authorisation.
        if let Some(new_refresh) = token.refresh_token() {
            session.refresh = Some(new_refresh.secret().clone());
            if self.fallback_refresh.is_none()
                && let Err(e) = self.keyring.set_secret(new_refresh.secret())
            {
                tracing::warn!(
                    error = %e,
                    "failed to persist rotated DigiKey refresh token to keyring; \
                     pricing keeps working until the app restarts, then \
                     DigiKey will need to be authorised again",
                );
            }
        }
        let access = token.access_token().secret().clone();
        let lifetime = token
            .expires_in()
            .unwrap_or(ACCESS_TOKEN_DEFAULT_LIFETIME)
            .saturating_sub(ACCESS_TOKEN_MARGIN);
        session.access = Some((access.clone(), Instant::now() + lifetime));
        Ok(access)
    }
}

fn build_http_client() -> reqwest::blocking::Client {
    super::http::finish(super::http::client_builder().redirect(reqwest::redirect::Policy::none()))
}

// ---------------------------------------------------------------------------
// Adapter.
// ---------------------------------------------------------------------------

pub struct DigiKeyAdapter {
    api_base: String,
    cache: Option<DistributorCache>,
    throttle: Mutex<Option<Instant>>,
    http: reqwest::blocking::Client,
    auth: AuthSource,
    /// ISO 4217 code sent as `X-DIGIKEY-Locale-Currency`; prices come back
    /// in it.
    currency: String,
    /// Sent as `X-DIGIKEY-Client-Id`; DigiKey rejects product calls without
    /// it, so prices are only claimed once it is known.
    client_id: Option<String>,
}

enum AuthSource {
    /// Tests inject a precomputed access token directly.
    InlineAccessToken(String),
    /// Production: refresh-token-grant on every call. Boxed because
    /// `DigiKeyAuth` is much larger than `String` and clippy flags the
    /// size disparity otherwise.
    Oauth(Box<DigiKeyAuth>),
}

impl DigiKeyAdapter {
    /// Production constructor.
    pub fn new(auth: DigiKeyAuth, cache: Option<DistributorCache>) -> Self {
        let client_id = Some(auth.client_id().to_string()).filter(|c| !c.is_empty());
        Self {
            api_base: DIGIKEY_API_BASE.into(),
            cache,
            throttle: Mutex::new(None),
            http: build_http_client(),
            auth: AuthSource::Oauth(Box::new(auth)),
            currency: DEFAULT_CURRENCY.into(),
            client_id,
        }
    }

    /// Test constructor: override the API base + provide a fixed access
    /// token. Skips the OAuth refresh flow entirely so wiremock can focus
    /// on the product-search request.
    pub fn with_access_token(
        api_base: impl Into<String>,
        access_token: impl Into<String>,
        cache: Option<DistributorCache>,
    ) -> Self {
        Self {
            api_base: api_base.into(),
            cache,
            throttle: Mutex::new(None),
            http: build_http_client(),
            auth: AuthSource::InlineAccessToken(access_token.into()),
            currency: DEFAULT_CURRENCY.into(),
            client_id: None,
        }
    }

    /// Test+production constructor: provide a `DigiKeyAuth` that points at
    /// a wiremock token endpoint. Combined with [`Self::with_access_token`]
    /// this lets us cover the full OAuth flow in tests.
    pub fn with_oauth_and_base(
        api_base: impl Into<String>,
        auth: DigiKeyAuth,
        cache: Option<DistributorCache>,
    ) -> Self {
        let client_id = Some(auth.client_id().to_string()).filter(|c| !c.is_empty());
        Self {
            api_base: api_base.into(),
            cache,
            throttle: Mutex::new(None),
            http: build_http_client(),
            auth: AuthSource::Oauth(Box::new(auth)),
            currency: DEFAULT_CURRENCY.into(),
            client_id,
        }
    }

    /// Send `client_id` as `X-DIGIKEY-Client-Id` (a client built from a
    /// `DigiKeyAuth` already does). Without one the client does not claim
    /// to return prices.
    pub fn with_client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_id = Some(client_id.into()).filter(|c| !c.is_empty());
        self
    }

    /// Ask DigiKey for prices in `currency` (ISO 4217, e.g. `"EUR"`).
    pub fn with_currency(mut self, currency: impl Into<String>) -> Self {
        self.currency = currency.into().to_ascii_uppercase();
        self
    }

    fn polite_wait(&self) {
        super::throttle::polite_wait(&self.throttle, THROTTLE_INTERVAL);
    }

    fn resolve_access_token(&self) -> Result<String, DistributorError> {
        match &self.auth {
            AuthSource::InlineAccessToken(t) => Ok(t.clone()),
            AuthSource::Oauth(a) => a.access_token().map_err(DistributorError::from),
        }
    }

    fn search_by_keyword(&self, mpn: &str) -> Result<Vec<DistributorPart>, DistributorError> {
        let url = format!("{}/products/v4/search/keyword", self.api_base);
        let body = serde_json::json!({
            "Keywords": mpn,
            "Limit": 25,
            "Offset": 0,
        });
        let resp = self.execute(self.http.post(&url).json(&body))?;
        let raw: DigiKeyResponse = resp
            .json()
            .map_err(|e| DistributorError::Backend(format!("decode: {e}")))?;
        Ok(raw.into_parts(&self.currency, None))
    }

    /// Product details by DigiKey product number (`…/{sku}/productdetails`).
    fn details_by_sku(&self, sku: &str) -> Result<Vec<DistributorPart>, DistributorError> {
        let url = format!(
            "{}/products/v4/search/{}/productdetails",
            self.api_base,
            super::price::encode_path_segment(sku)
        );
        let resp = self.execute(self.http.get(&url))?;
        let raw: DigiKeyDetailsResponse = resp
            .json()
            .map_err(|e| DistributorError::Backend(format!("decode: {e}")))?;
        let single = DigiKeyResponse {
            products: raw.product.into_iter().collect(),
        };
        Ok(single.into_parts(&self.currency, Some(sku)))
    }

    /// Send `request` with the bearer token and locale headers, honouring the
    /// throttle, and map the HTTP status onto [`DistributorError`].
    fn execute(
        &self,
        request: reqwest::blocking::RequestBuilder,
    ) -> Result<reqwest::blocking::Response, DistributorError> {
        let token = self.resolve_access_token()?;
        self.polite_wait();
        let mut request = request;
        if let Some(id) = &self.client_id {
            request = request.header("X-DIGIKEY-Client-Id", id);
        }
        let resp = request
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/json")
            .header("X-DIGIKEY-Locale-Currency", &self.currency)
            .send()
            .map_err(|e| DistributorError::Network(e.to_string()))?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || resp.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(DistributorError::Auth(format!(
                "DigiKey rejected token (HTTP {})",
                resp.status()
            )));
        }
        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after_seconds = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);
            return Err(DistributorError::RateLimited {
                retry_after_seconds,
            });
        }
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(DistributorError::NotFound);
        }
        if !resp.status().is_success() {
            return Err(DistributorError::Backend(format!("HTTP {}", resp.status())));
        }
        Ok(resp)
    }
}

impl DistributorAdapter for DigiKeyAdapter {
    fn name(&self) -> &'static str {
        DIGIKEY_NAME
    }

    fn source(&self) -> DistributorSource {
        DistributorSource::DigiKey
    }

    fn lookup_by_url(&self, url: &Url) -> Result<Option<DistributorPart>, DistributorError> {
        let host = url.host_str().unwrap_or("");
        if !super::host_in_domain(host, "digikey.com") {
            return Ok(None);
        }
        let from_query = url
            .query_pairs()
            .find(|(k, _)| {
                let lk = k.to_ascii_lowercase();
                lk == "keywords" || lk == "k" || lk == "search"
            })
            .map(|(_, v)| v.to_string());
        let from_path = digikey_path_token(url.path());
        let token = from_query.or(from_path).unwrap_or_default();
        if token.is_empty() {
            return Ok(None);
        }
        let hits = self.search_by_keyword(&token)?;
        Ok(super::price::pick_url_hit(hits, &token, &["digikey_pn"]))
    }

    fn lookup_by_mpn(&self, mpn: &str) -> Result<Vec<DistributorPart>, DistributorError> {
        if let Some(cache) = &self.cache
            && let Some(hit) = cache.get_or_warn(DIGIKEY_PROVIDER_KEY, mpn, DEFAULT_TTL)
        {
            return Ok(vec![hit]);
        }
        let hits = self.search_by_keyword(mpn)?;
        if let (Some(cache), Some(first)) = (&self.cache, hits.first()) {
            // H4: silenced cache writes hide disk-full / permission failures.
            if let Err(e) = cache.put(DIGIKEY_PROVIDER_KEY, first) {
                tracing::warn!(
                    error = %e,
                    mpn = %mpn,
                    "DigiKey cache write failed; live result returned but not persisted",
                );
            }
        }
        Ok(hits)
    }

    fn refresh_pricing(
        &self,
        part: &DistributorPart,
    ) -> Result<crate::distributor::PricingSnapshot, DistributorError> {
        let sku = part
            .extra
            .get("sku")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        // Only the row's own part is priced: the productdetails of its SKU
        // (whose variation must carry that number), or, with no SKU, the one
        // keyword hit with the row's exact MPN and maker. A failed or empty
        // SKU lookup is reported, never answered with another part's price.
        let fresh = match sku {
            Some(sku) => self
                .details_by_sku(sku)?
                .into_iter()
                .next()
                .ok_or(DistributorError::NotFound)?,
            None => {
                let hits = self.search_by_keyword(&part.mpn)?;
                super::price::pick_exact_mpn_hit(&hits, part)
                    .cloned()
                    .ok_or(DistributorError::NotFound)?
            }
        };
        super::price::pricing_or_error(fresh)
    }

    fn supports_pricing(&self) -> bool {
        // DigiKey answers product calls only with the client id header.
        self.client_id.is_some()
    }
}

// ---------------------------------------------------------------------------
// Response shape (subset of DigiKey Products v4 keyword search).
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct DigiKeyResponse {
    #[serde(default, rename = "Products")]
    products: Vec<DigiKeyProductDto>,
}

#[derive(Debug, Deserialize)]
struct DigiKeyProductDto {
    #[serde(default, rename = "ManufacturerProductNumber")]
    mpn: String,
    #[serde(default, rename = "Manufacturer")]
    manufacturer: DigiKeyManufacturer,
    #[serde(default, rename = "Description")]
    description: DigiKeyDescription,
    #[serde(default, rename = "DatasheetUrl")]
    datasheet_url: Option<String>,
    #[serde(default, rename = "QuantityAvailable")]
    quantity_available: Option<u32>,
    #[serde(default, rename = "ProductUrl")]
    product_url: Option<String>,
    #[serde(default, rename = "ProductVariations")]
    variations: Vec<DigiKeyVariation>,
}

/// `{ "Product": { … } }` — the shape of `…/{sku}/productdetails`.
#[derive(Debug, Deserialize)]
struct DigiKeyDetailsResponse {
    #[serde(default, rename = "Product")]
    product: Option<DigiKeyProductDto>,
}

/// One packaging of a product (cut tape, reel, …), with its own number.
#[derive(Debug, Deserialize)]
struct DigiKeyVariation {
    #[serde(default, rename = "DigiKeyProductNumber")]
    product_number: String,
    #[serde(default, rename = "StandardPricing")]
    standard_pricing: Vec<DigiKeyPriceBreak>,
}

#[derive(Debug, Deserialize)]
struct DigiKeyPriceBreak {
    #[serde(default, rename = "BreakQuantity")]
    break_quantity: u32,
    /// `None` when the field is absent: that must not read as a free part.
    #[serde(default, rename = "UnitPrice")]
    unit_price: Option<f64>,
}

/// Choose the packaging to price: the one numbered `sku` when given (never a
/// different packaging, whose price differs), else the first that lists
/// prices. `Err` carries why none could be read.
fn pick_variation<'a>(
    variations: &'a [DigiKeyVariation],
    sku: Option<&str>,
) -> Result<&'a DigiKeyVariation, String> {
    let found = match sku {
        Some(sku) => variations
            .iter()
            .find(|v| v.product_number.eq_ignore_ascii_case(sku)),
        None => variations.iter().find(|v| !v.standard_pricing.is_empty()),
    };
    let v = found.ok_or_else(|| match sku {
        Some(sku) => format!("no product variation numbered {sku}"),
        None => "no product variation lists prices".to_string(),
    })?;
    if v.standard_pricing.is_empty() {
        return Err(format!("variation {} lists no prices", v.product_number));
    }
    Ok(v)
}

#[derive(Debug, Default, Deserialize)]
struct DigiKeyManufacturer {
    #[serde(default, rename = "Name")]
    name: String,
}

#[derive(Debug, Default, Deserialize)]
struct DigiKeyDescription {
    #[serde(default, rename = "ProductDescription")]
    product_description: String,
}

impl DigiKeyResponse {
    /// `currency` is what the request asked for; `sku` selects the packaging
    /// whose pricing is attached.
    fn into_parts(self, currency: &str, sku: Option<&str>) -> Vec<DistributorPart> {
        self.products
            .into_iter()
            .map(|p| {
                let mut extra = BTreeMap::new();
                if let Some(u) = p.product_url {
                    extra.insert("digikey_product_url".into(), u);
                }
                let pricing = if p.variations.is_empty() {
                    None
                } else {
                    match pick_variation(&p.variations, sku) {
                        Ok(v) => {
                            extra.insert("digikey_pn".into(), v.product_number.clone());
                            let breaks: Option<Vec<PriceBreak>> = v
                                .standard_pricing
                                .iter()
                                .map(|b| {
                                    b.unit_price.map(|unit_price| PriceBreak {
                                        qty: b.break_quantity,
                                        unit_price,
                                    })
                                })
                                .collect();
                            match breaks {
                                None => {
                                    extra.insert(
                                        super::price::PRICING_ERROR_KEY.into(),
                                        format!(
                                            "variation {} has a price break with no UnitPrice",
                                            v.product_number
                                        ),
                                    );
                                    None
                                }
                                Some(breaks) => Some(super::price::snapshot(
                                    DIGIKEY_NAME,
                                    currency,
                                    p.quantity_available,
                                    Some(v.product_number.clone()),
                                    breaks,
                                )),
                            }
                        }
                        Err(why) => {
                            extra.insert(super::price::PRICING_ERROR_KEY.into(), why);
                            None
                        }
                    }
                };
                DistributorPart {
                    mpn: p.mpn,
                    manufacturer: p.manufacturer.name,
                    description: p.description.product_description,
                    datasheet_url: p.datasheet_url.as_deref().and_then(|u| Url::parse(u).ok()),
                    footprint_hint: None,
                    parameters: ParamMap::new(),
                    pricing,
                    stock: p.quantity_available,
                    source: DistributorSource::DigiKey,
                    captured_at: Utc::now(),
                    extra,
                }
            })
            .collect()
    }
}

/// The part token in a DigiKey product URL path. Product pages are
/// `/<lang>/products/detail/<mfr>/<mpn>/<numeric-id>`, and the trailing
/// numeric id matches no search hit, so the MPN segment before it is used.
/// Any other path falls back to its last non-empty segment.
fn digikey_path_token(path: &str) -> Option<String> {
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let last = *segs.last()?;
    if last.chars().all(|c| c.is_ascii_digit()) && segs.len() >= 2 {
        return Some(segs[segs.len() - 2].to_string());
    }
    Some(last.to_string())
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_product_url_uses_the_mpn_segment_not_the_numeric_id() {
        assert_eq!(
            super::digikey_path_token("/en/products/detail/yageo/RC0603FR-0710KL/727212")
                .as_deref(),
            Some("RC0603FR-0710KL")
        );
        assert_eq!(
            super::digikey_path_token("/en/products/detail/yageo/RC0603FR-0710KL/").as_deref(),
            Some("RC0603FR-0710KL")
        );
        assert_eq!(
            super::digikey_path_token("/product-detail/en/311-10.0KCRCT-ND").as_deref(),
            Some("311-10.0KCRCT-ND")
        );
        assert_eq!(super::digikey_path_token("/"), None);
    }

    use super::*;

    #[test]
    fn name_and_source_are_stable() {
        let a = DigiKeyAdapter::with_access_token("http://unused.invalid", "tok", None);
        assert_eq!(a.name(), "DigiKey");
        assert_eq!(a.source(), DistributorSource::DigiKey);
    }

    #[test]
    fn lookup_by_url_rejects_non_digikey_host() {
        let a = DigiKeyAdapter::with_access_token("http://unused.invalid", "tok", None);
        let url = Url::parse("https://example.com/x?k=foo").unwrap();
        assert!(a.lookup_by_url(&url).unwrap().is_none());
    }

    #[test]
    fn start_authorization_returns_pkce_protected_url() {
        let auth = DigiKeyAuth::with_endpoints(
            "client-id",
            "secret",
            "http://localhost/cb",
            "http://example.com/auth",
            "http://example.com/token",
        )
        .unwrap();
        let (url, _csrf, _verifier) = auth.start_authorization();
        let q: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(q.get("client_id").map(String::as_str), Some("client-id"));
        assert_eq!(
            q.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        assert!(q.contains_key("code_challenge"));
        assert!(q.contains_key("state"));
    }

    #[test]
    fn lookup_by_url_rejects_lookalike_host_offline() {
        // Dot-anchored host check returns before any network call.
        let a = DigiKeyAdapter::with_access_token("http://unused.invalid", "tok", None);
        let url = Url::parse("https://evildigikey.com/x").unwrap();
        assert!(a.lookup_by_url(&url).unwrap().is_none());
    }
}
