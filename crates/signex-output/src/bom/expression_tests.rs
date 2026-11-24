//! #1986 review, maintainer decision 2026-11-18: the BOM lists and groups
//! parts by their evaluated value, the one the sheet shows.

use super::tests::{context_with, unit_of};
use super::*;

fn part(reference: &str, value: &str, rbase: &str) -> Symbol {
    let mut part = unit_of(reference, "Device:R", 1);
    part.value = value.to_string();
    part.fields.insert("Rbase".to_string(), rbase.to_string());
    part
}

fn values_by_reference(table: &BomTable) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = table
        .rows
        .iter()
        .map(|row| (row.references.join(","), row.value.clone()))
        .collect();
    rows.sort();
    rows
}

#[test]
fn one_expression_with_different_inputs_lands_on_separate_lines() {
    let ctx = context_with(vec![
        part("R1", "= Rbase * 2", "1kΩ"),
        part("R2", "= Rbase * 2", "2kΩ"),
    ]);
    let table = rollup(&ctx, &BomOptions::default());
    assert_eq!(
        values_by_reference(&table),
        [
            ("R1".to_string(), "2kΩ".to_string()),
            ("R2".to_string(), "4kΩ".to_string()),
        ]
    );
}

#[test]
fn parts_that_evaluate_alike_group_on_one_line() {
    // The same evaluated value and the same fields: one line, though one
    // value is typed and the other calculated.
    let ctx = context_with(vec![
        part("R1", "2kΩ", "1kΩ"),
        part("R2", "= Rbase * 2", "1kΩ"),
    ]);
    let table = rollup(&ctx, &BomOptions::default());
    assert_eq!(table.rows.len(), 1, "{:?}", values_by_reference(&table));
    assert_eq!(table.rows[0].value, "2kΩ");
    assert_eq!(table.rows[0].references, ["R1", "R2"]);
}

#[test]
fn a_value_that_does_not_evaluate_keeps_its_source() {
    let ctx = context_with(vec![part("R1", "= R9.Value", "1kΩ")]);
    let table = rollup(&ctx, &BomOptions::default());
    assert_eq!(table.rows[0].value, "= R9.Value");
}
