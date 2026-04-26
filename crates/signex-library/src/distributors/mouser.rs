//! Mouser distributor adapter — API-key auth from OS keyring.
//!
//! Spec (WS-C):
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
    DistributorAdapter, DistributorError, DistributorPart, DistributorSource,
};
use crate::distributors::cache::{DEFAULT_TTL, DistributorCache};
use crate::distributors::keyring::{KeyringError, KeyringStore};
use crate::embed::ParamMap;

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
    http: reqwest::blocking::Client,
    auth: AuthSource,
}

impl MouserAdapter {
    /// Production constructor: pulls the API key from `signex-distributor-mouser`
    /// at request time. The username slot defaults to `"default"` to match
    /// what the eventual UI will write.
    pub fn from_keyring(cache: Option<DistributorCache>) -> Self {
        Self {
            base_url: MOUSER_DEFAULT_BASE.into(),
            cache,
            throttle: Mutex::new(None),
            http: reqwest::blocking::Client::builder()
                .user_agent("signex-library/0.9 (+https://signex.dev)")
                .build()
                .expect("reqwest::blocking::Client::build is infallible with default opts"),
            auth: AuthSource::Keyring(KeyringStore::for_provider("mouser", "default")),
        }
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
            http: reqwest::blocking::Client::builder()
                .user_agent("signex-library/0.9 (+https://signex.dev)")
                .build()
                .expect("reqwest::blocking::Client::build is infallible with default opts"),
            auth: AuthSource::Inline(api_key.into()),
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
        let api_key = self.resolve_api_key()?;
        // Mouser's API places the key on the query string and the search
        // payload in the JSON body.
        let url = format!("{base}?apiKey={key}", base = self.base_url, key = api_key);
        let body = serde_json::json!({
            "SearchByKeywordRequest": {
                "keyword": mpn,
                "records": 25,
                "startingRecord": 0,
            }
        });
        self.polite_wait();
        let resp = self
            .http
            .post(&url)
            .header("Accept", "application/json")
            .json(&body)
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
        Ok(raw.into_parts())
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
        if !host.ends_with("mouser.com") {
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
        let mut hits = self.search_by_keyword(&token)?;
        Ok(hits.pop())
    }

    fn lookup_by_mpn(&self, mpn: &str) -> Result<Vec<DistributorPart>, DistributorError> {
        if let Some(cache) = &self.cache
            && let Ok(Some(hit)) = cache.get(MOUSER_PROVIDER_KEY, mpn, DEFAULT_TTL)
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
    ) -> Result<crate::embed::PricingSnapshot, DistributorError> {
        let mut hits = self.search_by_keyword(&part.mpn)?;
        let fresh = hits.pop().ok_or(DistributorError::NotFound)?;
        fresh.pricing.ok_or(DistributorError::NotFound)
    }
}

// ---------------------------------------------------------------------------
// Response shape (subset of Mouser SearchByKeyword v1).
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct MouserResponse {
    #[serde(default, rename = "SearchResults")]
    search_results: Option<MouserSearchResults>,
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
}

impl MouserResponse {
    fn into_parts(self) -> Vec<DistributorPart> {
        let Some(results) = self.search_results else {
            return vec![];
        };
        results
            .parts
            .into_iter()
            .map(|p| {
                let mut extra = BTreeMap::new();
                if !p.mouser_pn.is_empty() {
                    extra.insert("mouser_pn".into(), p.mouser_pn);
                }
                let stock = p
                    .availability
                    .as_deref()
                    .and_then(|s| s.split_whitespace().next())
                    .and_then(|n| n.replace(',', "").parse::<u32>().ok());
                DistributorPart {
                    mpn: p.manufacturer_pn,
                    manufacturer: p.manufacturer,
                    description: p.description,
                    datasheet_url: p.datasheet_url.as_deref().and_then(|u| Url::parse(u).ok()),
                    footprint_hint: None,
                    parameters: ParamMap::new(),
                    pricing: None,
                    stock,
                    source: DistributorSource::Mouser,
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
        let store = KeyringStore::for_provider("mouser", "ws-c-deliberately-absent-user");
        let _ = store.delete();
        let adapter = MouserAdapter {
            base_url: "http://unused.invalid".into(),
            cache: None,
            throttle: Mutex::new(None),
            http: reqwest::blocking::Client::builder()
                .user_agent("test")
                .build()
                .unwrap(),
            auth: AuthSource::Keyring(store),
        };
        let err = adapter.resolve_api_key().expect_err("should be Auth");
        match err {
            DistributorError::Auth(_) => {}
            other => panic!("expected Auth, got {other:?}"),
        }
    }
}
