//! Cached distributor pricing for a component row.
//!
//! A row already binds an MPN ([`crate::ManufacturerPart`]) and per-distributor
//! SKUs ([`crate::DistributorListing`]). This module adds the missing half:
//! the last price the distributor reported for that SKU, kept in the
//! `.snxlib` so the library browser and the BOM can show it without a network
//! round trip.
//!
//! The cache is stored as a per-table sidecar (`[tables.<name>.pricing.<row_id>.<distributor>]`),
//! not as TSV columns: the component TSV header is a fixed schema, so a new
//! column would change every existing file. See `docs/architecture/snxlib-pricing.md`.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::distributor::{PriceBreak, PricingSnapshot};

/// Why a price record cannot be cached.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PricingError {
    #[error("no price breaks")]
    NoBreaks,
    #[error("price break at qty {qty} is {price} (must be finite and not negative)")]
    BadPrice { qty: u32, price: String },
    #[error("price break quantity must be at least 1")]
    ZeroQuantity,
    #[error("currency {0:?} is not a three-letter ISO 4217 code")]
    BadCurrency(String),
    #[error("SKU is empty")]
    EmptySku,
}

/// The last price one distributor reported for one SKU of one row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CachedPricing {
    /// Distributor SKU the price was fetched for. A price whose SKU no longer
    /// matches the row's listing is stale and is neither shown nor kept.
    pub sku: String,
    /// ISO 4217 code of every `unit_price` in `breaks`.
    pub currency: String,
    /// Units in stock at fetch time, when the distributor reports it.
    #[serde(default)]
    pub stock: Option<u32>,
    /// Price breaks, ascending by quantity.
    pub breaks: Vec<PriceBreak>,
    /// When the distributor was asked (UTC).
    pub fetched_at: DateTime<Utc>,
}

/// Cached prices of one row, keyed by distributor name (`"DigiKey"`, …).
pub type RowPricing = BTreeMap<String, CachedPricing>;

impl CachedPricing {
    /// Build the cache record from what a client returned for `sku`.
    ///
    /// Takes the breaks filed under `distributor` (the client's own name), or
    /// the only entry when the client filed them under another label. Refuses
    /// a snapshot without breaks, and any non-finite or negative price.
    pub fn from_snapshot(
        distributor: &str,
        sku: &str,
        snapshot: &PricingSnapshot,
    ) -> Result<Self, PricingError> {
        let breaks = snapshot
            .by_distributor
            .get(distributor)
            .or_else(|| {
                (snapshot.by_distributor.len() == 1)
                    .then(|| snapshot.by_distributor.values().next())
                    .flatten()
            })
            .cloned()
            .unwrap_or_default();
        let mut cached = Self {
            sku: sku.trim().to_string(),
            currency: snapshot.currency.trim().to_ascii_uppercase(),
            stock: snapshot.stock,
            breaks,
            fetched_at: snapshot.captured_at,
        };
        cached.breaks.sort_by_key(|b| b.qty);
        cached.validate()?;
        Ok(cached)
    }

