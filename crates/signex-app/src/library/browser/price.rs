//! Library Browser — the cached-price column.
//!
//! Reads the `OpenLibrary::pricing` cache; never the adapter (`view` is
//! pure). The price shown is the first of the row's distributor listings
//! that still has a cached price for its current SKU, so the user's
//! listing order is the preference order.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use signex_library::{ComponentRow, RowPricing, current_pricing};
use uuid::Uuid;

/// What the price column needs from the model.
pub(super) struct PriceView<'a> {
    pub(super) pricing: &'a HashMap<Uuid, RowPricing>,
    /// `LibraryState::clock`, so ages are computed without asking the OS.
    pub(super) now: DateTime<Utc>,
}

impl PriceView<'_> {
    /// Whether any of `rows` has a cached price worth a column.
    pub(super) fn any(&self, rows: &[ComponentRow]) -> bool {
        rows.iter().any(|r| self.headline(r).is_some())
    }

    fn headline(&self, row: &ComponentRow) -> Option<(String, String)> {
        let cached = self.pricing.get(&row.row_id)?;
        let (distributor, price) = current_pricing(row, cached).into_iter().next()?;
        Some((distributor.to_string(), price.summary(self.now)?))
    }

    /// `"0.0123 USD @1 · 3 d ago · DigiKey"`, or `—` without a price.
    pub(super) fn text(&self, row: &ComponentRow) -> String {
        match self.headline(row) {
            Some((distributor, summary)) => format!("{summary} \u{b7} {distributor}"),
            None => "\u{2014}".to_string(),
        }
    }

    /// The headline unit price as a bare number, so the grid's numeric-aware
    /// sort orders by price and not by the text above. Empty without one.
    pub(super) fn sort_value(&self, row: &ComponentRow) -> String {
        self.pricing
            .get(&row.row_id)
            .and_then(|cached| current_pricing(row, cached).into_iter().next())
            .and_then(|(_, price)| price.headline_break().map(|b| b.unit_price.to_string()))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::distributor::PriceBreak;
    use signex_library::{CachedPricing, DistributorListing};

    fn row(sku: &str) -> ComponentRow {
        let now = Utc::now();
        ComponentRow {
            row_id: Uuid::now_v7(),
            internal_pn: signex_library::InternalPn::new("R"),
            class: signex_library::ComponentClass::new("resistor"),
            datasheet: Default::default(),
            state: signex_library::LifecycleState::Draft,
            symbol_ref: signex_library::PrimitiveRef::new(Uuid::nil(), Uuid::nil()),
            footprint_ref: None,
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: signex_library::ManufacturerPart::draft("A", "B"),
            alternates: Vec::new(),
            supply: vec![DistributorListing::new("DigiKey", sku)],
            parameters: Default::default(),
            plm: Default::default(),
            version: "0.0.1".into(),
            released: false,
            symbol_version: String::new(),
            footprint_version: String::new(),
            sim_version: String::new(),
            created: now,
            updated: now,
            content_hash: [0; 32],
        }
    }

    fn cached(sku: &str, at: DateTime<Utc>) -> RowPricing {
        RowPricing::from([(
            "DigiKey".to_string(),
            CachedPricing {
                sku: sku.into(),
                currency: "USD".into(),
                stock: None,
                breaks: vec![
                    PriceBreak {
                        qty: 10,
                        unit_price: 0.08,
                    },
                    PriceBreak {
                        qty: 1,
                        unit_price: 0.1,
                    },
                ],
                fetched_at: at,
            },
        )])
    }

    #[test]
    fn shows_qty_one_price_currency_age_and_distributor() {
        let r = row("DK-1");
        let at = Utc::now();
        let pricing = HashMap::from([(r.row_id, cached("DK-1", at))]);
        let view = PriceView {
            pricing: &pricing,
            now: at + chrono::Duration::days(3),
        };
        assert_eq!(view.text(&r), "0.1 USD @1 \u{b7} 3 d ago \u{b7} DigiKey");
        assert_eq!(view.sort_value(&r), "0.1");
        assert!(view.any(std::slice::from_ref(&r)));
    }

    #[test]
    fn a_price_for_a_replaced_sku_is_not_shown() {
        let r = row("DK-2");
        let pricing = HashMap::from([(r.row_id, cached("DK-1", Utc::now()))]);
        let view = PriceView {
            pricing: &pricing,
            now: Utc::now(),
        };
        assert_eq!(view.text(&r), "\u{2014}");
        assert_eq!(view.sort_value(&r), "");
        assert!(
            !view.any(std::slice::from_ref(&r)),
            "no column without a current price"
        );
    }
}
