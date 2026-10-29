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
    /// The currency most rows are priced in (ties: alphabetically first).
    /// Sorting compares prices only within it.
    main_currency: std::borrow::Cow<'a, str>,
}

impl<'a> PriceView<'a> {
    pub(super) fn new(
        pricing: &'a HashMap<Uuid, RowPricing>,
        now: DateTime<Utc>,
        rows: &[ComponentRow],
    ) -> Self {
        let mut count: std::collections::BTreeMap<String, usize> = Default::default();
        for row in rows {
            if let Some(cached) = pricing.get(&row.row_id)
                && let Some((_, price)) = current_pricing(row, cached).into_iter().next()
            {
                *count.entry(price.currency.clone()).or_default() += 1;
            }
        }
        // BTreeMap iterates alphabetically, so `max_by_key` keeps the last
        // maximum; reverse to make ties resolve to the alphabetically first.
        let main_currency = count
            .into_iter()
            .rev()
            .max_by_key(|(_, n)| *n)
            .map(|(c, _)| c)
            .unwrap_or_default();
        Self {
            pricing,
            now,
            main_currency: main_currency.into(),
        }
    }

    /// A view over `pricing` whose main currency was already worked out
    /// (by an earlier [`Self::new`]), so no pass over the rows is repeated.
    pub(super) fn with_main_currency(
        pricing: &'a HashMap<Uuid, RowPricing>,
        now: DateTime<Utc>,
        main_currency: &'a str,
    ) -> Self {
        Self {
            pricing,
            now,
            main_currency: main_currency.into(),
        }
    }

    /// The currency most rows are priced in.
    pub(super) fn main_currency(&self) -> &str {
        &self.main_currency
    }

    /// Whether any of `rows` has a cached price worth a column.
    pub(super) fn any(&self, rows: &[ComponentRow]) -> bool {
        rows.iter().any(|r| self.headline(r).is_some())
    }

    fn first_price(&self, row: &ComponentRow) -> Option<(&str, &signex_library::CachedPricing)> {
        let cached = self.pricing.get(&row.row_id)?;
        current_pricing(row, cached).into_iter().next()
    }

    fn headline(&self, row: &ComponentRow) -> Option<(String, String)> {
        let (distributor, price) = self.first_price(row)?;
        Some((distributor.to_string(), price.summary(self.now)?))
    }

    /// `"0.0123 USD @1 · 3 d ago · DigiKey"`, or `—` without a price.
    pub(super) fn text(&self, row: &ComponentRow) -> String {
        match self.headline(row) {
            Some((distributor, summary)) => format!("{summary} \u{b7} {distributor}"),
            None => "\u{2014}".to_string(),
        }
    }

    /// The sort cell. Prices in the main currency are bare numbers, which
    /// the grid's numeric-aware sort orders by value and ranks first. Other
    /// currencies follow, grouped by currency code and ordered by price
    /// within it (a zero-padded text key), so a EUR 5 never lands between
    /// USD 4 and USD 6. Unpriced rows come last (`~` sorts after letters).
    pub(super) fn sort_value(&self, row: &ComponentRow) -> String {
        let Some((_, price)) = self.first_price(row) else {
            return "~".to_string();
        };
        let Some(headline) = price.headline_break() else {
            return "~".to_string();
        };
        if price.currency == self.main_currency {
            headline.unit_price.to_string()
        } else {
            format!("{} {:020.8}", price.currency, headline.unit_price)
        }
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
            alternate_symbols: Vec::new(),
            row_id: Uuid::now_v7(),
            internal_pn: signex_library::InternalPn::new("R"),
            class: signex_library::ComponentClass::new("resistor"),
            datasheet: Default::default(),
            datasheet_label: String::new(),
            description: String::new(),
            more_datasheets: Vec::new(),
            state: signex_library::LifecycleState::Draft,
            symbol_ref: signex_library::PrimitiveRef::new(Uuid::nil(), Uuid::nil()),
            footprint_ref: None,
            alternate_footprints: Vec::new(),
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
            server_revision: None,
        }
    }

    fn cached(sku: &str, at: DateTime<Utc>) -> RowPricing {
        RowPricing::from([(
            "DigiKey".to_string(),
            std::collections::BTreeMap::from([(
                sku.to_string(),
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
            )]),
        )])
    }

    #[test]
    fn shows_qty_one_price_currency_age_and_distributor() {
        let r = row("DK-1");
        let at = Utc::now();
        let pricing = HashMap::from([(r.row_id, cached("DK-1", at))]);
        let view = PriceView::new(
            &pricing,
            at + chrono::Duration::days(3),
            std::slice::from_ref(&r),
        );
        assert_eq!(view.text(&r), "0.1 USD @1 \u{b7} 3 d ago \u{b7} DigiKey");
        assert_eq!(view.sort_value(&r), "0.1");
        assert!(view.any(std::slice::from_ref(&r)));
    }

    #[test]
    fn a_price_for_a_replaced_sku_is_not_shown() {
        let r = row("DK-2");
        let pricing = HashMap::from([(r.row_id, cached("DK-1", Utc::now()))]);
        let view = PriceView::new(&pricing, Utc::now(), std::slice::from_ref(&r));
        assert_eq!(view.text(&r), "\u{2014}");
        assert_eq!(view.sort_value(&r), "~");
        assert!(
            !view.any(std::slice::from_ref(&r)),
            "no column without a current price"
        );
    }

    fn priced_row(sku: &str, currency: &str, unit_price: f64) -> (ComponentRow, RowPricing) {
        let r = row(sku);
        let mut c = cached(sku, Utc::now());
        let price = c.get_mut("DigiKey").unwrap().get_mut(sku).unwrap();
        price.currency = currency.into();
        price.breaks = vec![PriceBreak { qty: 1, unit_price }];
        (r, c)
    }

    #[test]
    fn sort_keeps_currencies_apart_and_unpriced_last() {
        let (usd_lo, c1) = priced_row("A", "USD", 4.0);
        let (usd_hi, c2) = priced_row("B", "USD", 6.0);
        let (usd_mid, c3) = priced_row("C", "USD", 5.5);
        let (eur, c4) = priced_row("D", "EUR", 5.0);
        let none = row("E");
        let mut pricing = HashMap::new();
        for (r, c) in [(&usd_lo, c1), (&usd_hi, c2), (&usd_mid, c3), (&eur, c4)] {
            pricing.insert(r.row_id, c);
        }
        let rows = [
            usd_lo.clone(),
            usd_hi.clone(),
            usd_mid.clone(),
            eur.clone(),
            none.clone(),
        ];
        let view = PriceView::new(&pricing, Utc::now(), &rows);
        // The grid's own comparator, fed the column's own cells.
        let mut order: Vec<&ComponentRow> = rows.iter().collect();
        super::super::sort::sort_by_cell_key(
            &mut order,
            |r| super::super::sort::CellSortKey::from_cell(&view.sort_value(r)),
            false,
        );
        let skus: Vec<&str> = order.iter().map(|r| r.supply[0].sku.as_str()).collect();
        assert_eq!(
            skus,
            ["A", "C", "B", "D", "E"],
            "USD by price, then EUR, then unpriced"
        );
    }
}
