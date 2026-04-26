//! DigiKey distributor adapter — OAuth2 PKCE scaffold.
//!
//! Spec (WS-C):
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
    DistributorAdapter, DistributorError, DistributorPart, DistributorSource,
};
use crate::distributors::cache::{DEFAULT_TTL, DistributorCache};
use crate::distributors::keyring::{KeyringError, KeyringStore};
use crate::embed::ParamMap;

/// DigiKey production endpoints. Tests override these via `with_endpoints`.
pub const DIGIKEY_AUTH_URL: &str = "https://api.digikey.com/v1/oauth2/authorize";
pub const DIGIKEY_TOKEN_URL: &str = "https://api.digikey.com/v1/oauth2/token";
pub const DIGIKEY_API_BASE: &str = "https://api.digikey.com";
const DIGIKEY_PROVIDER_KEY: &str = "digikey";
const DIGIKEY_NAME: &str = "DigiKey";
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
        }
    }
}

/// Scaffolds the OAuth2 PKCE authorization-code flow.
pub struct DigiKeyAuth {
    client: ConfiguredClient,
    keyring: KeyringStore,
    /// Test-only fallback: when set, [`DigiKeyAuth::access_token`] uses this
    /// refresh token instead of reading from the OS keyring. Lets wiremock
    /// integration tests cover the refresh-grant path without depending on
    /// the platform keyring. Production callers leave it `None`.
    fallback_refresh: Option<String>,
}

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

        let client = BasicClient::new(ClientId::new(client_id.into()))
            .set_client_secret(ClientSecret::new(client_secret.into()))
            .set_auth_uri(auth_url)
            .set_token_uri(token_url)
            .set_redirect_uri(redirect_url);

        Ok(Self {
            client,
            keyring: KeyringStore::for_provider("digikey", "refresh"),
            fallback_refresh: None,
        })
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
    pub fn exchange_code(
        &self,
        code: &str,
        verifier: PkceCodeVerifier,
    ) -> Result<String, DigiKeyAuthError> {
        let http = build_http_client();
        let token = self
            .client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_pkce_verifier(verifier)
            .request(&http)
            .map_err(|e| DigiKeyAuthError::Request(e.to_string()))?;
        if let Some(refresh) = token.refresh_token() {
            self.keyring
                .set_secret(refresh.secret())
                .map_err(|e| DigiKeyAuthError::Keyring(e.to_string()))?;
        }
        Ok(token.access_token().secret().clone())
    }

    /// Step 3 (every API call): use the keyring-stored refresh token to
    /// mint a fresh access token. Honours [`Self::with_test_refresh_token`]
    /// when set (tests).
    pub fn access_token(&self) -> Result<String, DigiKeyAuthError> {
        let refresh = if let Some(t) = &self.fallback_refresh {
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
        // Rotate: persist any newly-issued refresh token. If the fallback
        // is in use we update it so subsequent calls within the same
        // process see the rotated value.
        //
        // L5: a failed keyring write means DigiKey has consumed the old
        // refresh token but we couldn't store the new one — the next call
        // would fail with an opaque auth error. Surface it via tracing so
        // operators see *why* before debugging blind.
        if let Some(new_refresh) = token.refresh_token()
            && self.fallback_refresh.is_none()
        {
            if let Err(e) = self.keyring.set_secret(new_refresh.secret()) {
                tracing::warn!(
                    error = %e,
                    "failed to persist rotated DigiKey refresh token to keyring; \
                     next refresh will fail until the keyring is writable",
                );
            }
        }
        Ok(token.access_token().secret().clone())
    }
}

fn build_http_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .user_agent("signex-library/0.9 (+https://signex.dev)")
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("reqwest::blocking::Client::build is infallible with default opts")
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
        Self {
            api_base: DIGIKEY_API_BASE.into(),
            cache,
            throttle: Mutex::new(None),
            http: build_http_client(),
            auth: AuthSource::Oauth(Box::new(auth)),
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
        Self {
            api_base: api_base.into(),
            cache,
            throttle: Mutex::new(None),
            http: build_http_client(),
            auth: AuthSource::Oauth(Box::new(auth)),
        }
    }

    fn polite_wait(&self) {
        let mut guard = self.throttle.lock().expect("throttle mutex poisoned");
        if let Some(prev) = *guard {
            let elapsed = prev.elapsed();
            if elapsed < THROTTLE_INTERVAL {
                drop(guard);
                std::thread::sleep(THROTTLE_INTERVAL - elapsed);
                guard = self.throttle.lock().expect("throttle mutex poisoned");
            }
        }
        *guard = Some(Instant::now());
    }

    fn resolve_access_token(&self) -> Result<String, DistributorError> {
        match &self.auth {
            AuthSource::InlineAccessToken(t) => Ok(t.clone()),
            AuthSource::Oauth(a) => a.access_token().map_err(DistributorError::from),
        }
    }

    fn search_by_keyword(&self, mpn: &str) -> Result<Vec<DistributorPart>, DistributorError> {
        let token = self.resolve_access_token()?;
        let url = format!("{}/products/v4/search/keyword", self.api_base);
        let body = serde_json::json!({
            "Keywords": mpn,
            "Limit": 25,
            "Offset": 0,
        });
        self.polite_wait();
        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/json")
            .json(&body)
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
        if !resp.status().is_success() {
            return Err(DistributorError::Backend(format!("HTTP {}", resp.status())));
        }
        let raw: DigiKeyResponse = resp
            .json()
            .map_err(|e| DistributorError::Backend(format!("decode: {e}")))?;
        Ok(raw.into_parts())
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
        if !host.ends_with("digikey.com") {
            return Ok(None);
        }
        let from_query = url
            .query_pairs()
            .find(|(k, _)| {
                let lk = k.to_ascii_lowercase();
                lk == "keywords" || lk == "k" || lk == "search"
            })
            .map(|(_, v)| v.to_string());
        let from_path = url
            .path()
            .rsplit('/')
            .find(|seg| !seg.is_empty())
            .map(|s| s.to_string());
        let token = from_query.or(from_path).unwrap_or_default();
        if token.is_empty() {
            return Ok(None);
        }
        let mut hits = self.search_by_keyword(&token)?;
        Ok(hits.pop())
    }

    fn lookup_by_mpn(&self, mpn: &str) -> Result<Vec<DistributorPart>, DistributorError> {
        if let Some(cache) = &self.cache
            && let Ok(Some(hit)) = cache.get(DIGIKEY_PROVIDER_KEY, mpn, DEFAULT_TTL)
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
    ) -> Result<crate::embed::PricingSnapshot, DistributorError> {
        let mut hits = self.search_by_keyword(&part.mpn)?;
        let fresh = hits.pop().ok_or(DistributorError::NotFound)?;
        fresh.pricing.ok_or(DistributorError::NotFound)
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
    fn into_parts(self) -> Vec<DistributorPart> {
        self.products
            .into_iter()
            .map(|p| {
                let mut extra = BTreeMap::new();
                if let Some(u) = p.product_url {
                    extra.insert("digikey_product_url".into(), u);
                }
                DistributorPart {
                    mpn: p.mpn,
                    manufacturer: p.manufacturer.name,
                    description: p.description.product_description,
                    datasheet_url: p.datasheet_url.as_deref().and_then(|u| Url::parse(u).ok()),
                    footprint_hint: None,
                    parameters: ParamMap::new(),
                    pricing: None,
                    stock: p.quantity_available,
                    source: DistributorSource::DigiKey,
                    captured_at: Utc::now(),
                    extra,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
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
}
