//! #1294 — the symbols a sheet draws as not fitted: the DNP flag, the
//! variant's fit fields and the project's variant settings, as the BOM reads
//! them.

use signex_types::pcb::SymbolLink;
use signex_types::project::{VariantPart, VariantSettings};
use signex_types::schematic::{InstancePath, SchematicSheet, Symbol};
use uuid::Uuid;

use super::not_fitted_symbols;

fn part(reference: &str, unit: u32, fields: &[(&str, &str)]) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "Device:U".to_string(),
        reference: reference.to_string(),
        unit,
        fields: fields
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
        ..Default::default()
    }
}

fn sheet(symbols: Vec<Symbol>) -> SchematicSheet {
    SchematicSheet {
        symbols,
        ..Default::default()
    }
}

fn drawn(sheet: &SchematicSheet, variant: Option<&str>, settings: &VariantSettings) -> Vec<String> {
    let set = not_fitted_symbols(sheet, &InstancePath::root(), variant, settings);
    let mut names: Vec<String> = sheet
        .symbols
        .iter()
        .filter(|sym| set.contains(&sym.uuid))
        .map(|sym| format!("{}.{}", sym.reference, sym.unit))
        .collect();
    names.sort();
    names
}

#[test]
fn a_dnp_part_is_not_fitted_in_base_and_its_other_units_with_it() {
    let mut u1a = part("U1", 1, &[]);
    u1a.dnp = true;
    let s = sheet(vec![u1a, part("U1", 2, &[]), part("U2", 1, &[])]);
    let none = VariantSettings::default();
    assert_eq!(drawn(&s, None, &none), ["U1.1", "U1.2"]);
    assert_eq!(drawn(&s, Some("Base"), &none), ["U1.1", "U1.2"]);
}

#[test]
fn a_variant_fit_field_decides_in_that_variant_only() {
    let s = sheet(vec![
        part("U1", 1, &[("Fitted@Lite", "no")]),
        part("U2", 1, &[]),
    ]);
    let none = VariantSettings::default();
    assert!(drawn(&s, None, &none).is_empty(), "Base fits U1");
    assert_eq!(drawn(&s, Some("Lite"), &none), ["U1.1"]);
}

#[test]
fn the_project_settings_decide_over_the_sheet() {
    let mut fitted_dnp = part("U2", 1, &[]);
    fitted_dnp.dnp = true;
    let s = sheet(vec![part("U1", 1, &[]), fitted_dnp]);
    let settings = VariantSettings {
        parts: vec![
            VariantPart {
                variant: "Lite".into(),
                designator: "U1".into(),
                fitted: Some(false),
                ..VariantPart::default()
            },
            VariantPart {
                variant: "Lite".into(),
                designator: "U2".into(),
                fitted: Some(true),
                ..VariantPart::default()
            },
        ],
        ..VariantSettings::default()
    };
    assert_eq!(drawn(&s, Some("Lite"), &settings), ["U1.1"]);
    assert_eq!(drawn(&s, None, &settings), ["U2.1"], "Base has no settings");
}

#[test]
fn a_setting_keyed_by_the_part_survives_a_re_annotation() {
    let u7 = part("U7", 1, &[]);
    let link = SymbolLink::new(InstancePath::root(), vec![u7.uuid]);
    let s = sheet(vec![u7]);
    let settings = VariantSettings {
        parts: vec![VariantPart {
            variant: "Lite".into(),
            // The designator the part had when the entry was set.
            designator: "U3".into(),
            part: Some(link),
            fitted: Some(false),
            ..VariantPart::default()
        }],
        ..VariantSettings::default()
    };
    assert_eq!(drawn(&s, Some("Lite"), &settings), ["U7.1"]);
}

#[test]
fn a_power_port_is_never_drawn_as_not_fitted() {
    let mut port = part("#PWR01", 1, &[]);
    port.is_power = true;
    port.dnp = true;
    let s = sheet(vec![port]);
    assert!(drawn(&s, None, &VariantSettings::default()).is_empty());
}