    /// The finite-number rule for stored prices: every price is finite and
    /// not negative, quantities start at 1, the currency is a three-letter
    /// code. Run before a record is written and after one is read.
    pub fn validate(&self) -> Result<(), PricingError> {
        if self.sku.is_empty() {
            return Err(PricingError::EmptySku);
        }
        if self.currency.len() != 3 || !self.currency.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err(PricingError::BadCurrency(self.currency.clone()));
        }
        if self.breaks.is_empty() {
            return Err(PricingError::NoBreaks);
        }
        for b in &self.breaks {
            if b.qty == 0 {
                return Err(PricingError::ZeroQuantity);
            }
            if !b.unit_price.is_finite() || b.unit_price < 0.0 {
                return Err(PricingError::BadPrice {
                    qty: b.qty,
                    price: b.unit_price.to_string(),
                });
            }
        }
        Ok(())
    }

    /// The cheapest-quantity break: the one a single-unit buyer sees. This is
    /// the qty-1 break, or the smallest orderable quantity above it.
    pub fn headline_break(&self) -> Option<&PriceBreak> {
        self.breaks.iter().min_by_key(|b| b.qty)
    }

    /// Unit price when buying `qty` pieces: the break with the largest
    /// quantity not above `qty`. `None` when `qty` is below the first break.
    pub fn unit_price_at(&self, qty: u32) -> Option<f64> {
        self.breaks
            .iter()
            .filter(|b| b.qty <= qty)
            .max_by_key(|b| b.qty)
            .map(|b| b.unit_price)
    }

    /// Time since the fetch; zero if the clock went backwards.
    pub fn age(&self, now: DateTime<Utc>) -> Duration {
        (now - self.fetched_at).max(Duration::zero())
    }

    /// `"0.0123 USD @1 · 3 d ago"`, the one-line form the browser shows.
    pub fn summary(&self, now: DateTime<Utc>) -> Option<String> {
        let b = self.headline_break()?;
        Some(format!(
            "{} {} @{} \u{b7} {}",
            format_price(b.unit_price),
            self.currency,
            b.qty,
            age_label(self.age(now)),
        ))
    }

    /// RFC 3339 form written to the `.snxlib`; round-trips exactly.
    pub(crate) fn fetched_at_text(&self) -> String {
        self.fetched_at.to_rfc3339_opts(SecondsFormat::AutoSi, true)
    }
}

/// Price text without trailing noise (`0.1`, not `0.10000000000000001`).
pub fn format_price(price: f64) -> String {
    format!("{price}")
}

/// `"just now"`, `"5 min ago"`, `"3 h ago"`, `"2 d ago"`.
pub fn age_label(age: Duration) -> String {
    let mins = age.num_minutes();
    if mins < 1 {
        "just now".to_string()
    } else if mins < 60 {
        format!("{mins} min ago")
    } else if age.num_hours() < 48 {
        format!("{} h ago", age.num_hours())
    } else {
        format!("{} d ago", age.num_days())
    }
}

/// Normalised distributor identity for matching a row's free-text
/// `DistributorListing::distributor` ("Digi-Key", "digikey ") against a
/// client's name: lowercase alphanumerics only.
pub fn distributor_key(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The cached prices that still describe `row`, in the order the row lists
/// its distributors (the user's preference). A cached price whose distributor
/// or SKU the row no longer lists is left out.
pub fn current_pricing<'a>(
    row: &crate::component::ComponentRow,
    cached: &'a RowPricing,
) -> Vec<(&'a str, &'a CachedPricing)> {
    let mut out: Vec<(&str, &CachedPricing)> = Vec::new();
    for listing in &row.supply {
        let wanted = distributor_key(&listing.distributor);
        let hit = cached
            .iter()
            .find(|(name, c)| distributor_key(name) == wanted && c.sku == listing.sku.trim());
        if let Some((name, c)) = hit
            && !out.iter().any(|(n, _)| n == name)
        {
            out.push((name.as_str(), c));
        }
    }
    out
}

/// One price to write back: which row, which distributor, what was fetched.
#[derive(Clone, Debug, PartialEq)]
pub struct PricingUpdate {
    pub table: String,
    pub row_id: Uuid,
    /// Key in the row's [`RowPricing`]; the client's own name.
    pub distributor: String,
    pub pricing: CachedPricing,
}

/// A fetched price the library refused to store, with the reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DroppedUpdate {
    pub table: String,
    pub row_id: Uuid,
    pub distributor: String,
    pub reason: String,
}

