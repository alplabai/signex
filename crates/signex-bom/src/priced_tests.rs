//! #2938: a priced line, a line with no price, a line served by an
//! alternate and a line priced only in another currency.

use std::collections::BTreeMap;

use super::*;
use crate::PriceQuote;

const R: &str = "lib/r";
const C: &str = "lib/c";
const U: &str = "lib/u";
const ALT: &str = "lib/alt";

fn quote(currency: &str, stock: Option<u32>, breaks: &[(u32, f64)]) -> PriceQuote {
    PriceQuote {
        distributor: "D".into(),
        currency: currency.into(),
        stock,
        breaks: breaks.to_vec(),
        age: "1 d ago".into(),
    }
}

fn row(references: &[&str], library_row: Option<&str>, mpn: Option<&str>) -> BomRow {
    let fitted = references.len() as u32;
    BomRow {
        references: references.iter().map(|r| r.to_string()).collect(),
        qty: fitted,
        fitted_qty: fitted,
        library_row: library_row.map(str::to_string),
        custom: mpn
            .map(|mpn| BTreeMap::from([("MPN".to_string(), mpn.to_string())]))
            .unwrap_or_default(),
        ..Default::default()
    }
}

/// R1,R2 priced; C1 with no price; U1 out of stock with an alternate in
/// stock; D1 priced in EUR only.
fn fixture() -> (BomTable, PriceBook, Vec<BomLineAlternates>) {
    let table = BomTable {
        rows: vec![
            row(&["R1", "R2"], Some(R), None),
            row(&["C1"], Some(C), None),
            row(&["U1"], Some(U), None),
            row(&["D1"], None, Some("bat54")),
        ],
        ..Default::default()
    };
    let mut book = PriceBook::default();
    book.by_row.insert(
        R.into(),
        vec![quote("USD", Some(1000), &[(1, 0.10), (10, 0.05)])],
    );
    book.by_row
        .insert(U.into(), vec![quote("USD", Some(0), &[(1, 2.00)])]);
    book.by_row
        .insert(ALT.into(), vec![quote("USD", Some(50), &[(1, 2.50)])]);
    book.by_mpn
        .insert(mpn_key("bat54"), vec![quote("EUR", Some(10), &[(1, 0.30)])]);
    let alternates = vec![BomLineAlternates {
        line: U.into(),
        rows: vec![ALT.into()],
    }];
    (table, book, alternates)
}

#[test]
fn each_line_is_priced_for_the_build_and_an_unpriced_line_is_not_counted() {
    let (table, book, alternates) = fixture();
    let order = BuildOrder {
        quantity: 5,
        currency: Some("usd"),
    };
    let bom = priced_bom(&table, &book, &alternates, order);
    let r = &bom.lines[0];
    assert_eq!((r.quantity, r.build_quantity), (2, 10));
    let price = r.price.as_ref().expect("R1,R2 are priced");
    assert_eq!(price.unit_price, 0.05, "10 parts buy at the 10 break");
    assert_eq!(r.unpriced, None);
    assert_eq!(r.key.as_deref(), Some(R));

    let c = &bom.lines[1];
    assert_eq!(c.price, None);
    assert_eq!(c.unpriced, Some(Shortfall::NoPrice));

    let d = &bom.lines[3];
    assert_eq!(d.key.as_deref(), Some("mpn:BAT54"));
    assert_eq!(d.unpriced, Some(Shortfall::OtherCurrency));

    assert_eq!(bom.unpriced, ["C1", "D1"]);
    assert_eq!(bom.total, None, "an unpriced line makes the total unknown");
    assert_eq!(bom.unit_cost, None);
    // R: 10 x 0.05; U1: 5 x 2.00, out of stock but priced.
    assert_eq!(bom.known_total, Some(10.5));
}

#[test]
fn a_part_out_of_stock_is_served_by_its_first_alternate_in_stock() {
    let (table, book, alternates) = fixture();
    let order = BuildOrder {
        quantity: 5,
        currency: Some("USD"),
    };
    let bom = priced_bom(&table, &book, &alternates, order);
    let u = &bom.lines[2];
    assert_eq!(u.unpriced, None, "the part has a price; it counts");
    assert_eq!(u.alternates.len(), 1);
    let served = u.served_by.as_ref().expect("the alternate serves the line");
    assert_eq!(served.row, ALT);
    assert_eq!(served.reason, Shortfall::NoStock);
    assert_eq!(served.price.extended_price, 12.5);
    assert!(bom.lines[0].served_by.is_none(), "R1,R2 are in stock");
}

#[test]
fn a_complete_bom_has_a_total_and_a_unit_cost_per_currency() {
    let (mut table, book, alternates) = fixture();
    table
        .rows
        .retain(|row| row.library_row.as_deref() == Some(R));
    let order = BuildOrder {
        quantity: 10,
        currency: None,
    };
    let bom = priced_bom(&table, &book, &alternates, order);
    assert_eq!(bom.total, Some(1.0), "20 parts at 0.05");
    assert_eq!(bom.unit_cost, Some(0.1));
    assert_eq!(bom.totals, BTreeMap::from([("USD".to_string(), 1.0)]));
    assert!(bom.unpriced.is_empty());
}

#[test]
fn each_row_gets_its_lines_alternates_for_the_export() {
    let (mut table, _, alternates) = fixture();
    apply_alternates(&mut table, &alternates);
    assert_eq!(table.rows[2].alternates, [ALT]);
    assert!(table.rows[0].alternates.is_empty());
    assert!(table.rows[3].alternates.is_empty(), "an MPN line with none");
}
