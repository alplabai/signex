//! Mouser distributor adapter — API-key auth from OS keyring.
//!
//! - API key stored in OS keyring under service name
//!   `signex-distributor-mouser`. Adapter accepts the key directly via
//!   `with_api_key` (test-friendly) or pulls it lazily from `KeyringStore`.
//! - Mouser's Search API takes JSON POST bodies with an `apiKey` query
//!   string parameter. We use that placement (vs header) for spec parity
//!   with Mouser's documented flow; the spec calls this "header auth" but
//!   Mouser actually accepts both — we use the simpler query form.
//! - 24h disk cache, polite throttle, same error-mapping as LCSC.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::Deserialize;
use url::Url;

use crate::distributor::{
    DistributorAdapter, DistributorError, DistributorPart, DistributorSource, PriceBreak,
    PricingSnapshot,
};
use crate::distributors::cache::{DEFAULT_TTL, DistributorCache};
use crate::distributors::keyring::{KeyringError, KeyringStore};
use crate::param::ParamMap;

const MOUSER_PROVIDER_KEY: &str = "mouser";
const MOUSER_NAME: &str = "Mouser";
const MOUSER_DEFAULT_BASE: &str = "https://api.mouser.com/api/v1/search/keyword";
const THROTTLE_INTERVAL: Duration = Duration::from_secs(1);

/// How the Mouser adapter retrieves its API key.
enum AuthSource {
    /// Direct key (tests). Bypasses the keyring.
    Inline(String),
    /// Keyring-backed lookup at request time.
    Keyring(KeyringStore),
}

pub struct MouserAdapter {
    base_url: String,
    cache: Option<DistributorCache>,
    throttle: Mutex<Option<Instant>>,
    http: super::http::LazyClient,
    auth: AuthSource,
}

impl MouserAdapter {
    /// Production constructor: pulls the API key from `signex-distributor-mouser`
    /// at request time. The username slot defaults to `"default"` to match
    /// what the eventual UI will write.
    ///
    /// MD-17: returns `Result` because the OS keychain may not be
    /// available (Linux Docker without dbus, etc). On error, callers
    /// can fall back to `with_api_key` (env-var-driven) or surface a
    /// "keychain unavailable" message to the user.
    pub fn from_keyring(
        cache: Option<DistributorCache>,
    ) -> Result<Self, super::keyring::KeyringError> {
        Ok(Self {
            base_url: MOUSER_DEFAULT_BASE.into(),
            cache,
            throttle: Mutex::new(None),
            http: super::http::LazyClient::standard(),
            auth: AuthSource::Keyring(KeyringStore::for_provider("mouser", "default")?),
        })
    }

