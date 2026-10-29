//! Cached distributor prices on BOM rows (#1023).
//!
//! The BOM knows nothing about libraries. The app fills a [`PriceBook`] from
//! the prices its open libraries already cached (the `.snxlib` pricing
//! sidecar, #152), keyed by the placed symbol's library row and, as a
//! fallback, by MPN. [`apply_prices`] then prices each row at its quantity
//! after grouping, so a grouped line is priced at the quantity bought.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::{BomRow, BomTable};

/// One distributor's cached offer for a part.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceQuote {
    pub distributor: String,
    /// ISO 4217 code of every price in `breaks`.
    pub currency: String,
    pub stock: Option<u32>,
    /// `(quantity, unit price)`, ascending by quantity.
    pub breaks: Vec<(u32, f64)>,
    /// How old the cached price is, as the library shows it ("3 d ago").
    pub age: String,
}

impl PriceQuote {
    /// The minimum order: the smallest break quantity.
    pub fn minimum_order(&self) -> Option<u32> {
        self.breaks.iter().map(|(at, _)| *at).min()
    }

    /// Unit price when buying `qty`: the break with the largest quantity at
    /// or below `qty`, in whatever order the breaks are stored. Below the
    /// minimum order, the minimum order's price. `None` without breaks.
    ///
    /// Unlike `signex_library::CachedPricing::unit_price_at`, which returns
    /// `None` below the first break, this falls back to the minimum order:
    /// the BOM always buys at least that many.
    pub fn unit_price_at(&self, qty: u32) -> Option<f64> {
        self.breaks
            .iter()
            .filter(|(at, _)| *at <= qty)
            .max_by_key(|(at, _)| *at)
            .or_else(|| self.breaks.iter().min_by_key(|(at, _)| *at))
            .map(|(_, price)| *price)
    }
}

/// Cached quotes for the parts of a design, each list in preference order
/// (the first is the one priced).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PriceBook {
    /// By library row: [`library_row_key`] of the placed symbol's row.
    pub by_row: HashMap<String, Vec<PriceQuote>>,
    /// By MPN, normalised with [`mpn_key`], for parts placed without a
    /// library row link.
    pub by_mpn: HashMap<String, Vec<PriceQuote>>,
}

impl PriceBook {
    pub fn is_empty(&self) -> bool {
        self.by_row.is_empty() && self.by_mpn.is_empty()
    }

    /// The quote a row is priced from: its library row's first quote, else
    /// the first quote for its MPN.
    pub fn quote_for(&self, row: &BomRow) -> Option<&PriceQuote> {
        let by_row = row
            .library_row
            .as_ref()
            .and_then(|key| self.by_row.get(key))
            .and_then(|quotes| quotes.first());
        by_row.or_else(|| {
            row_mpn(row)
                .and_then(|mpn| self.by_mpn.get(&mpn_key(mpn)))
                .and_then(|quotes| quotes.first())
        })
    }
}

/// The key a library row is found under: `<library id>/<row id>`.
pub fn library_row_key(library_id: &str, row_id: &str) -> String {
    format!("{library_id}/{row_id}")
}

/// An MPN as it is looked up: trimmed and upper-cased.
pub fn mpn_key(mpn: &str) -> String {
    mpn.trim().to_uppercase()
}

pub use signex_types::schematic::MPN_FIELDS;

fn row_mpn(row: &BomRow) -> Option<&str> {
    MPN_FIELDS.iter().find_map(|wanted| {
        row.custom
            .iter()
            .find(|(key, value)| key.eq_ignore_ascii_case(wanted) && !value.trim().is_empty())
            .map(|(_, value)| value.as_str())
    })
}

/// A BOM row's price, at the quantity the row buys.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BomPrice {
    pub distributor: String,
    pub currency: String,
    /// Unit price at [`Self::buy_qty`].
    pub unit_price: f64,
    /// What the row costs: `unit_price` × [`Self::buy_qty`], rounded to
    /// cents ([`round_cents`]).
    pub extended_price: f64,
    /// The quantity priced: the fitted quantity, raised to the distributor's
    /// minimum order when it is below it (you pay for the minimum order).
    /// Zero for a line with no fitted part.
    pub buy_qty: u32,
    pub stock: Option<u32>,
    pub age: String,
}

/// Price every row of `table` from `book`; a row with no cached price for
/// its row or MPN keeps `price: None`. A row is
/// priced at its fitted quantity, raised to the minimum order when below it;
/// a row with none fitted gets the unit price at the minimum order and costs
/// nothing.
pub fn apply_prices(table: &mut BomTable, book: &PriceBook) {
    for row in &mut table.rows {
        row.price = book.quote_for(row).and_then(|quote| {
            let minimum = quote.minimum_order()?.max(1);
            let buy_qty = if row.fitted_qty == 0 {
                0
            } else {
                row.fitted_qty.max(minimum)
            };
            let unit_price = quote.unit_price_at(buy_qty.max(minimum))?;
            Some(BomPrice {
                distributor: quote.distributor.clone(),
                currency: quote.currency.clone(),
                unit_price,
                extended_price: round_cents(unit_price * f64::from(buy_qty)),
                buy_qty,
                stock: quote.stock,
                age: quote.age.clone(),
            })
        });
    }
}

