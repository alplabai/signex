//! JLCPCB distributor adapter — anonymous, polite-throttled (1 req/s).
//!
//! Spec (WS-C): same shape as LCSC — no auth, 1 req/s throttle, disk cache
//! with 24h TTL. JLCPCB's public component search returns an LCSC-style
//! response (the JLCPCB parts catalogue is a curated subset of LCSC).

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::Deserialize;
use url::Url;

use crate::distributor::{
    DistributorAdapter, DistributorError, DistributorPart, DistributorSource,
};
use crate::distributors::cache::{DistributorCache, DEFAULT_TTL};
use crate::embed::ParamMap;

const JLCPCB_PROVIDER_KEY: &str = "jlcpcb";
const JLCPCB_NAME: &str = "JLCPCB";
const JLCPCB_DEFAULT_BASE: &str = "https://jlcpcb.com/api/overseas-pcb-order/v1/shoppingCart/smtGood/list";
const THROTTLE_INTERVAL: Duration = Duration::from_secs(1);

pub struct JlcpcbAdapter {
    base_url: String,
    cache: Option<DistributorCache>,
    throttle: Mutex<Option<Instant>>,
    http: reqwest::blocking::Client,
}

impl JlcpcbAdapter {
    pub fn new(cache: Option<DistributorCache>) -> Self {
        Self {
            base_url: JLCPCB_DEFAULT_BASE.into(),
            cache,
            throttle: Mutex::new(None),
            http: reqwest::blocking::Client::builder()
                .user_agent("signex-library/0.9 (+https://signex.dev)")
                .build()
                .expect("reqwest::blocking::Client::build is infallible with default opts"),
        }
    }

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

    fn http_post_json<B: serde::Serialize, T: for<'de> Deserialize<'de>>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<T, DistributorError> {
        self.polite_wait();
        let resp = self
            .http
            .post(url)
            .json(body)
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
            return Err(DistributorError::Backend(format!(
                "HTTP {}",
                resp.status()
            )));
        }
        resp.json::<T>()
            .map_err(|e| DistributorError::Backend(format!("decode: {e}")))
    }

    fn search_by_keyword(&self, mpn: &str) -> Result<Vec<DistributorPart>, DistributorError> {
        let body = serde_json::json!({
            "keyword": mpn,
            "currentPage": 1,
            "pageSize": 25,
        });
        let raw: JlcpcbResponse = self.http_post_json(&self.base_url, &body)?;
        Ok(raw.into_parts())
    }
}

impl DistributorAdapter for JlcpcbAdapter {
    fn name(&self) -> &'static str {
        JLCPCB_NAME
    }

    fn source(&self) -> DistributorSource {
        DistributorSource::Jlcpcb
    }

    fn lookup_by_url(&self, url: &Url) -> Result<Option<DistributorPart>, DistributorError> {
        let host = url.host_str().unwrap_or("");
        if !host.ends_with("jlcpcb.com") {
            return Ok(None);
        }
        // JLCPCB part URLs include the LCSC code as a query param `partno=Cxxxx`
        // or in the path. Pull the first `C<digits>` token we find.
        let from_query = url
            .query_pairs()
            .find(|(k, _)| k == "partno" || k == "componentCode")
            .map(|(_, v)| v.to_string());
        let from_path = url.path().split('/').rev().find_map(|seg| {
            if seg.starts_with('C') && seg[1..].chars().all(|c| c.is_ascii_digit()) {
                Some(seg.to_string())
            } else {
                None
            }
        });
        let token = from_query.or(from_path).unwrap_or_default();
        if token.is_empty() {
            return Ok(None);
        }
        let mut hits = self.search_by_keyword(&token)?;
        Ok(hits.pop())
    }

    fn lookup_by_mpn(&self, mpn: &str) -> Result<Vec<DistributorPart>, DistributorError> {
        if let Some(cache) = &self.cache {
            if let Ok(Some(hit)) = cache.get(JLCPCB_PROVIDER_KEY, mpn, DEFAULT_TTL) {
                return Ok(vec![hit]);
            }
        }
        let hits = self.search_by_keyword(mpn)?;
        if let (Some(cache), Some(first)) = (&self.cache, hits.first()) {
            let _ = cache.put(JLCPCB_PROVIDER_KEY, first);
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
// Response shape (subset).
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct JlcpcbResponse {
    #[serde(default)]
    data: Option<JlcpcbData>,
}

#[derive(Debug, Deserialize)]
struct JlcpcbData {
    #[serde(default)]
    list: Vec<JlcpcbItem>,
}

#[derive(Debug, Deserialize)]
struct JlcpcbItem {
    #[serde(default, rename = "componentCode")]
    component_code: String,
    #[serde(default, rename = "mfrPart")]
    mfr_part: String,
    #[serde(default, rename = "manufacturer")]
    manufacturer: String,
    #[serde(default, rename = "describe")]
    describe: String,
    #[serde(default, rename = "stockCount")]
    stock_count: Option<u32>,
    #[serde(default, rename = "dataManualUrl")]
    data_manual_url: Option<String>,
    #[serde(default, rename = "componentSpecificationEn")]
    component_specification_en: Option<String>,
}

impl JlcpcbResponse {
    fn into_parts(self) -> Vec<DistributorPart> {
        let Some(data) = self.data else {
            return vec![];
        };
        data.list
            .into_iter()
            .map(|i| {
                let mut extra = BTreeMap::new();
                if !i.component_code.is_empty() {
                    extra.insert("jlcpcb_component_code".into(), i.component_code);
                }
                DistributorPart {
                    mpn: i.mfr_part,
                    manufacturer: i.manufacturer,
                    description: i.describe,
                    datasheet_url: i.data_manual_url.as_deref().and_then(|u| Url::parse(u).ok()),
                    footprint_hint: i.component_specification_en,
                    parameters: ParamMap::new(),
                    pricing: None,
                    stock: i.stock_count,
                    source: DistributorSource::Jlcpcb,
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
        let a = JlcpcbAdapter::with_base_url("http://unused.invalid", None);
        assert_eq!(a.name(), "JLCPCB");
        assert_eq!(a.source(), DistributorSource::Jlcpcb);
    }

    #[test]
    fn lookup_by_url_rejects_non_jlcpcb_host() {
        let a = JlcpcbAdapter::with_base_url("http://unused.invalid", None);
        let url = Url::parse("https://example.com/parts/C123").unwrap();
        assert!(a.lookup_by_url(&url).unwrap().is_none());
    }
}