    /// Test constructor: inline API key, override base URL.
    pub fn with_api_key(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        cache: Option<DistributorCache>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            cache,
            throttle: Mutex::new(None),
            http: super::http::LazyClient::standard(),
            auth: AuthSource::Inline(api_key.into()),
        }
    }

    fn polite_wait(&self) {
        super::throttle::polite_wait(&self.throttle, THROTTLE_INTERVAL);
    }

    fn resolve_api_key(&self) -> Result<String, DistributorError> {
        match &self.auth {
            AuthSource::Inline(k) => Ok(k.clone()),
            AuthSource::Keyring(store) => store.get_secret().map_err(|e| match e {
                KeyringError::NotFound => DistributorError::Auth(
                    "no Mouser API key in keyring (signex-distributor-mouser/default)".into(),
                ),
                KeyringError::Backend(msg) => DistributorError::Auth(msg),
            }),
        }
    }

    fn search_by_keyword(&self, mpn: &str) -> Result<Vec<DistributorPart>, DistributorError> {
        let body = serde_json::json!({
            "SearchByKeywordRequest": {
                "keyword": mpn,
                "records": 25,
                "startingRecord": 0,
            }
        });
        self.post_search(&self.base_url, &body)
    }

    /// Look a Mouser part number up directly (`/search/partnumber`).
    fn search_by_part_number(&self, sku: &str) -> Result<Vec<DistributorPart>, DistributorError> {
        let url = match self.base_url.strip_suffix("/keyword") {
            Some(prefix) => format!("{prefix}/partnumber"),
            None => self.base_url.clone(),
        };
        let body = serde_json::json!({
            "SearchByPartRequest": { "mouserPartNumber": sku }
        });
        self.post_search(&url, &body)
    }

    fn post_search(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<Vec<DistributorPart>, DistributorError> {
        let api_key = self.resolve_api_key()?;
        // H2: previously the Mouser API key was placed on the query string
        // (`?apiKey=...`), which leaks into proxy logs, CDN access logs, and
        // browser history if the URL is ever inspected via the wire. The
        // Mouser SearchAPI also accepts the key via the `apiKey` HTTP header,
        // which keeps the credential out of URL space.
        self.polite_wait();
        let resp = self
            .http
            .get()?
            .post(url)
            .header("Accept", "application/json")
            .header("apiKey", &api_key)
            .json(body)
            .send()
            .map_err(|e| DistributorError::Network(e.to_string()))?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || resp.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(DistributorError::Auth(format!(
                "Mouser rejected key (HTTP {})",
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
        let raw: MouserResponse = resp
            .json()
            .map_err(|e| DistributorError::Backend(format!("decode: {e}")))?;
        raw.into_parts()
    }
}

impl DistributorAdapter for MouserAdapter {
    fn name(&self) -> &'static str {
        MOUSER_NAME
    }

    fn source(&self) -> DistributorSource {
        DistributorSource::Mouser
    }

    fn lookup_by_url(&self, url: &Url) -> Result<Option<DistributorPart>, DistributorError> {
        let host = url.host_str().unwrap_or("");
        if !super::host_in_domain(host, "mouser.com") {
            return Ok(None);
        }
        // Mouser product pages have the part number in either the path or
        // a query parameter. Try the obvious shapes.
        let from_query = url
            .query_pairs()
            .find(|(k, _)| {
                let lk = k.to_ascii_lowercase();
                lk == "qs" || lk == "search" || lk == "keyword"
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
        let hits = self.search_by_keyword(&token)?;
        Ok(super::price::pick_url_hit(hits, &token, &["mouser_pn"]))
    }

    fn lookup_by_mpn(&self, mpn: &str) -> Result<Vec<DistributorPart>, DistributorError> {
        if let Some(cache) = &self.cache
            && let Some(hit) = cache.get_or_warn(MOUSER_PROVIDER_KEY, mpn, DEFAULT_TTL)
        {
            return Ok(vec![hit]);
        }
        let hits = self.search_by_keyword(mpn)?;
        if let (Some(cache), Some(first)) = (&self.cache, hits.first()) {
            // H4: cache-write failures (disk full, permissions, traversal-rejected
            // MPN) used to be silently swallowed. Surface them via tracing so
            // the next API call doesn't burn quota with no operator signal —
            // the lookup still succeeds because the live result is in `hits`.
            if let Err(e) = cache.put(MOUSER_PROVIDER_KEY, first) {
                tracing::warn!(
                    error = %e,
                    mpn = %mpn,
                    "Mouser cache write failed; live result returned but not persisted",
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
        // The part searches are fuzzy, so a hit is accepted only when it IS
        // the row's part: the SKU exactly, or (no SKU) the MPN and maker
        // exactly. Anything else is "not found" — never a neighbour's price.
        let fresh = match sku {
            Some(sku) => self
                .search_by_part_number(sku)?
                .into_iter()
                .find(|p| {
                    p.extra
                        .get("mouser_pn")
                        .is_some_and(|pn| pn.trim().eq_ignore_ascii_case(sku))
                })
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
        true
    }
}

// ---------------------------------------------------------------------------
// Response shape (subset of Mouser SearchByKeyword v1).
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct MouserResponse {
    /// Mouser reports a bad key or an exhausted quota with HTTP 200 and this
    /// list populated, not with a 4xx.
    #[serde(default, rename = "Errors")]
    errors: Vec<MouserApiError>,
    #[serde(default, rename = "SearchResults")]
    search_results: Option<MouserSearchResults>,
}

#[derive(Debug, Deserialize)]
struct MouserApiError {
    #[serde(default, rename = "Code")]
    code: String,
    #[serde(default, rename = "Message")]
    message: String,
    #[serde(default, rename = "ResourceKey")]
    resource_key: String,
}

impl MouserApiError {
    fn into_error(self) -> DistributorError {
        let text = format!("{} {} {}", self.code, self.resource_key, self.message).to_lowercase();
        let detail = if self.message.trim().is_empty() {
            self.code.clone()
        } else {
            self.message.clone()
        };
        if text.contains("toomany") || text.contains("too many") || text.contains("limit") {
            DistributorError::RateLimited {
                retry_after_seconds: 60,
            }
        } else if text.contains("unauthor")
            || text.contains("apikey")
            || text.contains("api key")
            || text.contains("identifier")
            || text.contains("forbidden")
        {
            DistributorError::Auth(detail)
        } else {
            DistributorError::Backend(detail)
        }
    }
}

#[derive(Debug, Deserialize)]
struct MouserSearchResults {
    #[serde(default, rename = "Parts")]
    parts: Vec<MouserPartDto>,
}

#[derive(Debug, Deserialize)]
struct MouserPartDto {
    #[serde(default, rename = "MouserPartNumber")]
    mouser_pn: String,
    #[serde(default, rename = "ManufacturerPartNumber")]
    manufacturer_pn: String,
    #[serde(default, rename = "Manufacturer")]
    manufacturer: String,
    #[serde(default, rename = "Description")]
    description: String,
    #[serde(default, rename = "DataSheetUrl")]
    datasheet_url: Option<String>,
    #[serde(default, rename = "Availability")]
    availability: Option<String>,
    /// Units in stock as a bare number (string or number in the payload).
    #[serde(default, rename = "AvailabilityInStock")]
    availability_in_stock: Option<serde_json::Value>,
    #[serde(default, rename = "PriceBreaks")]
    price_breaks: Vec<MouserPriceBreak>,
}

#[derive(Debug, Deserialize)]
struct MouserPriceBreak {
    #[serde(default, rename = "Quantity")]
    quantity: u32,
    /// Display string in the request locale: `"$0.10"`, `"0,10 €"`.
    #[serde(default, rename = "Price")]
    price: String,
    #[serde(default, rename = "Currency")]
    currency: String,
}

/// Read Mouser's `PriceBreaks`: one currency across the breaks, finite
/// prices. `None` when the part lists no prices (a quote-only part).
fn mouser_pricing(
    breaks: &[MouserPriceBreak],
    stock: Option<u32>,
    part_number: Option<String>,
) -> Option<Result<PricingSnapshot, String>> {
    let first = breaks.first()?;
    let read = || -> Result<PricingSnapshot, String> {
        let currency = first.currency.trim().to_ascii_uppercase();
        let mut out = Vec::with_capacity(breaks.len());
        for b in breaks {
            if !b.currency.trim().eq_ignore_ascii_case(&currency) {
                return Err(format!(
                    "price breaks mix currencies ({currency} and {})",
                    b.currency
                ));
            }
            out.push(PriceBreak {
                qty: b.quantity,
                unit_price: super::price::parse_price(&b.price, &currency)?,
            });
        }
        Ok(super::price::snapshot(
            MOUSER_NAME,
            &currency,
            stock,
            part_number,
            out,
        ))
    };
    Some(read())
}

impl MouserResponse {
    fn into_parts(mut self) -> Result<Vec<DistributorPart>, DistributorError> {
        if !self.errors.is_empty() {
            return Err(self.errors.remove(0).into_error());
        }
        let Some(results) = self.search_results else {
            return Ok(vec![]);
        };
        Ok(results
            .parts
            .into_iter()
            .map(|p| {
                let mut extra = BTreeMap::new();
                let mouser_pn = Some(p.mouser_pn.clone()).filter(|pn| !pn.is_empty());
                if let Some(pn) = &mouser_pn {
                    extra.insert("mouser_pn".into(), pn.clone());
                }
                let stock = p
                    .availability_in_stock
                    .as_ref()
                    .and_then(|v| match v {
                        serde_json::Value::Number(n) => {
                            n.as_u64().and_then(|n| u32::try_from(n).ok())
                        }
                        serde_json::Value::String(s) => {
                            s.replace(',', "").trim().parse::<u32>().ok()
                        }
                        _ => None,
                    })
                    .or_else(|| {
                        p.availability
                            .as_deref()
                            .and_then(|s| s.split_whitespace().next())
                            .and_then(|n| n.replace(',', "").parse::<u32>().ok())
                    });
                let pricing = match mouser_pricing(&p.price_breaks, stock, mouser_pn.clone()) {
                    Some(Ok(snapshot)) => Some(snapshot),
                    Some(Err(why)) => {
                        extra.insert(super::price::PRICING_ERROR_KEY.into(), why);
                        None
                    }
                    None => None,
                };
                DistributorPart {
                    mpn: p.manufacturer_pn,
                    manufacturer: p.manufacturer,
                    description: p.description,
                    datasheet_url: p.datasheet_url.as_deref().and_then(|u| Url::parse(u).ok()),
                    footprint_hint: None,
                    parameters: ParamMap::new(),
                    pricing,
                    stock,
                    source: DistributorSource::Mouser,
                    captured_at: Utc::now(),
                    extra,
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_and_source_are_stable() {
        let a = MouserAdapter::with_api_key("http://unused.invalid", "key", None);
        assert_eq!(a.name(), "Mouser");
        assert_eq!(a.source(), DistributorSource::Mouser);
    }

    #[test]
    fn lookup_by_url_rejects_non_mouser_host() {
        let a = MouserAdapter::with_api_key("http://unused.invalid", "key", None);
        let url = Url::parse("https://example.com/x").unwrap();
        assert!(a.lookup_by_url(&url).unwrap().is_none());
    }

    #[test]
    fn missing_keyring_key_yields_auth_error() {
        // Force keyring path with a guaranteed-absent username.
        // MD-17: skip on platforms without a keyring daemon.
        let Ok(store) = KeyringStore::for_provider("mouser", "ws-c-deliberately-absent-user")
        else {
            return;
        };
        let _ = store.delete();
        let adapter = MouserAdapter {
            base_url: "http://unused.invalid".into(),
            cache: None,
            throttle: Mutex::new(None),
            http: super::super::http::LazyClient::standard(),
            auth: AuthSource::Keyring(store),
        };
        let err = adapter.resolve_api_key().expect_err("should be Auth");
        match err {
            DistributorError::Auth(_) => {}
            other => panic!("expected Auth, got {other:?}"),
        }
    }

    #[test]
    fn lookup_by_url_rejects_lookalike_host_offline() {
        // Dot-anchored host check returns before any network call.
        let a = MouserAdapter::with_api_key("http://unused.invalid", "key", None);
        let url = Url::parse("https://evilmouser.com/x").unwrap();
        assert!(a.lookup_by_url(&url).unwrap().is_none());
    }
}
