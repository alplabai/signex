//! #1988: computed BOM columns through the rollup and the writers.

use std::collections::BTreeMap;

use super::tests::{context_with, unit_of};
use super::*;
use crate::SheetSnapshot;

fn resistor(reference: &str) -> Symbol {
    Symbol {
        value: "10kΩ".to_string(),
        ..unit_of(reference, "Device:R", 1)
    }
}

fn power() -> BomColumn {
    BomColumn::Computed(ComputedColumn::new("Power", "= VCC^2 / Value").unwrap())
}

fn options() -> BomOptions {
    BomOptions {
        columns: vec![BomColumn::Designator, BomColumn::Value, power()],
        ..BomOptions::default()
    }
}

fn with_project_vcc(mut ctx: ExportContext, vcc: &str) -> ExportContext {
    ctx.metadata.project_parameters = Some(BTreeMap::from([("VCC".to_string(), vcc.to_string())]));
    ctx
}

/// A second page holding `symbols`, with its own `VCC`.
fn page_with_vcc(symbols: Vec<Symbol>, vcc: &str) -> SheetSnapshot {
    SheetSnapshot {
        path: "rail.snxsch".into(),
        schematic: SchematicSheet {
            paper_size: "A4".to_string(),
            symbols,
            variables: BTreeMap::from([("VCC".to_string(), vcc.to_string())]),
            ..Default::default()
        },
        sheet_name: "Rail".to_string(),
        sheet_number: 2,
        sheet_count: 2,
        placement: Default::default(),
    }
}

#[test]
fn a_computed_column_reads_the_project_variable_and_exports_as_text() {
    let ctx = with_project_vcc(context_with(vec![resistor("R1")]), "5V");
    let opts = options();
    let table = rollup(&ctx, &opts);
    assert_eq!(cell_text(&table.rows[0], &power()), "2.5mW");
    assert_eq!(
        cell_number(&table.rows[0], &power()),
        None,
        "XLSX writes it as text"
    );
    let csv = String::from_utf8(csv_emit(&table, &opts.columns).unwrap()).unwrap();
    assert!(csv.contains("Designator,Value,Power\r\n"), "{csv}");
    assert!(csv.contains("R1,10kΩ,2.5mW\r\n"), "{csv}");
    assert!(xlsx_emit(&table, &opts.columns).is_ok());
    assert!(computed_issues(&table, &opts.columns).is_empty());
}

#[test]
fn a_grouped_line_whose_parts_sit_on_sheets_with_different_rails_says_they_differ() {
    let mut ctx = with_project_vcc(context_with(vec![resistor("R1")]), "5V");
    ctx.sheets.push(page_with_vcc(vec![resistor("R2")], "3.3V"));
    let table = rollup(&ctx, &options());
    assert_eq!(table.rows.len(), 1);
    assert_eq!(table.rows[0].references, ["R1", "R2"]);
    assert_eq!(cell_text(&table.rows[0], &power()), MEMBERS_DIFFER);
    let csv = String::from_utf8(csv_emit(&table, &options().columns).unwrap()).unwrap();
    assert!(csv.contains("(members differ)"), "{csv}");
}

#[test]
fn a_grouped_line_whose_parts_agree_shows_the_result() {
    let mut ctx = with_project_vcc(context_with(vec![resistor("R1")]), "5V");
    ctx.sheets.push(page_with_vcc(vec![resistor("R2")], "5V"));
    let table = rollup(&ctx, &options());
    assert_eq!(table.rows.len(), 1);
    assert_eq!(cell_text(&table.rows[0], &power()), "2.5mW");
}

#[test]
fn a_row_that_fails_is_empty_and_named_once() {
    let ctx = context_with(vec![resistor("R1")]);
    let opts = options();
    let (table, _) = rollup_with_validation(&ctx, &opts);
    assert_eq!(cell_text(&table.rows[0], &power()), "");
    let issues = computed_issues(&table, &opts.columns);
    assert_eq!(issues.len(), 1);
    assert_eq!(
        issues[0].to_string(),
        "BOM line 1 (R1): column Power: unknown parameter 'VCC'"
    );
}

#[test]
fn the_export_writes_a_failed_cell_empty_and_hands_back_its_issue() {
    let ctx = context_with(vec![resistor("R1")]);
    let opts = BomOptions {
        format: BomFormat::Csv,
        ..options()
    };
    let output = BomExporter.export(&ctx, &opts).unwrap();
    let csv = String::from_utf8(output.bytes).unwrap();
    assert!(csv.contains("R1,10kΩ,\r\n"), "{csv}");
    assert_eq!(output.computed_issues.len(), 1);
    assert_eq!(output.computed_issues[0].column, "Power");
}

#[test]
fn no_computed_column_leaves_the_rows_as_they_were() {
    let ctx = with_project_vcc(context_with(vec![resistor("R1")]), "5V");
    let table = rollup(&ctx, &BomOptions::default());
    assert!(table.rows[0].computed.is_empty());
}
