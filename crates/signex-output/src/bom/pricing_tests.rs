//! #1023 — cached library prices through the rollup and every writer.

use uuid::Uuid;

use super::tests::{context_with, unit_of};
use super::*;
use signex_bom::{PriceBook, PriceQuote, library_row_key};

const LIBRARY: u128 = 0xA;
const ROW: u128 = 0xB;

/// R1 and R2 placed from one priced library row; C1 from no row.
fn priced_context() -> (ExportContext, PriceBook) {
    let linked = |reference: &str| Symbol {
        library_id: Some(Uuid::from_u128(LIBRARY)),
        row_id: Some(Uuid::from_u128(ROW)),
        ..unit_of(reference, "Device:R", 1)
    };
    let ctx = context_with(vec![
        linked("R1"),
        linked("R2"),
        unit_of("C1", "Device:C", 1),
    ]);
    let mut book = PriceBook::default();
    book.by_row.insert(
        library_row_key(
            &Uuid::from_u128(LIBRARY).to_string(),
            &Uuid::from_u128(ROW).to_string(),
        ),
        vec![PriceQuote {
            distributor: "DigiKey".into(),
            currency: "USD".into(),
            stock: Some(1200),
            breaks: vec![(1, 0.125), (10, 0.05)],
            age: "3 d ago".into(),
        }],
    );
    (ctx, book)
}

fn priced_options(book: PriceBook) -> BomOptions {
    BomOptions {
        columns: vec![
            BomColumn::Designator,
            BomColumn::Qty,
            BomColumn::UnitPrice,
            BomColumn::ExtendedPrice,
            BomColumn::Currency,
            BomColumn::Stock,
            BomColumn::Distributor,
            BomColumn::PriceAge,
        ],
        price_book: book,
        ..BomOptions::default()
    }
}

#[test]
fn a_placed_part_is_priced_from_its_library_row() {
    let (ctx, book) = priced_context();
    let table = rollup(&ctx, &priced_options(book));
    let resistors = table
        .rows
        .iter()
        .find(|row| row.references.contains(&"R1".to_string()))
        .unwrap();
    assert_eq!(resistors.qty, 2);
    let price = resistors.price.as_ref().expect("priced");
    assert_eq!(price.unit_price, 0.125);
    assert_eq!(price.extended_price, 0.25);
    let capacitor = table
        .rows
        .iter()
        .find(|row| row.references == ["C1"])
        .unwrap();
    assert!(
        capacitor.price.is_none(),
        "no library row, no MPN: unpriced"
    );
}

#[test]
fn the_csv_carries_the_price_cells_and_a_total_line() {
    let (ctx, book) = priced_context();
    let opts = priced_options(book);
    let table = rollup(&ctx, &opts);
    let csv = String::from_utf8(csv_emit(&table, &opts.columns).unwrap()).unwrap();
    let lines: Vec<&str> = csv.trim_start_matches('\u{feff}').lines().collect();
    assert_eq!(
        lines[0],
        "Designator,Qty,Unit Price,Ext. Price,Currency,Stock,Distributor,Price Age"
    );
    assert!(
        lines.contains(&"C1,1,,,,,,"),
        "an unpriced line keeps empty price cells: {lines:?}"
    );
    assert!(
        lines.contains(&"\"R1, R2\",2,0.125,0.25,USD,1200,DigiKey,3 d ago"),
        "{lines:?}"
    );
    assert_eq!(lines.last(), Some(&"Total,,,0.25,USD,,,"));
}

#[test]
fn the_html_and_xlsx_writers_emit_the_total_line_too() {
    let (ctx, book) = priced_context();
    let opts = priced_options(book);
    let table = rollup(&ctx, &opts);
    let html = String::from_utf8(html_emit(&table, &opts.columns).unwrap()).unwrap();
    assert!(html.contains("<tr class=\"total\">"), "{html}");
    assert!(html.contains("<td>0.25</td>"));
    assert!(!xlsx_emit(&table, &opts.columns).unwrap().is_empty());
}

#[test]
fn without_the_extended_price_column_there_is_no_total_line() {
    let (ctx, book) = priced_context();
    let mut opts = priced_options(book);
    opts.columns
        .retain(|column| *column != BomColumn::ExtendedPrice);
    let table = rollup(&ctx, &opts);
    assert!(total_lines(&table, &opts.columns).is_empty());
}

#[test]
fn unit_prices_show_two_to_four_decimals() {
    assert_eq!(unit_money(2.0), "2.00");
    assert_eq!(unit_money(0.5), "0.50");
    assert_eq!(unit_money(0.125), "0.125");
    assert_eq!(unit_money(0.01234), "0.0123");
    assert_eq!(money(0.249), "0.25");
}

#[test]
fn the_total_label_survives_when_the_first_columns_are_the_sum_and_the_currency() {
    let (ctx, book) = priced_context();
    let mut opts = priced_options(book);
    opts.columns = vec![BomColumn::ExtendedPrice, BomColumn::Currency];
    let table = rollup(&ctx, &opts);
    let lines = total_lines(&table, &opts.columns);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0][0].0, "0.25");
    assert_eq!(lines[0][1].0, "Total USD");

    opts.columns = vec![BomColumn::ExtendedPrice];
    let lines = total_lines(&table, &opts.columns);
    assert_eq!(lines[0][0].0, "Total 0.25");
}

#[test]
fn an_empty_line_separates_the_csv_totals_from_the_parts() {
    let (ctx, book) = priced_context();
    let opts = priced_options(book);
    let table = rollup(&ctx, &opts);
    let csv = String::from_utf8(csv_emit(&table, &opts.columns).unwrap()).unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    let total = lines.iter().position(|l| l.starts_with("Total")).unwrap();
    assert_eq!(lines[total - 1], "", "{lines:?}");
}
