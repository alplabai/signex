//! Cached distributor pricing for a component row.
//!
//! A row already binds an MPN ([`crate::ManufacturerPart`]) and per-distributor
//! SKUs ([`crate::DistributorListing`]). This module adds the missing half:
//! the last price the distributor reported for that SKU, kept in the
//! `.snxlib` so the library browser and the BOM can show it without a network
//! round trip.
//!
//! The cache is stored as a per-table sidecar
//! (`[tables.<name>.pricing.<row_id>.<distributor>.<part number>]`),
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
    #[error("the distributor priced part {priced:?}, not the {asked:?} the row lists")]
    WrongPart { asked: String, priced: String },
    #[error("the distributor did not say which part it priced")]
    UnknownPart,
}

/// The last price one distributor reported for one SKU of one row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CachedPricing {
    /// Distributor part number the price belongs to: the SKU of the row's
    /// listing, or, for a listing without a SKU that was priced by MPN, the
    /// part number the distributor reported. A price whose part number the
    /// row's listings no longer include is stale: it is not shown, not
    /// stored, and is pruned on the next save.
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

/// Cached prices of one row: distributor name (`"DigiKey"`, …) then part
/// number. Two listings of one distributor (cut tape and reel) keep separate
/// records.
pub type RowPricing = BTreeMap<String, BTreeMap<String, CachedPricing>>;

/// Whether two distributor part numbers name the same part (trimmed,
/// case-insensitive).
pub fn same_part_number(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

impl CachedPricing {
    /// Build the cache record from what a client returned.
    ///
    /// `listing_sku` is the SKU the request named, if the row has one. The
    /// snapshot's `part_number` says which part the client actually priced:
    /// when both exist they must agree, and with no listing SKU the client's
    /// own part number is what gets recorded. A snapshot that names no part
    /// for a SKU-less listing is refused.
    ///
    /// Takes the breaks filed under `distributor` (the client's own name), or
    /// the only entry when the client filed them under another label. Refuses
    /// a snapshot without breaks, and any non-finite or negative price.
    pub fn from_snapshot(
        distributor: &str,
        listing_sku: Option<&str>,
        snapshot: &PricingSnapshot,
    ) -> Result<Self, PricingError> {
        let priced = snapshot
            .part_number
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty());
        let sku = match (listing_sku.map(str::trim).filter(|s| !s.is_empty()), priced) {
            (Some(asked), Some(priced)) if !same_part_number(asked, priced) => {
                return Err(PricingError::WrongPart {
                    asked: asked.to_string(),
                    priced: priced.to_string(),
                });
            }
            (Some(asked), _) => asked.to_string(),
            (None, Some(priced)) => priced.to_string(),
            (None, None) => return Err(PricingError::UnknownPart),
        };
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
            sku,
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

/// Whether a cached price of `distributor` for part number `sku` still
/// describes `row`: the row lists that distributor with the same SKU, or
/// lists it with no SKU at all (priced by MPN).
pub fn record_is_current(
    row: &crate::component::ComponentRow,
    distributor: &str,
    sku: &str,
) -> bool {
    let wanted = distributor_key(distributor);
    row.supply.iter().any(|l| {
        distributor_key(&l.distributor) == wanted
            && (l.sku.trim().is_empty() || same_part_number(&l.sku, sku))
    })
}

/// Whether a record of `distributor` for `sku` exists only because the row
/// has a listing of that distributor without a SKU (priced by MPN): no
/// listing of it names this SKU.
fn is_sku_less_record(row: &crate::component::ComponentRow, distributor: &str, sku: &str) -> bool {
    let wanted = distributor_key(distributor);
    let mut sku_less = false;
    for l in &row.supply {
        if distributor_key(&l.distributor) != wanted {
            continue;
        }
        if l.sku.trim().is_empty() {
            sku_less = true;
        } else if same_part_number(&l.sku, sku) {
            return false;
        }
    }
    sku_less
}

/// The newest of `records` (ties broken by part number, so the pick is
/// stable).
fn newest<'a>(records: impl Iterator<Item = &'a CachedPricing>) -> Option<&'a CachedPricing> {
    records.max_by(|a, b| {
        a.fetched_at
            .cmp(&b.fetched_at)
            .then_with(|| a.sku.cmp(&b.sku))
    })
}

