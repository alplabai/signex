//! #2949, mockup row 59 (d): Compare variants lists only the parts two
//! variants differ on, read as the Variant Manager and the BOM read them.

use signex_types::project::{FOOTPRINT_PARAMETER, VALUE_PARAMETER, VariantPart, VariantSettings};
use signex_types::schematic::{InstancePath, SchematicSheet, Symbol};
use uuid::Uuid;

use super::*;
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

fn entry(designator: &str, fitted: Option<bool>, parameters: &[(&str, &str)]) -> VariantPart {
    VariantPart {
        variant: "Lite".into(),
        designator: designator.into(),
        fitted,
        parameters: parameters
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect(),
        ..VariantPart::default()
    }
}

/// R1 the same everywhere; R2 not fitted in Lite on its sheet; R3 47k in
/// Lite; R4 not fitted in Lite by its settings, which also give it a value;
/// R5 not fitted anywhere; U1 another MPN in Lite.
fn matrix_and_settings() -> (VariantMatrix, VariantSettings) {
    let mut dnp = part("R5", "1k", &[]);
    dnp.dnp = true;
    let symbols = vec![
        part("R1", "10k", &[]),
        part("R2", "10k", &[("Fitted@Lite", "no")]),
        part("R3", "31.6k", &[]),
        part("R4", "1k", &[]),
        dnp,
        part("U1", "MCU", &[("MPN", "PART-A")]),
    ];
    let ctx = ExportContext {
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
        metadata: ProjectMetadata::default(),
        netlist: None,
        netlist_sheets: Vec::new(),
        netlist_placements: Vec::new(),
        current_sheet: None,
    };
    let settings = VariantSettings {
        parts: vec![
            entry("R3", None, &[(VALUE_PARAMETER, "47k")]),
            entry("R4", Some(false), &[(VALUE_PARAMETER, "2k")]),
            entry("U1", None, &[("MPN", "PART-B")]),
        ],
        ..VariantSettings::default()
    };
    (variant_matrix(&ctx, &["Lite".to_string()]), settings)
}

#[test]
fn only_the_parts_that_differ_are_listed_with_what_differs() {
    let (matrix, settings) = matrix_and_settings();
    let compared = matrix
        .compare(&settings, None, Some("Lite"))
        .expect("both variants were read");
    assert_eq!(compared.from, "Base");
    assert_eq!(compared.to, "Lite");
    let listed: Vec<(&str, &[PartChange])> = compared
        .parts
        .iter()
        .map(|part| (part.designator.as_str(), part.changes.as_slice()))
        .collect();
    assert_eq!(
        listed,
        [
            ("R2", &[PartChange::Fit][..]),
            ("R3", &[PartChange::Parameter(VALUE_PARAMETER.into())][..]),
            ("R4", &[PartChange::Fit][..]),
            ("U1", &[PartChange::Parameter("MPN".into())][..]),
        ]
    );
    let r3 = &compared.parts[1];
    assert_eq!(r3.from.parameters[VALUE_PARAMETER], "31.6k");
    assert_eq!(r3.to.parameters[VALUE_PARAMETER], "47k");
    assert_eq!(r3.to.parameters[FOOTPRINT_PARAMETER], "R_0603");
    assert!(!compared.parts[2].to.fitted);
    // Base fits five of the six parts (R5 is DNP); Lite three.
    assert_eq!((compared.from_fitted, compared.to_fitted), (5, 3));
}

#[test]
fn the_sides_swap_and_a_variant_matches_itself() {
    let (matrix, settings) = matrix_and_settings();
    let forward = matrix.compare(&settings, None, Some("Lite")).unwrap();
    let back = matrix
        .compare(&settings, Some("lite"), Some("Base"))
        .unwrap();
    assert_eq!(back.from, "Lite", "the name as read");
    assert_eq!(
        back.parts.iter().map(|p| &p.designator).collect::<Vec<_>>(),
        forward
            .parts
            .iter()
            .map(|p| &p.designator)
            .collect::<Vec<_>>()
    );
    assert_eq!(back.parts[1].from, forward.parts[1].to);
    let same = matrix
        .compare(&settings, Some("Lite"), Some("Lite"))
        .unwrap();
    assert!(same.parts.is_empty());
}

#[test]
fn an_unknown_variant_is_refused_with_its_name() {
    let (matrix, settings) = matrix_and_settings();
    let refused = matrix.compare(&settings, Some("Pro"), None).unwrap_err();
    assert_eq!(refused, CompareProblem::Unknown("Pro".into()));
    assert_eq!(
        refused.to_string(),
        "the project has no variant named `Pro`"
    );
}

#[test]
fn the_comparison_serializes_with_its_changes_tagged() {
    let (matrix, settings) = matrix_and_settings();
    let compared = matrix.compare(&settings, None, Some("Lite")).unwrap();
    let json = serde_json::to_value(&compared).expect("serializes");
    assert_eq!(json["parts"][0]["changes"][0]["kind"], "fit");
    assert_eq!(json["parts"][1]["changes"][0]["kind"], "parameter");
    assert_eq!(json["parts"][1]["changes"][0]["parameter"], "Value");
    assert_eq!(json["to_fitted"], 3);
}
