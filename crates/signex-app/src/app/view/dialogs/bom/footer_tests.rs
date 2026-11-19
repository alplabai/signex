use signex_output::bom::BomPrice;
use signex_output::{BomColumn, BomFormat, BomRow, BomTable};

fn en() -> signex_i18n::I18n {
    signex_i18n::I18n::en_us()
}

fn price_status(table: &BomTable, columns: &[BomColumn]) -> Option<String> {
    super::price_status(&en(), table, columns)
}

fn status_line(table: &BomTable, columns: &[BomColumn], visible: usize) -> String {
    super::status_line(&en(), table, columns, visible)
        .into_string()
        .replace(['\u{2068}', '\u{2069}'], "")
}

fn row(price: Option<(&str, f64)>) -> BomRow {
    BomRow {
        qty: 1,
        price: price.map(|(currency, extended_price)| BomPrice {
            distributor: "D".into(),
            currency: currency.into(),
            unit_price: extended_price,
            extended_price,
            buy_qty: 1,
            stock: None,
            age: String::new(),
        }),
        ..Default::default()
    }
}

#[test]
fn the_footer_shows_totals_and_unpriced_lines_only_with_a_price_column() {
    let table = BomTable {
        rows: vec![row(Some(("USD", 1.5))), row(Some(("EUR", 2.0))), row(None)],
        ..Default::default()
    };
    assert_eq!(price_status(&table, &[BomColumn::Name]), None);
    assert_eq!(
        price_status(&table, &[BomColumn::UnitPrice]).as_deref(),
        Some("Total 2.00 EUR + 1.50 USD · 1 line without a cached price")
    );
    let two_unpriced = BomTable {
        rows: vec![row(Some(("USD", 1.5))), row(None), row(None)],
        ..Default::default()
    };
    assert_eq!(
        price_status(&two_unpriced, &[BomColumn::UnitPrice]).as_deref(),
        Some("Total 1.50 USD · 2 lines without a cached price")
    );
    let all_priced = BomTable {
        rows: vec![row(Some(("USD", 1.5)))],
        ..Default::default()
    };
    assert_eq!(
        price_status(&all_priced, &[BomColumn::UnitPrice]).as_deref(),
        Some("Total 1.50 USD")
    );
    let none = BomTable {
        rows: vec![row(None)],
        ..Default::default()
    };
    assert_eq!(
        price_status(&none, &[BomColumn::ExtendedPrice]).as_deref(),
        Some("no cached prices")
    );
}

/// m112: the status reads as drawn: lines shown of all lines, the parts the
/// BOM lists, then the price note, with no "|" and no variant (the variant
/// is shown once, in the toolbar).
#[test]
fn the_status_line_counts_lines_and_parts_and_adds_the_price_note() {
    let mut table = BomTable {
        rows: vec![row(Some(("EUR", 4.86))), row(None), row(None)],
        ..Default::default()
    };
    table.rows[0].qty = 29;
    assert_eq!(
        status_line(&table, &[BomColumn::UnitPrice], 3),
        "3 of 3 lines · 31 parts · Total 4.86 EUR · 2 lines without a cached price"
    );
    assert_eq!(
        status_line(&table, &[BomColumn::Name], 2),
        "2 of 3 lines · 31 parts"
    );
    let one = BomTable {
        rows: vec![row(None)],
        ..Default::default()
    };
    assert_eq!(
        status_line(&one, &[BomColumn::Name], 1),
        "1 of 1 lines · 1 part"
    );
}

/// m112: the primary button names the format it writes.
#[test]
fn the_export_button_names_its_format() {
    let shown = |format| en().message_plain(super::export_label(format), &[]);
    assert_eq!(shown(BomFormat::Csv), "Export CSV\u{2026}");
    assert_eq!(shown(BomFormat::Xlsx), "Export XLSX\u{2026}");
    assert_eq!(shown(BomFormat::Html), "Export HTML\u{2026}");
}