/// Result of [`crate::LibraryAdapter::store_pricing`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StoreOutcome {
    pub stored: usize,
    /// Updates the row no longer supports: deleted, or its SKU changed while
    /// the fetch was in flight.
    pub dropped: Vec<DroppedUpdate>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(breaks: Vec<PriceBreak>) -> PricingSnapshot {
        PricingSnapshot {
            captured_at: Utc::now(),
            currency: "usd".into(),
            stock: Some(10),
            by_distributor: BTreeMap::from([("Mouser".to_string(), breaks)]),
        }
    }

    #[test]
    fn from_snapshot_sorts_and_normalises() {
        let c = CachedPricing::from_snapshot(
            "Mouser",
            " 123 ",
            &snap(vec![
                PriceBreak {
                    qty: 100,
                    unit_price: 0.05,
                },
                PriceBreak {
                    qty: 1,
                    unit_price: 0.1,
                },
            ]),
        )
        .unwrap();
        assert_eq!(c.sku, "123");
        assert_eq!(c.currency, "USD");
        assert_eq!(c.breaks[0].qty, 1);
        assert_eq!(c.headline_break().unwrap().unit_price, 0.1);
        assert_eq!(c.unit_price_at(250), Some(0.05));
        assert_eq!(c.unit_price_at(0), None);
    }

    #[test]
    fn non_finite_and_negative_prices_are_refused() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.01] {
            let err = CachedPricing::from_snapshot(
                "Mouser",
                "1",
                &snap(vec![PriceBreak {
                    qty: 1,
                    unit_price: bad,
                }]),
            )
            .unwrap_err();
            assert!(matches!(err, PricingError::BadPrice { .. }), "{bad}: {err}");
        }
    }

    #[test]
    fn empty_breaks_and_bad_currency_are_refused() {
        assert_eq!(
            CachedPricing::from_snapshot("Mouser", "1", &snap(vec![])).unwrap_err(),
            PricingError::NoBreaks
        );
        let mut s = snap(vec![PriceBreak {
            qty: 1,
            unit_price: 1.0,
        }]);
        s.currency = "dollars".into();
        assert!(matches!(
            CachedPricing::from_snapshot("Mouser", "1", &s).unwrap_err(),
            PricingError::BadCurrency(_)
        ));
    }

    #[test]
    fn summary_and_age_labels() {
        let mut c = CachedPricing::from_snapshot(
            "Mouser",
            "1",
            &snap(vec![PriceBreak {
                qty: 1,
                unit_price: 0.0123,
            }]),
        )
        .unwrap();
        let now = c.fetched_at + Duration::days(3);
        assert_eq!(c.summary(now).unwrap(), "0.0123 USD @1 \u{b7} 3 d ago");
        c.fetched_at = now;
        assert!(c.summary(now).unwrap().ends_with("just now"));
    }

    #[test]
    fn current_pricing_follows_the_rows_listings() {
        use crate::component::{ComponentRow, DatasheetRef, PlmReserved};
        use crate::identity::{ComponentClass, InternalPn};
        use crate::lifecycle::LifecycleState;
        use crate::manufacturer::{DistributorListing, ManufacturerPart};
        use crate::param::ParamMap;
        use crate::primitive::PrimitiveRef;
        let now = Utc::now();
        let mut row = ComponentRow {
            row_id: Uuid::nil(),
            internal_pn: InternalPn::new("R"),
            class: ComponentClass::new("resistor"),
            datasheet: DatasheetRef::default(),
            state: LifecycleState::Draft,
            symbol_ref: PrimitiveRef::new(Uuid::nil(), Uuid::nil()),
            footprint_ref: None,
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("A", "B"),
            alternates: Vec::new(),
            supply: vec![DistributorListing::new("Digi-Key", "DK-1")],
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            version: "0.0.1".into(),
            released: false,
            symbol_version: String::new(),
            footprint_version: String::new(),
            sim_version: String::new(),
            created: now,
            updated: now,
            content_hash: [0; 32],
        };
        let cached = RowPricing::from([(
            "DigiKey".to_string(),
            CachedPricing {
                sku: "DK-1".into(),
                currency: "USD".into(),
                stock: None,
                breaks: vec![PriceBreak {
                    qty: 1,
                    unit_price: 1.0,
                }],
                fetched_at: now,
            },
        )]);
        assert_eq!(current_pricing(&row, &cached).len(), 1);
        row.supply[0].sku = "DK-2".into();
        assert!(current_pricing(&row, &cached).is_empty(), "SKU changed");
    }

    #[test]
    fn distributor_key_ignores_case_and_punctuation() {
        assert_eq!(distributor_key("Digi-Key "), distributor_key("DIGIKEY"));
        assert_ne!(distributor_key("LCSC"), distributor_key("JLCPCB"));
    }
}
