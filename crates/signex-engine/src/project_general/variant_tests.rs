//! Mockup row 32: the Variant Manager's edits are section commands. Each
//! changed cell, override, duplicate and default is one undo step of the
//! General history; a refused one records nothing.

use signex_types::project::{ProjectData, SheetFit, VALUE_PARAMETER, VariantProblem};

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
        part: None,
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
        part: None,
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
        sheet_fits: vec![SheetFit {
            designator: "D3".into(),
            part: None,
            fitted: false,
        }],
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
        sheet_fits: Vec::new(),
    };
    let json = serde_json::to_value(&command).unwrap();
    assert_eq!(json["edit"], "duplicate_variant");
    let back: ProjectSectionCommand = serde_json::from_value(
        serde_json::json!({"edit": "duplicate_variant", "from": "Lite", "name": "Lite copy"}),
    )
    .unwrap();
    assert_eq!(back, command);
}

/// A cell edit names its placed part: the setting is stored on it, the
/// command reads back from its JSON with the link, and undo takes it away.
#[test]
fn a_cell_edit_on_a_placed_part_keeps_the_link() {
    use signex_types::pcb::SymbolLink;
    use signex_types::schematic::InstancePath;

    let link = SymbolLink::new(InstancePath::root(), [uuid::Uuid::now_v7()]);
    let command = ProjectSectionCommand::SetVariantPartFit {
        variant: "Lite".into(),
        designator: "R5".into(),
        part: Some(link.clone()),
        fitted: Some(false),
    };
    let json = serde_json::to_value(&command).unwrap();
    let back: ProjectSectionCommand = serde_json::from_value(json).unwrap();
    assert_eq!(back, command);

    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    assert_eq!(apply(&mut history, &mut data, command), Ok(true));
    assert_eq!(data.variant_settings.parts[0].part.as_ref(), Some(&link));
    assert!(history.undo(&mut data));
    assert!(data.variant_settings.is_empty());
}

/// #2662: every channel's copy of a part, set in one command, is one undo
/// step; the command reads back from its JSON.
#[test]
fn every_channel_copy_set_at_once_is_one_undo_step() {
    use signex_types::pcb::SymbolLink;
    use signex_types::project::PartRef;
    use signex_types::schematic::InstancePath;

    let unit = uuid::Uuid::now_v7();
    let copy = |n: u128| {
        let path = InstancePath::from_chain(vec![
            uuid::Uuid::from_u128(0x2662),
            uuid::Uuid::from_u128(0x2662 + n),
        ]);
        PartRef::new(format!("CH{n}_R5"), Some(SymbolLink::new(path, [unit])))
    };
    let command = ProjectSectionCommand::SetVariantPartsFit {
        variant: "Lite".into(),
        parts: vec![copy(1), copy(2), copy(3)],
        fitted: Some(false),
    };
    let json = serde_json::to_value(&command).unwrap();
    let back: ProjectSectionCommand = serde_json::from_value(json).unwrap();
    assert_eq!(back, command);

    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    assert_eq!(apply(&mut history, &mut data, command.clone()), Ok(true));
    assert_eq!(data.variant_settings.parts.len(), 3);
    // The same fit again changes nothing and records nothing.
    assert_eq!(apply(&mut history, &mut data, command), Ok(false));
    assert!(history.undo(&mut data));
    assert!(data.variant_settings.is_empty(), "one step takes all three");
    assert!(!history.undo(&mut data));
}
