//! Mockup row 32: the project's variant settings decide over the sheets, the
//! same way for the matrix and the BOM.

use signex_types::project::{VALUE_PARAMETER, VariantPart, VariantSettings};
use signex_types::schematic::{InstancePath, SchematicSheet, Symbol};
use uuid::Uuid;

use super::*;
use crate::bom::{BomOptions, rollup};
use crate::{ExportContext, ProjectMetadata, SheetSnapshot};

fn part(reference: &str, value: &str, fields: &[(&str, &str)]) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: value.to_string(),
        footprint: "R_0603".to_string(),
        fields: fields
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
        ..Default::default()
    }
}

fn context(symbols: Vec<Symbol>, settings: VariantSettings) -> ExportContext {
    ExportContext {
        sheets: vec![SheetSnapshot {
            path: "main.snxsch".into(),
            schematic: SchematicSheet {
                symbols,
                ..Default::default()
            },
            sheet_name: "Main".to_string(),
            sheet_number: 1,
            sheet_count: 1,
            placement: InstancePath::root(),
        }],
        metadata: ProjectMetadata {
            variant_settings: settings,
            ..ProjectMetadata::default()
        },
        netlist: None,
        netlist_sheets: Vec::new(),
        netlist_placements: Vec::new(),
        current_sheet: None,
    }
}

/// Lite does not fit R1 and gives R2 the value 47k; R3 is not fitted in
/// Lite on its sheet.
fn lite() -> VariantSettings {
    VariantSettings {
        default: None,
        parts: vec![
            VariantPart {
                variant: "Lite".into(),
                designator: "R1".into(),
                fitted: Some(false),
                ..VariantPart::default()
            },
            VariantPart {
                variant: "Lite".into(),
                designator: "R2".into(),
                parameters: [(VALUE_PARAMETER.to_string(), "47k".to_string())].into(),
                ..VariantPart::default()
            },
        ],
    }
}

fn symbols() -> Vec<Symbol> {
    vec![
        part("R1", "1k", &[("Tolerance", "1%")]),
        part("R2", "31.6k", &[]),
        part("R3", "10k", &[("Fitted@Lite", "no")]),
    ]
}

#[test]
fn the_matrix_takes_a_variants_own_fit_and_marks_its_own_values() {
    let read = variant_matrix(&context(symbols(), lite()), &["Lite".to_string()]);
    // The read is what the sheets say; the settings are applied on top.
    assert_eq!(read.rows[0].fitted, [true, true]);
    assert_eq!(
        read.rows[0].parameters,
        [("Tolerance".to_string(), "1%".to_string())]
    );
    let shown = read.with_settings(&lite());
    let cells: Vec<(&str, &[bool], &[bool])> = shown
        .rows
        .iter()
        .map(|row| {
            (
                row.designator.as_str(),
                row.fitted.as_slice(),
                row.overridden.as_slice(),
            )
        })
        .collect();
    assert_eq!(
        cells,
        [
            ("R1", &[true, false][..], &[false, false][..]),
            ("R2", &[true, true][..], &[false, true][..]),
            ("R3", &[true, false][..], &[false, false][..]),
        ]
    );
    assert_eq!(shown.changes(1), 3, "a value of its own is a change");
    assert_eq!(shown.fitted_count(1), 1);
    assert!(shown.rows[1].differs());
}

#[test]
fn the_bom_takes_the_same_fit_and_values_as_the_matrix() {
    let ctx = context(symbols(), lite());
    let lite_bom = rollup(
        &ctx,
        &BomOptions {
            active_variant: Some("Lite".into()),
            ..BomOptions::default()
        },
    );
    let fitted: Vec<(Vec<String>, String)> = lite_bom
        .rows
        .iter()
        .filter(|row| row.fitted_qty > 0)
        .map(|row| (row.references.clone(), row.value.clone()))
        .collect();
    assert_eq!(fitted, [(vec!["R2".to_string()], "47k".to_string())]);

    // Base lists every part with its own value.
    let base = rollup(&ctx, &BomOptions::default());
    let mut values: Vec<String> = base.rows.iter().map(|row| row.value.clone()).collect();
    values.sort();
    assert_eq!(values, ["10k", "1k", "31.6k"]);
}

#[test]
fn base_never_takes_a_setting() {
    assert_eq!(project_fit(&lite(), None, "R1"), None);
    assert_eq!(project_fit(&lite(), Some("Base"), "R1"), None);
    assert_eq!(project_fit(&lite(), Some("lite"), "R1"), Some(false));
    assert_eq!(project_fit(&lite(), Some("Lite"), "R2"), None);
}