/// `value` rounded to cents, halves away from zero. Extended prices are
/// rounded per line and the totals sum the rounded lines, so a total
/// always equals the sum of the lines printed above it.
pub fn round_cents(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// The BOM's total cost, one sum per currency (a BOM priced from several
/// distributors may mix them; they are not converted).
pub fn price_totals(table: &BomTable) -> BTreeMap<String, f64> {
    let mut totals = BTreeMap::new();
    for price in table.rows.iter().filter_map(|row| row.price.as_ref()) {
        *totals.entry(price.currency.clone()).or_insert(0.0) += price.extended_price;
    }
    // The lines are whole cents already; this only drops float drift.
    totals
        .values_mut()
        .for_each(|total| *total = round_cents(*total));
    totals
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quote(distributor: &str, currency: &str, breaks: &[(u32, f64)]) -> PriceQuote {
        PriceQuote {
            distributor: distributor.into(),
            currency: currency.into(),
            stock: Some(500),
            breaks: breaks.to_vec(),
            age: "2 d ago".into(),
        }
    }

    fn row(reference: &str, fitted: u32, library_row: Option<&str>, mpn: Option<&str>) -> BomRow {
        BomRow {
            references: vec![reference.into()],
            qty: fitted,
            fitted_qty: fitted,
            library_row: library_row.map(str::to_string),
            custom: mpn
                .map(|mpn| BTreeMap::from([("MPN".to_string(), mpn.to_string())]))
                .unwrap_or_default(),
            ..Default::default()
        }
    }

    #[test]
    fn the_unit_price_is_the_largest_break_at_or_below_the_quantity() {
        let q = quote("D", "USD", &[(1, 0.10), (10, 0.08), (100, 0.05)]);
        assert_eq!(q.unit_price_at(1), Some(0.10));
        assert_eq!(q.unit_price_at(9), Some(0.10));
        assert_eq!(q.unit_price_at(10), Some(0.08));
        assert_eq!(q.unit_price_at(250), Some(0.05));
        // Below the minimum order the first break is what can be bought.
        let moq = quote("D", "USD", &[(5, 0.20)]);
        assert_eq!(moq.unit_price_at(2), Some(0.20));
        assert_eq!(quote("D", "USD", &[]).unit_price_at(1), None);
        // Breaks stored out of order are read by quantity, not position.
        let unsorted = quote("D", "USD", &[(100, 0.05), (1, 0.10), (10, 0.08)]);
        assert_eq!(unsorted.unit_price_at(50), Some(0.08));
        assert_eq!(unsorted.unit_price_at(0), Some(0.10));
        assert_eq!(unsorted.minimum_order(), Some(1));
    }

    #[test]
    fn a_line_below_the_minimum_order_is_priced_at_the_minimum_order() {
        let mut book = PriceBook::default();
        book.by_row.insert(
            "lib/a".into(),
            vec![quote("D", "USD", &[(5, 0.20), (50, 0.10)])],
        );
        let mut table = BomTable {
            rows: vec![row("U1", 2, Some("lib/a"), None)],
            ..Default::default()
        };
        apply_prices(&mut table, &book);
        let price = table.rows[0].price.as_ref().unwrap();
        assert_eq!(price.buy_qty, 5, "you buy the minimum order");
        assert_eq!(price.unit_price, 0.20);
        assert!((price.extended_price - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_row_is_priced_from_its_library_row_then_its_mpn() {
        let mut book = PriceBook::default();
        book.by_row
            .insert("lib/row".into(), vec![quote("Mouser", "EUR", &[(1, 1.0)])]);
        book.by_mpn.insert(
            mpn_key(" rc0603 "),
            vec![quote("DigiKey", "USD", &[(1, 2.0)])],
        );

        let linked = row("R1", 1, Some("lib/row"), Some("RC0603"));
        assert_eq!(book.quote_for(&linked).unwrap().distributor, "Mouser");
        let by_mpn = row("R2", 1, None, Some("Rc0603"));
        assert_eq!(book.quote_for(&by_mpn).unwrap().distributor, "DigiKey");
        // A row link with no cached price falls back to the MPN.
        let stale_link = row("R3", 1, Some("lib/other"), Some("RC0603"));
        assert_eq!(book.quote_for(&stale_link).unwrap().distributor, "DigiKey");
        assert!(book.quote_for(&row("R4", 1, None, None)).is_none());
    }

    #[test]
    fn rows_are_priced_at_their_fitted_quantity_and_totalled_per_currency() {
        let mut book = PriceBook::default();
        book.by_row.insert(
            "lib/a".into(),
            vec![quote("DigiKey", "USD", &[(1, 0.10), (10, 0.08)])],
        );
        book.by_row
            .insert("lib/b".into(), vec![quote("Mouser", "EUR", &[(1, 1.50)])]);
        let mut table = BomTable {
            rows: vec![
                row("R1", 12, Some("lib/a"), None),
                row("U1", 2, Some("lib/b"), None),
                row("C1", 3, None, None),
                row("R9", 0, Some("lib/a"), None),
            ],
            ..Default::default()
        };

        apply_prices(&mut table, &book);

        let r1 = table.rows[0].price.as_ref().unwrap();
        assert_eq!(r1.unit_price, 0.08, "12 parts buy at the 10 break");
        assert!((r1.extended_price - 0.96).abs() < 1e-9);
        assert_eq!(r1.stock, Some(500));
        assert_eq!(r1.age, "2 d ago");
        let not_fitted = table.rows[3].price.as_ref().unwrap();
        assert_eq!(
            (not_fitted.unit_price, not_fitted.extended_price),
            (0.10, 0.0)
        );
        assert!(table.rows[2].price.is_none(), "C1 has no quote");

        let totals = price_totals(&table);
        assert!((totals["USD"] - 0.96).abs() < 1e-9);
        assert!((totals["EUR"] - 3.0).abs() < 1e-9);
        assert_eq!(totals.len(), 2);
    }
}
