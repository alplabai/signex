//! The one rule a value is shown by (#1596, #1986), for the canvas and every
//! output alike.

use super::*;
use std::collections::BTreeMap;

fn part(reference: &str, value: &str) -> Symbol {
    Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: value.to_string(),
        ..Default::default()
    }
}

fn project() -> ProjectMetadata {
    let params = BTreeMap::from([
        ("title".to_string(), "Supply".to_string()),
        ("VCC".to_string(), "5V".to_string()),
    ]);
    let mut metadata = ProjectMetadata::from_values(&params);
    metadata.project_parameters = Some(params);
    metadata
}

#[test]
fn an_expression_markup_and_a_special_string_resolve_by_one_rule() {
    let metadata = project();
    let special = sheet_context(&metadata);
    let mut tolerance = part("R1", "${TITLE} @{Tolerance}");
    tolerance
        .fields
        .insert("Tolerance".to_string(), "1%".to_string());
    let symbols = vec![tolerance, part("R2", "= VCC / 10mA")];
    let scope = ValueScope::of_sheet(&symbols, Some(&special));
    assert_eq!(shown_value(&scope, &symbols[0]).text, "Supply 1%");
    assert_eq!(shown_value(&scope, &symbols[1]).text, "500Ω");
}

#[test]
fn a_value_that_does_not_evaluate_shows_its_source_and_says_why() {
    let symbols = vec![part("R1", "= R9.Value")];
    let shown = shown_value(&ValueScope::of_sheet(&symbols, None), &symbols[0]);
    assert_eq!(shown.text, "= R9.Value");
    assert_eq!(
        shown.error,
        Some(ExprError::Unknown("R9.Value".to_string()))
    );
}

#[test]
fn a_power_port_value_is_a_net_name() {
    let metadata = project();
    let special = sheet_context(&metadata);
    let mut port = part("#PWR1", "${VCC}");
    port.is_power = true;
    let mut evaluated = part("#PWR2", "= VCC");
    evaluated.is_power = true;
    let symbols = vec![port, evaluated];
    let scope = ValueScope::of_sheet(&symbols, Some(&special));
    assert_eq!(shown_value(&scope, &symbols[0]).text, "${VCC}");
    assert_eq!(shown_value(&scope, &symbols[1]).text, "= VCC");
}
