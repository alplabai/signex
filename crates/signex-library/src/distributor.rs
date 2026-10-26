//! `DistributorAdapter` — vendor metadata + pricing lookup. v0.9-library-plan.md §14a.4.

use std::collections::BTreeMap;

use crate::param::ParamMap;

/// Price-break tier for one distributor — `qty @ unit_price`, in the
/// currency of the [`PricingSnapshot`] that carries it.
///
/// Pricing is a runtime/cache concept, not a static `Component` field. The
/// per-row cache lives in the `.snxlib` as [`crate::CachedPricing`], which
/// reuses this type for its breaks.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PriceBreak {
    pub qty: u32,
    /// Unit price at `qty`. Must be finite and non-negative to be cached.
    /// Legacy payloads called this `unit_price_usd`; they were always USD,
    /// which is the [`PricingSnapshot::currency`] default.
    #[serde(alias = "unit_price_usd")]
    pub unit_price: f64,
}

fn default_currency() -> String {
    "USD".to_string()
}

/// Captured pricing snapshot for a single MPN, partitioned by distributor.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PricingSnapshot {
    pub captured_at: chrono::DateTime<chrono::Utc>,
    /// ISO 4217 code of every `unit_price` in `by_distributor`.
    #[serde(default = "default_currency")]
    pub currency: String,
    /// Units in stock at capture time, when the distributor reports it.
    #[serde(default)]
    pub stock: Option<u32>,
    /// The distributor's own part number the prices belong to. A client that
    /// looked a part up by SKU or by MPN reports which one it actually
    /// priced, so a caller can refuse a price for another part.
    #[serde(default)]
    pub part_number: Option<String>,
    pub by_distributor: BTreeMap<String, Vec<PriceBreak>>,
}

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

    /// Whether [`Self::refresh_pricing`] can return price breaks. A client
    /// whose response parser does not read prices keeps the default `false`,
    /// so Refresh All Pricing neither offers nor runs a walk that would
    /// report "no price" for every row.
    fn supports_pricing(&self) -> bool {
        false
    }
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
