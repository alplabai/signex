//! Mockup row 32: the Variant Manager's edits are section commands. Each
//! changed cell, override, duplicate and default is one undo step of the
//! General history; a refused one records nothing.

use std::collections::BTreeMap;

use signex_types::project::{ProjectData, VALUE_PARAMETER, VariantProblem};

use super::*;

fn project() -> ProjectData {
    let mut data: ProjectData =
        serde_json::from_str(r#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null}"#)
            .unwrap();
    data.variant_definitions = vec!["Lite".into(), "Pro".into()];
    data
}

fn apply(
    history: &mut ProjectGeneralHistory,
    data: &mut ProjectData,
    command: ProjectSectionCommand,
) -> Result<bool, VariantProblem> {
    history.apply_section(data, &command)
}

fn fit(variant: &str, designator: &str, fitted: Option<bool>) -> ProjectSectionCommand {
    ProjectSectionCommand::SetVariantPartFit {
        variant: variant.into(),
        designator: designator.into(),
        fitted,
    }
}

#[test]
fn each_cell_change_is_one_undo_step() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    assert_eq!(
        apply(&mut history, &mut data, fit("Lite", "J3", Some(false))),
        Ok(true)
    );
    let value = ProjectSectionCommand::SetVariantPartParameter {
        variant: "Lite".into(),
        designator: "R5".into(),
        parameter: VALUE_PARAMETER.into(),
        value: Some("47k".into()),
    };
    assert_eq!(apply(&mut history, &mut data, value.clone()), Ok(true));
    // The same value again changes nothing and records nothing.
    assert_eq!(apply(&mut history, &mut data, value), Ok(false));
    let settings = &data.variant_settings;
    assert_eq!(
        settings.part(Some("Lite"), "J3").unwrap().fitted,
        Some(false)
    );

    assert!(history.undo(&mut data));
    assert!(data.variant_settings.part(Some("Lite"), "R5").is_none());
    assert!(data.variant_settings.part(Some("Lite"), "J3").is_some());
    assert!(history.undo(&mut data));
    assert!(data.variant_settings.is_empty());
    assert!(!history.can_undo());
    assert!(history.redo(&mut data));
    assert_eq!(
        data.variant_settings
            .part(Some("Lite"), "J3")
            .unwrap()
            .fitted,
        Some(false)
    );
}

#[test]
fn a_refused_cell_change_records_nothing() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    assert_eq!(
        apply(&mut history, &mut data, fit("Base", "J3", Some(false))),
        Err(VariantProblem::Reserved)
    );
    assert_eq!(
        apply(&mut history, &mut data, fit("Max", "J3", Some(false))),
        Err(VariantProblem::Unknown("Max".into()))
    );
    assert!(!history.can_undo());
    assert!(data.variant_settings.is_empty());
}

#[test]
fn a_duplicate_and_a_default_are_steps_and_undo_puts_both_back() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    apply(&mut history, &mut data, fit("Lite", "U3", Some(true))).unwrap();
    let duplicate = ProjectSectionCommand::DuplicateVariant {
        from: "Lite".into(),
        name: "Lite copy".into(),
        sheet_fits: BTreeMap::from([("D3".to_string(), false)]),
    };
    assert_eq!(apply(&mut history, &mut data, duplicate), Ok(true));
    assert_eq!(data.variant_definitions, ["Lite", "Pro", "Lite copy"]);
    let default = ProjectSectionCommand::SetDefaultVariant {
        name: Some("Lite copy".into()),
    };
    assert_eq!(apply(&mut history, &mut data, default), Ok(true));
    assert_eq!(data.default_variant(), Some("Lite copy"));

    assert!(history.undo(&mut data));
    assert_eq!(data.default_variant(), None);
    assert!(history.undo(&mut data));
    assert_eq!(data.variant_definitions, ["Lite", "Pro"]);
    assert!(
        data.variant_settings
            .part(Some("Lite copy"), "D3")
            .is_none(),
        "the copy's settings go with it"
    );
    assert!(data.variant_settings.part(Some("Lite"), "U3").is_some());
}

#[test]
fn a_removal_takes_the_settings_and_undo_brings_them_back() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    apply(&mut history, &mut data, fit("Pro", "C22", Some(false))).unwrap();
    let remove = ProjectSectionCommand::RemoveVariant { name: "Pro".into() };
    assert_eq!(apply(&mut history, &mut data, remove), Ok(true));
    assert!(data.variant_settings.is_empty());
    assert!(history.undo(&mut data));
    assert_eq!(
        data.variant_settings
            .part(Some("Pro"), "C22")
            .unwrap()
            .fitted,
        Some(false)
    );
}

#[test]
fn a_variant_command_reads_back_from_its_json() {
    let command = ProjectSectionCommand::DuplicateVariant {
        from: "Lite".into(),
        name: "Lite copy".into(),
        sheet_fits: BTreeMap::new(),
    };
    let json = serde_json::to_value(&command).unwrap();
    assert_eq!(json["edit"], "duplicate_variant");
    let back: ProjectSectionCommand = serde_json::from_value(
        serde_json::json!({"edit": "duplicate_variant", "from": "Lite", "name": "Lite copy"}),
    )
    .unwrap();
    assert_eq!(back, command);
}
