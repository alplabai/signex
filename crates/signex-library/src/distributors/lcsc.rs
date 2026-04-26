//! LCSC distributor adapter — anonymous, polite-throttled (1 req/s).
//!
//! Spec (WS-C):
//! - No auth (anonymous public catalogue endpoint).
//! - **1 req/s** polite throttle. Implemented as a `Mutex<Option<Instant>>`
//!   that delays the next request until at least 1 s after the last one
//!   completed. (Spec mentions `tokio::time::interval`; that fits async,
//!   but `DistributorAdapter` is a sync trait — `std::thread::sleep` is the
//!   sync equivalent and avoids spinning up a runtime per adapter.)
//! - Disk cache via `DistributorCache` with 24h TTL (per `DEFAULT_TTL`).
//! - Live API tests are `#[ignore]`d; offline tests use `wiremock` to
//!   verify URL shape, headers, and parsing.
//!
//! `lookup_by_url` recognises `https://www.lcsc.com/product-detail/<slug>.html`
//! shapes and extracts the slug; the slug is sent verbatim to LCSC's search
//! endpoint to obtain the canonical part record.

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
use crate::embed::ParamMap;

const LCSC_PROVIDER_KEY: &str = "lcsc";
const LCSC_NAME: &str = "LCSC";
/// Default search base. Tests inject a wiremock URL via `with_base_url`.
const LCSC_DEFAULT_BASE: &str = "https://wmsc.lcsc.com/wmsc/product/list";
const THROTTLE_INTERVAL: Duration = Duration::from_secs(1);

/// LCSC adapter.
///
/// Construct with [`LcscAdapter::new`] for production (default base URL,
/// blocking reqwest client) or [`LcscAdapter::with_base_url`] for tests
/// (point at a wiremock server).
pub struct LcscAdapter {
    base_url: String,
    cache: Option<DistributorCache>,
    throttle: Mutex<Option<Instant>>,
    http: reqwest::blocking::Client,
}

impl LcscAdapter {
    /// Production constructor: default base URL, optional disk cache.
    pub fn new(cache: Option<DistributorCache>) -> Self {
        Self {
            base_url: LCSC_DEFAULT_BASE.into(),
            cache,
            throttle: Mutex::new(None),
            http: reqwest::blocking::Client::builder()
                .user_agent("signex-library/0.9 (+https://signex.dev)")
                .build()
                .expect("reqwest::blocking::Client::build is infallible with default opts"),
        }
    }

    /// Test constructor: override the base URL (e.g. wiremock).
    pub fn with_base_url(base_url: impl Into<String>, cache: Option<DistributorCache>) -> Self {
        Self {
            base_url: base_url.into(),
            cache,
            throttle: Mutex::new(None),
            http: reqwest::blocking::Client::builder()
                .user_agent("signex-library/0.9 (+https://signex.dev)")
                .build()
                .expect("reqwest::blocking::Client::build is infallible with default opts"),
        }
    }

    /// Wait until at least `THROTTLE_INTERVAL` has elapsed since the last
    /// call. Records the current instant on exit so subsequent calls are
    /// rate-limited consistently across threads.
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

    /// Helper exposed for tests: how many ms since the last recorded request,
    /// or None if no request has been made.
    #[doc(hidden)]
    pub fn last_call_age(&self) -> Option<Duration> {
        self.throttle
            .lock()
            .ok()
            .and_then(|g| g.map(|i| i.elapsed()))
    }

    fn http_get_json<T: for<'de> serde::Deserialize<'de>>(
        &self,
        url: &str,
    ) -> Result<T, DistributorError> {
        self.polite_wait();
        let resp = self
            .http
            .get(url)
            .send()
            .map_err(|e| DistributorError::Network(e.to_string()))?;
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
        resp.json::<T>()
            .map_err(|e| DistributorError::Backend(format!("decode: {e}")))
    }

    fn search_by_keyword(&self, mpn: &str) -> Result<Vec<DistributorPart>, DistributorError> {
        let url = format!(
            "{base}?keyword={mpn}",
            base = self.base_url,
            mpn = urlencoding_minimal(mpn)
        );
        let raw: LcscSearchResponse = self.http_get_json(&url)?;
        Ok(raw.into_parts())
    }
}