/// The cached prices that still describe `row`, in the order the row lists
/// its distributors (the user's preference), as `(distributor, price)`. A
/// listing with a SKU shows that SKU's price; a listing without one shows the
/// newest price of its distributor that no SKU listing claims. A price whose
/// distributor or part number the row no longer lists is left out.
pub fn current_pricing<'a>(
    row: &crate::component::ComponentRow,
    cached: &'a RowPricing,
) -> Vec<(&'a str, &'a CachedPricing)> {
    let mut out: Vec<(&str, &CachedPricing)> = Vec::new();
    for listing in &row.supply {
        let wanted = distributor_key(&listing.distributor);
        for (name, per_sku) in cached {
            if distributor_key(name) != wanted {
                continue;
            }
            let pick = if listing.sku.trim().is_empty() {
                newest(
                    per_sku
                        .values()
                        .filter(|p| is_sku_less_record(row, name, &p.sku)),
                )
            } else {
                per_sku
                    .values()
                    .find(|p| same_part_number(&listing.sku, &p.sku))
            };
            if let Some(price) = pick
                && !out
                    .iter()
                    .any(|(n, p)| *n == name && same_part_number(&p.sku, &price.sku))
            {
                out.push((name.as_str(), price));
            }
        }
    }
    out
}

/// Drop the records of `cached` that no longer describe `row`, and keep only
/// the newest record a SKU-less listing accounts for; returns whether
/// anything was removed.
pub fn prune_row_pricing(row: &crate::component::ComponentRow, cached: &mut RowPricing) -> bool {
    let before: usize = cached.values().map(BTreeMap::len).sum();
    for (name, per_sku) in cached.iter_mut() {
        per_sku.retain(|_, price| record_is_current(row, name, &price.sku));
        let keep = newest(
            per_sku
                .values()
                .filter(|p| is_sku_less_record(row, name, &p.sku)),
        )
        .map(|p| p.sku.clone());
        per_sku.retain(|sku, price| {
            !is_sku_less_record(row, name, &price.sku) || keep.as_deref() == Some(sku.as_str())
        });
    }
    cached.retain(|_, per_sku| !per_sku.is_empty());
    let after: usize = cached.values().map(BTreeMap::len).sum();
    after != before
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
            part_number: None,
            by_distributor: BTreeMap::from([("Mouser".to_string(), breaks)]),
        }
    }

    #[test]
    fn from_snapshot_sorts_and_normalises() {
        let c = CachedPricing::from_snapshot(
            "Mouser",
            Some(" 123 "),
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
                Some("1"),
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
            CachedPricing::from_snapshot("Mouser", Some("1"), &snap(vec![])).unwrap_err(),
            PricingError::NoBreaks
        );
        let mut s = snap(vec![PriceBreak {
            qty: 1,
            unit_price: 1.0,
        }]);
        s.currency = "dollars".into();
        assert!(matches!(
            CachedPricing::from_snapshot("Mouser", Some("1"), &s).unwrap_err(),
            PricingError::BadCurrency(_)
        ));
    }

    #[test]
    fn summary_and_age_labels() {
        let mut c = CachedPricing::from_snapshot(
            "Mouser",
            Some("1"),
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
            alternate_symbols: Vec::new(),
            row_id: Uuid::nil(),
            internal_pn: InternalPn::new("R"),
            class: ComponentClass::new("resistor"),
            datasheet: DatasheetRef::default(),
            datasheet_label: String::new(),
            more_datasheets: Vec::new(),
            state: LifecycleState::Draft,
            symbol_ref: PrimitiveRef::new(Uuid::nil(), Uuid::nil()),
            footprint_ref: None,
            alternate_footprints: Vec::new(),
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
            server_revision: None,
        };
        let cached = RowPricing::from([(
            "DigiKey".to_string(),
            BTreeMap::from([("DK-1".to_string(), price("DK-1", now))]),
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

    fn price(sku: &str, at: DateTime<Utc>) -> CachedPricing {
        CachedPricing {
            sku: sku.into(),
            currency: "USD".into(),
            stock: None,
            breaks: vec![PriceBreak {
                qty: 1,
                unit_price: 1.0,
            }],
            fetched_at: at,
        }
    }

    fn row_listing(listings: &[(&str, &str)]) -> crate::component::ComponentRow {
        use crate::component::{ComponentRow, DatasheetRef, PlmReserved};
        use crate::identity::{ComponentClass, InternalPn};
        use crate::lifecycle::LifecycleState;
        use crate::manufacturer::{DistributorListing, ManufacturerPart};
        use crate::param::ParamMap;
        use crate::primitive::PrimitiveRef;
        let now = Utc::now();
        ComponentRow {
            alternate_symbols: Vec::new(),
            row_id: Uuid::nil(),
            internal_pn: InternalPn::new("R"),
            class: ComponentClass::new("resistor"),
            datasheet: DatasheetRef::default(),
            datasheet_label: String::new(),
            more_datasheets: Vec::new(),
            state: LifecycleState::Draft,
            symbol_ref: PrimitiveRef::new(Uuid::nil(), Uuid::nil()),
            footprint_ref: None,
            alternate_footprints: Vec::new(),
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("A", "B"),
            alternates: Vec::new(),
            supply: listings
                .iter()
                .map(|(d, s)| DistributorListing::new(*d, *s))
                .collect(),
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
            server_revision: None,
        }
    }

    #[test]
    fn a_snapshot_for_another_part_is_refused() {
        let mut s = snap(vec![PriceBreak {
            qty: 1,
            unit_price: 1.0,
        }]);
        s.part_number = Some("603-OTHER".into());
        assert!(matches!(
            CachedPricing::from_snapshot("Mouser", Some("603-ASKED"), &s).unwrap_err(),
            PricingError::WrongPart { .. }
        ));
        assert!(
            CachedPricing::from_snapshot("Mouser", Some("603-other"), &s).is_ok(),
            "case-insensitive"
        );
    }

    #[test]
    fn a_sku_less_listing_records_the_part_the_distributor_priced() {
        let mut s = snap(vec![PriceBreak {
            qty: 1,
            unit_price: 1.0,
        }]);
        assert_eq!(
            CachedPricing::from_snapshot("Mouser", None, &s).unwrap_err(),
            PricingError::UnknownPart
        );
        s.part_number = Some("603-PRICED".into());
        assert_eq!(
            CachedPricing::from_snapshot("Mouser", None, &s)
                .unwrap()
                .sku,
            "603-PRICED"
        );
    }

    #[test]
    fn two_listings_of_one_distributor_keep_separate_records() {
        let row = row_listing(&[("DigiKey", "CT-ND"), ("DigiKey", "TR-ND")]);
        let now = Utc::now();
        let mut cached = RowPricing::new();
        for sku in ["CT-ND", "TR-ND"] {
            cached
                .entry("DigiKey".into())
                .or_default()
                .insert(sku.into(), price(sku, now));
        }
        let skus: Vec<&str> = current_pricing(&row, &cached)
            .iter()
            .map(|(_, p)| p.sku.as_str())
            .collect();
        assert_eq!(skus, ["CT-ND", "TR-ND"]);
    }

    #[test]
    fn prune_drops_records_the_row_no_longer_lists() {
        let row = row_listing(&[("DigiKey", "CT-ND")]);
        let now = Utc::now();
        let mut cached = RowPricing::new();
        let per = cached.entry("DigiKey".into()).or_default();
        per.insert("CT-ND".into(), price("CT-ND", now));
        per.insert("OLD-ND".into(), price("OLD-ND", now));
        cached
            .entry("Mouser".into())
            .or_default()
            .insert("M".into(), price("M", now));
        assert!(prune_row_pricing(&row, &mut cached));
        assert_eq!(cached.len(), 1);
        assert_eq!(cached["DigiKey"].keys().collect::<Vec<_>>(), ["CT-ND"]);
        assert!(
            !prune_row_pricing(&row, &mut cached),
            "second prune is a no-op"
        );
    }

    #[test]
    fn a_sku_less_listing_keeps_whatever_part_it_was_priced_as() {
        let row = row_listing(&[("Mouser", "")]);
        assert!(record_is_current(&row, "mouser", "603-ANY"));
        assert!(!record_is_current(&row, "DigiKey", "603-ANY"));
    }

    #[test]
    fn a_sku_less_listing_shows_and_keeps_only_its_newest_record() {
        let row = row_listing(&[("Mouser", "")]);
        let t0 = Utc::now();
        let mut cached = RowPricing::new();
        let per = cached.entry("Mouser".into()).or_default();
        per.insert("603-A".into(), price("603-A", t0));
        per.insert("603-Z".into(), price("603-Z", t0 + Duration::hours(1)));
        per.insert("603-M".into(), price("603-M", t0 - Duration::hours(1)));
        let shown = current_pricing(&row, &cached);
        assert_eq!(shown.len(), 1);
        assert_eq!(
            shown[0].1.sku, "603-Z",
            "the newest, not the alphabetically first"
        );
        assert!(prune_row_pricing(&row, &mut cached));
        assert_eq!(cached["Mouser"].keys().collect::<Vec<_>>(), ["603-Z"]);
    }

    #[test]
    fn a_sku_less_listing_does_not_steal_a_sku_listings_record() {
        let row = row_listing(&[("Mouser", "603-A"), ("Mouser", "")]);
        let t0 = Utc::now();
        let mut cached = RowPricing::new();
        let per = cached.entry("Mouser".into()).or_default();
        per.insert("603-A".into(), price("603-A", t0));
        per.insert("603-B".into(), price("603-B", t0 + Duration::hours(1)));
        per.insert("603-C".into(), price("603-C", t0 + Duration::hours(2)));
        prune_row_pricing(&row, &mut cached);
        assert_eq!(
            cached["Mouser"].keys().collect::<Vec<_>>(),
            ["603-A", "603-C"]
        );
    }
}
