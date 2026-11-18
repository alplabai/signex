use super::*;
use crate::property::SchematicProperty;
use crate::variables::TextTable;
use std::collections::BTreeMap;

fn part(reference: &str, value: &str) -> Symbol {
    Symbol {
        uuid: uuid::Uuid::now_v7(),
        reference: reference.to_string(),
        value: value.to_string(),
        ..Default::default()
    }
}

fn value_of(symbols: &[Symbol], at: usize) -> Option<Result<String, ExprError>> {
    part_field_value(symbols, &symbols[at], "Value", &[], false)
}

#[test]
fn a_literal_field_is_drawn_as_typed() {
    let symbols = vec![part("R1", "10k")];
    assert_eq!(value_of(&symbols, 0), None);
}

#[test]
fn an_own_field_reference_evaluates_with_its_unit() {
    let mut r1 = part("R1", "= Rbase * 2");
    r1.fields.insert("Rbase".to_string(), "4.7kΩ".to_string());
    let symbols = vec![r1];
    assert_eq!(value_of(&symbols, 0), Some(Ok("9.4kΩ".to_string())));
}

#[test]
fn a_custom_property_is_an_own_field_too() {
    let mut c1 = part("C1", "= C / 2");
    c1.custom_properties.push(SchematicProperty {
        key: "C".to_string(),
        value: "200nF".to_string(),
        ..Default::default()
    });
    assert_eq!(value_of(&[c1], 0), Some(Ok("100nF".to_string())));
}

#[test]
fn a_cross_part_reference_reads_the_other_parts_field() {
    let mut r1 = part("R1", "= Rbase");
    r1.fields.insert("Rbase".to_string(), "10kΩ".to_string());
    // R2's expression names R1's value; R1's own `Rbase` stays R1's.
    let mut r2 = part("R2", "= 2 * R1.Value");
    r2.fields.insert("Rbase".to_string(), "1Ω".to_string());
    let symbols = vec![r1, r2];
    assert_eq!(value_of(&symbols, 1), Some(Ok("20kΩ".to_string())));
}

#[test]
fn a_multi_unit_part_resolves_to_the_part() {
    let mut unit_a = part("U1", "= 3.3V");
    unit_a.unit = 1;
    let mut unit_b = part("U1", "= 3.3V");
    unit_b.unit = 2;
    let reader = part("R1", "= U1.Value / 10mA");
    let symbols = vec![unit_a, unit_b, reader];
    assert_eq!(value_of(&symbols, 2), Some(Ok("330Ω".to_string())));
}

#[test]
fn a_cycle_between_two_parts_is_an_error_and_draws_the_source() {
    let symbols = vec![part("R1", "= R2.Value"), part("R2", "= R1.Value * 2")];
    assert!(matches!(
        value_of(&symbols, 0),
        Some(Err(ExprError::Cycle(_)))
    ));
    assert_eq!(
        part_field_display(&symbols, &symbols[0], "Value", &[], false),
        Some("= R2.Value".to_string())
    );
    let issues = part_field_issues(&symbols, &[], false);
    assert_eq!(issues.len(), 2);
    assert!(issues[0].to_string().starts_with("R1 field 'Value': "));
}

#[test]
fn a_unit_mismatch_and_an_unknown_designator_are_errors() {
    let symbols = vec![part("R1", "= 1V + 1A"), part("R2", "= R9.Value")];
    assert!(matches!(
        value_of(&symbols, 0),
        Some(Err(ExprError::UnitMismatch { .. }))
    ));
    assert_eq!(
        value_of(&symbols, 1),
        Some(Err(ExprError::Unknown("R9.Value".to_string())))
    );
}

#[test]
fn a_field_reads_the_sheet_and_project_variables() {
    let sheet = BTreeMap::from([("VCC".to_string(), "3.3V".to_string())]);
    let project = BTreeMap::from([("ILED".to_string(), "10mA".to_string())]);
    let (sheet_t, project_t) = (TextTable(&sheet), TextTable(&project));
    let variables: [&dyn Resolver; 2] = [&sheet_t, &project_t];
    let symbols = vec![part("R1", "= (VCC - 2V) / ILED")];
    assert_eq!(
        part_field_value(&symbols, &symbols[0], "Value", &variables, false),
        Some(Ok("130Ω".to_string()))
    );
}

#[test]
fn an_unannotated_part_still_reads_its_own_fields() {
    let mut r = part("R?", "= Rbase");
    r.fields.insert("Rbase".to_string(), "1kΩ".to_string());
    assert_eq!(value_of(&[r], 0), Some(Ok("1kΩ".to_string())));
}

/// #1416: under the full-codes standard `-QAC1` is only how QAC1 is shown;
/// typing the sign in an expression is an error, not a silent negation.
#[test]
fn a_signed_designator_in_an_expression_is_an_error() {
    let source = part("QAC1", "10kΩ");
    let reader = part("QAC2", "= -QAC1.Value");
    let symbols = vec![source, reader];
    let shown = part_field_value(&symbols, &symbols[1], "Value", &[], true);
    let Some(Err(ExprError::Domain(why))) = shown.clone() else {
        panic!("expected a domain error, got {shown:?}");
    };
    assert!(why.contains("display-only"), "{why}");
    // Without the sign it reads the part; under another standard the `-`
    // is an ordinary negation.
    let plain = vec![part("QAC1", "10kΩ"), part("QAC2", "= QAC1.Value * 2")];
    assert_eq!(
        part_field_value(&plain, &plain[1], "Value", &[], true),
        Some(Ok("20kΩ".to_string()))
    );
    assert_eq!(
        part_field_value(&symbols, &symbols[1], "Value", &[], false),
        Some(Ok("-10kΩ".to_string()))
    );
}

/// An export copy stores `-QAC1`: a reference without the sign still finds
/// the part, and one with it is refused.
#[test]
fn an_export_copy_with_signed_designators_still_resolves() {
    let symbols = vec![part("-QAC1", "10kΩ"), part("-QAC2", "= QAC1.Value / 2")];
    assert_eq!(
        part_field_value(&symbols, &symbols[1], "Value", &[], false),
        Some(Ok("5kΩ".to_string()))
    );
    let signed = vec![part("-QAC1", "10kΩ"), part("-QAC2", "= 1 + -QAC1.Value")];
    assert!(matches!(
        part_field_value(&signed, &signed[1], "Value", &[], false),
        Some(Err(ExprError::Domain(_)))
    ));
}

#[test]
fn evaluated_value_keeps_literals_power_ports_and_failures_as_stored() {
    let mut power = part("#PWR1", "= 1V");
    power.is_power = true;
    let symbols = vec![part("R1", "10k"), part("R2", "= R9.Value"), power];
    assert_eq!(evaluated_value(&symbols, &symbols[0], &[], false), "10k");
    assert_eq!(
        evaluated_value(&symbols, &symbols[1], &[], false),
        "= R9.Value"
    );
    assert_eq!(evaluated_value(&symbols, &symbols[2], &[], false), "= 1V");
}
