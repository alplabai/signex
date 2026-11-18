//! Sheet and project variables in special strings (#1987, #1596).

use super::*;
use signex_types::schematic::SchematicSheet;
use std::collections::BTreeMap;

fn project() -> ProjectMetadata {
    let params = BTreeMap::from([
        ("rev".to_string(), "B".to_string()),
        ("VCC".to_string(), "5V".to_string()),
        ("VREF".to_string(), "= VCC / 2".to_string()),
        ("LOOP".to_string(), "= LOOP * 2".to_string()),
    ]);
    let mut m = ProjectMetadata::from_values(&params);
    m.project_parameters = Some(params);
    m
}

fn sheet(variables: &[(&str, &str)], title_block: &[(&str, &str)]) -> SchematicSheet {
    SchematicSheet {
        variables: variables
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        title_block: title_block
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        ..SchematicSheet::default()
    }
}

fn on<'a>(m: &'a ProjectMetadata) -> SubstitutionContext<'a> {
    SubstitutionContext {
        metadata: m,
        document_path: std::path::PathBuf::new(),
        filename: "power.snxsch".into(),
        sheet_name: "power".into(),
        sheet_number: 1,
        sheet_count: 1,
        signex_version: "1.0.0",
        variant: None,
        physical_structure: false,
        physical_sheet_number: true,
        physical_document_number: true,
    }
}

#[test]
fn a_project_variable_resolves_and_its_expression_evaluates() {
    let m = project().for_schematic(&sheet(&[], &[]));
    assert_eq!(resolve("${VCC} ${VREF} ${REVISION}", &on(&m)), "5V 2.5V B");
}

#[test]
fn a_sheet_variable_shadows_the_project_variable() {
    let m = project().for_schematic(&sheet(&[("VCC", "3.3V")], &[]));
    assert_eq!(resolve("${VCC} ${VREF}", &on(&m)), "3.3V 1.65V");
    // Another sheet keeps the project's value.
    let other = project().for_schematic(&sheet(&[], &[]));
    assert_eq!(resolve("${VCC}", &on(&other)), "5V");
}

#[test]
fn a_cycle_or_an_unknown_name_draws_the_source_not_a_value() {
    let m = project().for_schematic(&sheet(&[("BAD", "= NOPE + 1")], &[]));
    assert_eq!(resolve("[${LOOP}]", &on(&m)), "[= LOOP * 2]");
    assert_eq!(resolve("[${BAD}]", &on(&m)), "[= NOPE + 1]");
    // An unknown token stays empty, as before.
    assert_eq!(resolve("[${MISSING}]", &on(&m)), "[]");
}

#[test]
fn a_title_block_expression_draws_its_value_and_keeps_its_source() {
    let source = sheet(&[("VCC", "3.3V")], &[("rev", "= VCC * 2")]);
    let m = project().for_schematic(&source);
    assert_eq!(resolve("Rev ${REV}", &on(&m)), "Rev 6.6V");
    assert_eq!(source.title_block["rev"], "= VCC * 2");
}

#[test]
fn a_text_note_follows_the_eq_rule() {
    let m = project().for_schematic(&sheet(&[("VCC", "3.3V")], &[]));
    let c = on(&m);
    assert_eq!(resolve_text("= VCC / 2", &c), "1.65V");
    assert_eq!(resolve_text("Supply ${VCC}", &c), "Supply 3.3V");
    assert_eq!(resolve_text("==5", &c), "=5");
    // Not an expression that evaluates: the note shows what was typed.
    assert_eq!(resolve_text("= see note", &c), "= see note");
}
