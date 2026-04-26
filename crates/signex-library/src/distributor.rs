//! `DistributorAdapter` — vendor metadata + pricing lookup. LIBRARY_PLAN §14a.4.

use std::collections::BTreeMap;

use crate::embed::{ParamMap, PricingSnapshot};

#[derive(Debug, thiserror::Error)]
pub enum DistributorError {
    #[error("auth: {0}")]
    Auth(String),
    #[error("network: {0}")]
    Network(String),
    #[error("rate-limited; retry after {retry_after_seconds}s")]
    RateLimited { retry_after_seconds: u64 },
    #[error("not found")]
    NotFound,
    #[error("backend: {0}")]
    Backend(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DistributorSource {
    DigiKey,
    Mouser,
    Lcsc,
    Jlcpcb,
    Octopart,
    Oemsecrets,
    Other,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DistributorPart {
    pub mpn: String,
    pub manufacturer: String,
    pub description: String,
    pub datasheet_url: Option<url::Url>,
    pub footprint_hint: Option<String>,
    pub parameters: ParamMap,
    pub pricing: Option<PricingSnapshot>,
    pub stock: Option<u32>,
    pub source: DistributorSource,
    /// Captured timestamp — for cache TTL.
    pub captured_at: chrono::DateTime<chrono::Utc>,
    /// Distributor-specific metadata not normalised into the typed fields above.
    #[serde(default)]
    pub extra: BTreeMap<String, String>,
}

pub trait DistributorAdapter: Send + Sync {
    fn name(&self) -> &'static str;
    fn source(&self) -> DistributorSource;

    fn lookup_by_url(&self, url: &url::Url) -> Result<Option<DistributorPart>, DistributorError>;

    fn lookup_by_mpn(&self, mpn: &str) -> Result<Vec<DistributorPart>, DistributorError>;

    fn refresh_pricing(&self, part: &DistributorPart) -> Result<PricingSnapshot, DistributorError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distributor_adapter_is_object_safe() {
        fn _accepts_dyn(_a: &dyn DistributorAdapter) {}
    }

    #[test]
    fn distributor_part_round_trip() {
        let part = DistributorPart {
            mpn: "RC0805FR-0710KL".into(),
            manufacturer: "Yageo".into(),
            description: "Resistor".into(),
            datasheet_url: Some(url::Url::parse("https://example.com/ds.pdf").unwrap()),
            footprint_hint: Some("0805_2012Metric".into()),
            parameters: ParamMap::new(),
            pricing: None,
            stock: Some(50_000),
            source: DistributorSource::DigiKey,
            captured_at: chrono::Utc::now(),
            extra: Default::default(),
        };
        let json = serde_json::to_string(&part).unwrap();
        let back: DistributorPart = serde_json::from_str(&json).unwrap();
        assert_eq!(part, back);
    }
}
