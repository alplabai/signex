//! #1294 — the symbols a sheet draws as not fitted: the DNP flag, the
//! variant's fit fields and the project's variant settings, as the BOM reads
//! them.

use std::collections::HashSet;

use signex_types::pcb::SymbolLink;
use signex_types::project::{VariantPart, VariantSettings};
use signex_types::schematic::{InstancePath, SchematicSheet, Symbol};
use uuid::Uuid;

use super::{PlacedFits, not_fitted_symbols};

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

/// A dual part U1 with unit 1 on a power sheet and unit 2 on an amplifier
/// sheet, unit 1 marked DNP when `dnp`.
fn split_part(dnp: bool) -> (SchematicSheet, SchematicSheet) {
    let mut a = part("U1", 1, &[]);
    a.dnp = dnp;
    (sheet(vec![a]), sheet(vec![part("U1", 2, &[])]))
}

fn power() -> InstancePath {
    InstancePath::from_chain(vec![Uuid::from_u128(0xA)])
}

fn amp() -> InstancePath {
    InstancePath::from_chain(vec![Uuid::from_u128(0xB)])
}

#[test]
fn a_dnp_unit_on_one_sheet_makes_the_unit_on_another_sheet_not_fitted() {
    let (p, a) = split_part(true);
    let none = VariantSettings::default();
    let fits = PlacedFits::of(&[(&p, power()), (&a, amp())], None, &none);
    let b = a.symbols[0].uuid;
    assert_eq!(fits.fit_at(&amp(), b), Some(false), "the BOM drops U1");
    assert_eq!(
        fits.not_fitted_on(&a, &amp(), None, &none),
        HashSet::from([b])
    );
    assert!(
        not_fitted_symbols(&a, &amp(), None, &none).is_empty(),
        "the sheet alone does not know unit 1"
    );
}

#[test]
fn a_setting_linked_to_the_first_units_sheet_reaches_the_other_sheet() {
    let (p, a) = split_part(false);
    // The Variant Manager links the part by its first unit's placement.
    let link = SymbolLink::new(power(), vec![p.symbols[0].uuid]);
    let settings = VariantSettings {
        parts: vec![VariantPart {
            variant: "Lite".into(),
            designator: "U1".into(),
            part: Some(link),
            fitted: Some(false),
            ..VariantPart::default()
        }],
        ..VariantSettings::default()
    };
    let placements = [(&p, power()), (&a, amp())];
    let fits = PlacedFits::of(&placements, Some("Lite"), &settings);
    let b = a.symbols[0].uuid;
    assert_eq!(
        fits.not_fitted_on(&a, &amp(), Some("Lite"), &settings),
        HashSet::from([b])
    );
    let base = PlacedFits::of(&placements, None, &settings);
    assert_eq!(base.fit_at(&amp(), b), Some(true), "Base has no settings");
}

#[test]
fn a_repeated_sheet_draws_a_part_crossed_only_when_every_channel_leaves_it_out() {
    let r5 = part("R5", 1, &[]);
    let s = sheet(vec![r5.clone()]);
    let ch1 = InstancePath::from_chain(vec![Uuid::from_u128(0xC1)]);
    let ch2 = InstancePath::from_chain(vec![Uuid::from_u128(0xC2)]);
    let entry = |channel: &InstancePath| VariantPart {
        variant: "Lite".into(),
        designator: "R5".into(),
        part: Some(SymbolLink::new(channel.clone(), vec![r5.uuid])),
        fitted: Some(false),
        ..VariantPart::default()
    };
    let one = VariantSettings {
        parts: vec![entry(&ch1)],
        ..VariantSettings::default()
    };
    let placements = [(&s, ch1.clone()), (&s, ch2.clone())];
    let channels = [ch1.clone(), ch2.clone()];
    let mixed = PlacedFits::of(&placements, Some("Lite"), &one);
    assert_eq!(mixed.fit_at(&ch1, r5.uuid), Some(false));
    assert!(
        mixed.not_fitted_in_every(&s, &channels).is_empty(),
        "fitted in CH2, so drawn fitted"
    );
    let both = VariantSettings {
        parts: vec![entry(&ch1), entry(&ch2)],
        ..VariantSettings::default()
    };
    let out = PlacedFits::of(&placements, Some("Lite"), &both);
    assert_eq!(
        out.not_fitted_in_every(&s, &channels),
        HashSet::from([r5.uuid])
    );
}

#[test]
fn the_drawing_and_the_bom_agree_on_a_part_split_across_sheets() {
    use crate::bom::{BomOptions, rollup};
    let (p, a) = split_part(true);
    let ctx = crate::ExportContext {
        sheets: Vec::new(),
        metadata: crate::ProjectMetadata::default(),
        netlist: None,
        netlist_sheets: vec![p, a],
        netlist_placements: vec![
            signex_types::net::SheetPlacement {
                sheet_index: 0,
                instance_path: power(),
            },
            signex_types::net::SheetPlacement {
                sheet_index: 1,
                instance_path: amp(),
            },
        ],
        current_sheet: None,
    };
    let listed = BomOptions {
        include_dnp: true,
        include_not_fitted: true,
        ..BomOptions::default()
    };
    let table = rollup(&ctx, &listed);
    assert_eq!(table.rows.len(), 1, "one part U1 across both sheets");
    assert_eq!(table.rows[0].not_fitted_qty, 1);
    assert_eq!(table.rows[0].fitted_qty, 0);
    let fits = PlacedFits::of_context(&ctx, None);
    for (sheet, path) in [(0, power()), (1, amp())] {
        let unit = ctx.netlist_sheets[sheet].symbols[0].uuid;
        assert_eq!(fits.fit_at(&path, unit), Some(false));
    }
}

#[test]
fn a_power_port_is_never_drawn_as_not_fitted() {
    let mut port = part("#PWR01", 1, &[]);
    port.is_power = true;
    port.dnp = true;
    let s = sheet(vec![port]);
    assert!(drawn(&s, None, &VariantSettings::default()).is_empty());
}
