//! #1988: `bom_lines` answers with the BOM the export dialog rolls up, with
//! a template's columns, computed ones included.

use std::collections::BTreeMap;

use serde_json::json;
use signex_types::schematic::{SchematicSheet, Symbol};

use crate::app::Signex;
use crate::app::command::queries::tests::{answer, path, refusal};
use crate::app::handlers::editing_commands::net_color_driver as fixture;

const POWER_TEMPLATE: &str = "format = \"snxbomt/1\"\n\n\
    [[columns]]\ncolumn = \"designator\"\n\n\
    [[columns]]\ncolumn = \"value\"\n\n\
    [[columns]]\nname = \"Power\"\nexpression = \"= VCC^2 / Value\"\n";

fn resistor(reference: &str, value: &str) -> Symbol {
    Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: value.to_string(),
        footprint: "R_0603".to_string(),
        ..Default::default()
    }
}

/// One open sheet, `bom-lines-1988.snxsch`, with R1 and R2 at 10 kΩ and C1,
/// and its own `VCC` when `vcc` is given.
fn app_with_parts(vcc: Option<&str>) -> (Signex, String) {
    let _ = crate::diagnostics::init_logging();
    let (mut app, _task) = Signex::new();
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![
            resistor("R1", "10kΩ"),
            resistor("R2", "10kΩ"),
            resistor("C1", "100nF"),
        ],
        variables: vcc
            .map(|v| BTreeMap::from([("VCC".to_string(), v.to_string())]))
            .unwrap_or_default(),
        ..Default::default()
    };
    let file = path("bom-lines-1988.snxsch");
    fixture::add_sheet_with(&mut app, &file, sheet);
    (app, format!("document={}", file.display()))
}

/// `text` as the `template` operand: its line breaks written `\n`.
fn template_arg(text: &str) -> String {
    format!("template={}", text.replace('\n', "\\n"))
}

#[test]
fn the_default_columns_list_every_line_and_a_template_that_names_them() {
    let (mut app, doc) = app_with_parts(None);
    let value = answer(&mut app, "bom_lines", &[&doc]);
    assert_eq!(value["variant"], "Base");
    assert_eq!(value["grouping"], "grouped");
    assert_eq!(value["columns"].as_array().unwrap().len(), 6);
    assert_eq!(
        value["columns"][2],
        json!({"header": "Designator", "kind": "standard", "key": "designator"})
    );
    let lines = value["lines"].as_array().unwrap();
    assert_eq!(lines.len(), 2, "{value:#}");
    assert_eq!(lines[1]["references"], json!(["R1", "R2"]));
    assert_eq!(lines[1]["qty"], 2);
    assert!(
        value["template"]
            .as_str()
            .unwrap()
            .starts_with("format = \"snxbomt/1\"")
    );
    assert_eq!(value["issues"], json!([]));
}

#[test]
fn a_computed_column_answers_with_its_value_and_unit() {
    let (mut app, doc) = app_with_parts(Some("5V"));
    let template = template_arg(POWER_TEMPLATE);
    let value = answer(&mut app, "bom_lines", &[&doc, &template]);
    assert_eq!(
        value["columns"][2],
        json!({"header": "Power", "kind": "computed", "expression": "= VCC^2 / Value"})
    );
    let r_line = &value["lines"][1];
    assert_eq!(r_line["cells"], json!(["R1, R2", "10kΩ", "2.5mW"]));
    assert_eq!(
        r_line["computed"]["Power"],
        json!({"state": "value", "text": "2.5mW"})
    );
    // C1 is 100 nF: V² over farads is no power, but it evaluates.
    assert_eq!(value["issues"], json!([]));
}

#[test]
fn a_row_whose_expression_fails_is_empty_and_listed_as_an_issue() {
    let (mut app, doc) = app_with_parts(None);
    let template = template_arg(POWER_TEMPLATE);
    let value = answer(&mut app, "bom_lines", &[&doc, &template]);
    assert_eq!(value["lines"][0]["cells"][2], "");
    let issues = value["issues"].as_array().unwrap();
    assert_eq!(issues.len(), 2, "{value:#}");
    assert_eq!(issues[1]["references"], json!(["R1", "R2"]));
    assert_eq!(issues[1]["column"], "Power");
    assert_eq!(issues[1]["member"], "R1");
    assert_eq!(issues[1]["error"], "unknown parameter 'VCC'");
}

#[test]
fn the_answers_template_reads_back_to_the_same_columns_byte_identical() {
    let (mut app, doc) = app_with_parts(Some("5V"));
    let first = answer(
        &mut app,
        "bom_lines",
        &[&doc, &template_arg(POWER_TEMPLATE)],
    );
    let text = first["template"].as_str().unwrap().to_string();
    let second = answer(&mut app, "bom_lines", &[&doc, &template_arg(&text)]);
    assert_eq!(second["template"], text.as_str());
    assert_eq!(second["columns"], first["columns"]);
    assert_eq!(second["lines"], first["lines"]);
}

#[test]
fn ungrouped_lists_one_line_per_part() {
    let (mut app, doc) = app_with_parts(None);
    let value = answer(&mut app, "bom_lines", &[&doc, "grouping=ungrouped"]);
    assert_eq!(value["grouping"], "ungrouped");
    assert_eq!(value["lines"].as_array().unwrap().len(), 3);
}

#[test]
fn a_bad_template_grouping_or_variant_is_refused_with_the_reason() {
    let (mut app, doc) = app_with_parts(None);
    let no_token = template_arg("[[columns]]\ncolumn = \"qty\"\n");
    assert!(refusal(&mut app, "bom_lines", &[&doc, &no_token]).contains("format token"));
    let unknown = template_arg("format = \"snxbomt/1\"\n[[columns]]\ncolumn = \"weight\"\n");
    assert!(refusal(&mut app, "bom_lines", &[&doc, &unknown]).contains("`weight`"));
    assert!(
        refusal(&mut app, "bom_lines", &[&doc, "grouping=by_value"]).contains("unknown grouping")
    );
    assert!(refusal(&mut app, "bom_lines", &[&doc, "variant=Lite"]).contains("no variant `Lite`"));
    assert!(refusal(&mut app, "bom_lines", &[]).contains("document"));
}
