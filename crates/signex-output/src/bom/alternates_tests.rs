//! #2938: the project's alternates per BOM line reach the Alternates column
//! of every writer, and a line with none keeps the cell empty.

use uuid::Uuid;

use super::tests::{context_with, unit_of};
use super::*;
use signex_types::project::BomLineAlternates;

const LIBRARY: u128 = 0xA;
const ROW: u128 = 0xB;
const ALT_A: &str = "0000000a-0000-0000-0000-00000000000a/0000000a-0000-0000-0000-0000000000aa";
const ALT_B: &str = "0000000a-0000-0000-0000-00000000000a/0000000a-0000-0000-0000-0000000000bb";

#[test]
fn the_csv_lists_a_lines_alternates_in_order() {
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
    let line = library_row_key(
        &Uuid::from_u128(LIBRARY).to_string(),
        &Uuid::from_u128(ROW).to_string(),
    );
    let opts = BomOptions {
        columns: vec![BomColumn::Designator, BomColumn::Alternates],
        alternates: vec![BomLineAlternates {
            line,
            rows: vec![ALT_B.into(), ALT_A.into()],
        }],
        ..BomOptions::default()
    };
    let (table, _) = rollup_with_validation(&ctx, &opts);
    let csv = String::from_utf8(csv_emit(&table, &opts.columns).unwrap()).unwrap();
    let lines: Vec<&str> = csv.trim_start_matches('\u{feff}').lines().collect();
    assert_eq!(lines[0], "Designator,Alternates");
    assert!(
        lines.contains(&format!("\"R1, R2\",\"{ALT_B}, {ALT_A}\"").as_str()),
        "{lines:?}"
    );
    assert!(lines.contains(&"C1,"), "a line with none: {lines:?}");
    let html = String::from_utf8(html_emit(&table, &opts.columns).unwrap()).unwrap();
    assert!(html.contains(ALT_A), "{html}");
    assert!(!xlsx_emit(&table, &opts.columns).unwrap().is_empty());
}