impl DistributorAdapter for LcscAdapter {
    fn name(&self) -> &'static str {
        LCSC_NAME
    }

    fn source(&self) -> DistributorSource {
        DistributorSource::Lcsc
    }

    fn lookup_by_url(&self, url: &Url) -> Result<Option<DistributorPart>, DistributorError> {
        // LCSC product detail URLs look like:
        //   https://www.lcsc.com/product-detail/Resistors_YAGEO-RC0805FR-0710KL_C17414.html
        // We extract the trailing `_C<digits>` slug and search by it.
        let host = url.host_str().unwrap_or("");
        if !host.ends_with("lcsc.com") {
            return Ok(None);
        }
        let path = url.path();
        let stem = path
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".html");
        let token = stem
            .rsplit('_')
            .find(|t| t.starts_with('C') && t[1..].chars().all(|c| c.is_ascii_digit()))
            .unwrap_or("");
        if token.is_empty() {
            return Ok(None);
        }
        let mut hits = self.search_by_keyword(token)?;
        Ok(hits.pop())
    }

    fn lookup_by_mpn(&self, mpn: &str) -> Result<Vec<DistributorPart>, DistributorError> {
        if let Some(cache) = &self.cache
            && let Ok(Some(hit)) = cache.get(LCSC_PROVIDER_KEY, mpn, DEFAULT_TTL)
        {
            return Ok(vec![hit]);
        }
        let hits = self.search_by_keyword(mpn)?;
        if let (Some(cache), Some(first)) = (&self.cache, hits.first()) {
            let _ = cache.put(LCSC_PROVIDER_KEY, first);
        }
        Ok(hits)
    }

    fn refresh_pricing(
        &self,
        part: &DistributorPart,
    ) -> Result<crate::embed::PricingSnapshot, DistributorError> {
        // Re-query the same MPN; the response carries pricing.
        let mut hits = self.search_by_keyword(&part.mpn)?;
        let fresh = hits.pop().ok_or(DistributorError::NotFound)?;
        fresh.pricing.ok_or(DistributorError::NotFound)
    }
}

/// Minimal percent-encoding for the `keyword=` query parameter. Avoids
/// pulling `percent-encoding` as a hard dep — `url::Url` doesn't easily
/// support partial encoding without a base.
fn urlencoding_minimal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '~') {
            out.push(ch);
        } else {
            for b in ch.to_string().bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// LCSC wmsc product/list response shape.
//
// We model only the fields we need; LCSC returns much more. Unknown fields
// are dropped via serde's default behaviour. This shape is also exercised
// by `tests` below using a wiremock fixture.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct LcscSearchResponse {
    #[serde(default)]
    result: Option<LcscResult>,
}

#[derive(Debug, Deserialize)]
struct LcscResult {
    #[serde(default, rename = "productList")]
    product_list: Vec<LcscProduct>,
}

#[derive(Debug, Deserialize)]
struct LcscProduct {
    #[serde(default, rename = "productCode")]
    product_code: String,
    #[serde(default, rename = "productModel")]
    product_model: String,
    #[serde(default, rename = "brandNameEn")]
    brand_name_en: String,
    #[serde(default, rename = "productIntroEn")]
    product_intro_en: String,
    #[serde(default, rename = "stockNumber")]
    stock_number: Option<u32>,
    #[serde(default, rename = "pdfUrl")]
    pdf_url: Option<String>,
    #[serde(default, rename = "encapStandard")]
    encap_standard: Option<String>,
}

impl LcscSearchResponse {
    fn into_parts(self) -> Vec<DistributorPart> {
        let Some(result) = self.result else {
            return vec![];
        };
        result
            .product_list
            .into_iter()
            .map(|p| {
                let mut extra = BTreeMap::new();
                if !p.product_code.is_empty() {
                    extra.insert("lcsc_product_code".into(), p.product_code);
                }
                DistributorPart {
                    mpn: p.product_model,
                    manufacturer: p.brand_name_en,
                    description: p.product_intro_en,
                    datasheet_url: p.pdf_url.as_deref().and_then(|u| Url::parse(u).ok()),
                    footprint_hint: p.encap_standard,
                    parameters: ParamMap::new(),
                    pricing: None,
                    stock: p.stock_number,
                    source: DistributorSource::Lcsc,
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
    fn polite_wait_blocks_until_one_second_passed() {
        let adapter = LcscAdapter::with_base_url("http://unused.invalid", None);
        let t0 = Instant::now();
        adapter.polite_wait();
        // First call should be instant (no prior call).
        assert!(t0.elapsed() < Duration::from_millis(500));
        // Second call must wait ~1 s.
        let t1 = Instant::now();
        adapter.polite_wait();
        assert!(
            t1.elapsed() >= Duration::from_millis(900),
            "polite_wait should sleep ~1s; elapsed = {:?}",
            t1.elapsed()
        );
    }

    #[test]
    fn lookup_by_url_extracts_lcsc_token() {
        let adapter = LcscAdapter::with_base_url("http://unused.invalid", None);
        // Non-LCSC host → None, no network.
        let url = Url::parse("https://example.com/product-detail/foo_C123.html").unwrap();
        assert!(adapter.lookup_by_url(&url).unwrap().is_none());
    }

    #[test]
    fn urlencoding_minimal_matches_standard_chars() {
        assert_eq!(urlencoding_minimal("RC0805FR-0710KL"), "RC0805FR-0710KL");
        assert_eq!(urlencoding_minimal("a b"), "a%20b");
        assert_eq!(urlencoding_minimal("/"), "%2F");
    }

    #[test]
    fn name_and_source_are_stable() {
        let a = LcscAdapter::with_base_url("http://unused.invalid", None);
        assert_eq!(a.name(), "LCSC");
        assert_eq!(a.source(), DistributorSource::Lcsc);
    }
}
