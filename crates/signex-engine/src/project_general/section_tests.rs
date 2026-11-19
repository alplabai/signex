//! #1689-#1693: each section edit is one undo step of the General history,
//! a refused one records nothing, and an undo puts back an order without
//! dropping what another writer added.

use std::path::{Path, PathBuf};

use signex_types::project::{
    AnnotateUndo, LibraryEntry, LibraryEntryKind, ListMove, ProjectData, SheetEntry, VariantProblem,
};

use super::*;

fn project() -> ProjectData {
    let mut data: ProjectData =
        serde_json::from_str(r#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null}"#)
            .unwrap();
    data.sheets = ["a", "b", "c"]
        .iter()
        .map(|name| SheetEntry {
            name: (*name).into(),
            filename: format!("{name}.snxsch"),
            symbols_count: 0,
            wires_count: 0,
            labels_count: 0,
            unknown: Default::default(),
        })
        .collect();
    data.libraries = ["x", "y"]
        .iter()
        .map(|name| LibraryEntry {
            path: PathBuf::from(format!("{name}.snxlib")),
            kind: LibraryEntryKind::ProjectLocal,
            library_id: None,
            unknown: Default::default(),
        })
        .collect();
    data
}

fn apply(
    history: &mut ProjectGeneralHistory,
    data: &mut ProjectData,
    command: ProjectSectionCommand,
) -> Result<bool, VariantProblem> {
    history.apply_section(data, &command)
}

#[test]
fn a_sheet_move_is_one_step_and_undo_keeps_a_sheet_added_since() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    let moved = apply(
        &mut history,
        &mut data,
        ProjectSectionCommand::MoveSheet {
            filename: "c.snxsch".into(),
            step: ListMove::Up,
        },
    );
    assert_eq!(moved, Ok(true));
    assert_eq!(data.sheet_order(), ["a.snxsch", "c.snxsch", "b.snxsch"]);
    // Another writer adds a sheet; the undo keeps it.
    let mut added = data.sheets[0].clone();
    added.filename = "d.snxsch".into();
    data.sheets.push(added);
    assert!(history.undo(&mut data));
    assert_eq!(
        data.sheet_order(),
        ["a.snxsch", "b.snxsch", "c.snxsch", "d.snxsch"]
    );
    assert!(history.redo(&mut data));
    assert_eq!(
        data.sheet_order(),
        ["a.snxsch", "c.snxsch", "b.snxsch", "d.snxsch"]
    );
}

#[test]
fn a_move_at_the_end_records_nothing() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    let moved = apply(
        &mut history,
        &mut data,
        ProjectSectionCommand::MoveLibrary {
            path: PathBuf::from("y.snxlib"),
            step: ListMove::Down,
        },
    );
    assert_eq!(moved, Ok(false));
    assert!(!history.can_undo());
}

#[test]
fn variant_edits_are_steps_and_a_refused_one_records_nothing() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    let add = |name: &str| ProjectSectionCommand::AddVariant { name: name.into() };
    assert_eq!(apply(&mut history, &mut data, add("Lite")), Ok(true));
    assert_eq!(
        apply(&mut history, &mut data, add("lite")),
        Err(VariantProblem::Taken("Lite".into()))
    );
    let active = ProjectSectionCommand::SetActiveVariant {
        name: Some("Lite".into()),
    };
    assert_eq!(apply(&mut history, &mut data, active), Ok(true));
    let rename = ProjectSectionCommand::RenameVariant {
        from: "Lite".into(),
        to: "Mini".into(),
    };
    assert_eq!(apply(&mut history, &mut data, rename), Ok(true));
    assert_eq!(data.active_variant.as_deref(), Some("Mini"));

    // Three steps: the rename, the active pick, the add.
    assert!(history.undo(&mut data));
    assert_eq!(data.variant_definitions, ["Lite"]);
    assert_eq!(data.active_variant.as_deref(), Some("Lite"));
    assert!(history.undo(&mut data));
    assert_eq!(data.active_variant, None);
    assert!(history.undo(&mut data));
    assert!(data.variant_definitions.is_empty());
    assert!(!history.can_undo());
}

#[test]
fn a_section_edit_closes_a_typing_run() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    assert!(history.apply(&mut data, ProjectGeneralCommand::SetDescription("M".into())));
    let choice = ProjectSectionCommand::SetAnnotateUndo {
        choice: Some(AnnotateUndo::PerSheet),
    };
    assert_eq!(apply(&mut history, &mut data, choice), Ok(true));
    assert!(history.apply(
        &mut data,
        ProjectGeneralCommand::SetDescription("Mo".into())
    ));
    assert!(history.undo(&mut data));
    assert_eq!(data.description.as_deref(), Some("M"));
    assert_eq!(data.annotate_undo(), Some(AnnotateUndo::PerSheet));
    assert!(history.undo(&mut data));
    assert_eq!(data.annotate_undo(), None);
}

#[test]
fn version_control_and_library_order_undo() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    let on = ProjectSectionCommand::SetVersionControl { on: true };
    assert_eq!(apply(&mut history, &mut data, on), Ok(true));
    let up = ProjectSectionCommand::MoveLibrary {
        path: PathBuf::from("y.snxlib"),
        step: ListMove::Up,
    };
    assert_eq!(apply(&mut history, &mut data, up), Ok(true));
    assert_eq!(data.libraries[0].path, Path::new("y.snxlib"));
    assert!(history.undo(&mut data));
    assert_eq!(data.libraries[0].path, Path::new("x.snxlib"));
    assert!(data.enable_git);
    assert!(history.undo(&mut data));
    assert!(!data.enable_git);
}

#[test]
fn a_section_command_round_trips_as_json() {
    let command = ProjectSectionCommand::MoveSheet {
        filename: "a.snxsch".into(),
        step: ListMove::Down,
    };
    let text = serde_json::to_string(&command).unwrap();
    assert_eq!(
        text,
        r#"{"edit":"move_sheet","filename":"a.snxsch","step":"down"}"#
    );
    let back: ProjectSectionCommand = serde_json::from_str(&text).unwrap();
    assert_eq!(back, command);
}

#[test]
fn clearing_the_annotate_choice_over_an_unreadable_section_records_nothing() {
    let text = br#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null,"annotation":{"designator_standard":"iso9999"}}"#;
    let mut data = signex_types::project::parse_project_bytes(Path::new("P.snxprj"), text).unwrap();
    assert!(data.has_unreadable_annotation());
    let mut history = ProjectGeneralHistory::default();
    let clear = ProjectSectionCommand::SetAnnotateUndo { choice: None };
    assert_eq!(apply(&mut history, &mut data, clear), Ok(false));
    assert!(
        data.has_unreadable_annotation(),
        "the newer section is kept"
    );
    assert!(!history.can_undo());
    let pick = ProjectSectionCommand::SetAnnotateUndo {
        choice: Some(AnnotateUndo::PerSheet),
    };
    assert_eq!(apply(&mut history, &mut data, pick), Ok(true));
    assert_eq!(data.annotate_undo(), Some(AnnotateUndo::PerSheet));
    assert!(history.undo(&mut data));
    assert!(
        data.has_unreadable_annotation(),
        "undo puts the section back"
    );
}

#[test]
fn a_requirement_edit_is_one_undo_step_and_a_refused_one_records_nothing() {
    use signex_types::project::{RequirementField, RequirementProblem};
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    let add = ProjectRequirementCommand::Add {
        kind: "board_size".into(),
        id: None,
    };
    assert_eq!(history.apply_requirement(&mut data, &add), Ok(true));
    let set = ProjectRequirementCommand::SetField {
        index: 0,
        field: RequirementField::MaxWidthMm,
        text: "50".into(),
    };
    assert_eq!(history.apply_requirement(&mut data, &set), Ok(true));
    let after_set = data.requirements.clone();
    let refused = ProjectRequirementCommand::SetField {
        index: 0,
        field: RequirementField::MaxWidthMm,
        text: "-1".into(),
    };
    assert_eq!(
        history.apply_requirement(&mut data, &refused),
        Err(RequirementProblem::NotPositive("-1".into()))
    );
    assert_eq!(data.requirements, after_set);
    assert!(history.undo(&mut data), "the refused edit recorded no step");
    assert_eq!(
        data.requirements[0].raw()["max_width_mm"],
        serde_json::json!(100)
    );
    assert!(history.undo(&mut data));
    assert!(data.requirements.is_empty());
    assert!(history.redo(&mut data));
    assert!(history.redo(&mut data));
    assert_eq!(data.requirements, after_set);
}

/// #1280: a harness definitions edit is one undo step of the project's
/// history, and the undo puts the old list back.
#[test]
fn a_harness_definitions_edit_is_one_undo_step() {
    use signex_types::schematic::{HarnessDefinition, HarnessDefinitionEntry};
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    let spi = HarnessDefinition {
        name: "SPI_BUS".into(),
        description: "Sensor SPI, mode 0".into(),
        entries: ["SCK", "MOSI"]
            .iter()
            .map(|n| HarnessDefinitionEntry {
                name: (*n).into(),
                harness: None,
            })
            .collect(),
    };
    let set = ProjectSectionCommand::SetHarnessDefinitions {
        definitions: vec![spi.clone()],
    };
    assert_eq!(apply(&mut history, &mut data, set), Ok(true));
    assert_eq!(data.schematic_design.harness_definitions, [spi]);
    assert!(history.undo(&mut data));
    assert!(data.schematic_design.harness_definitions.is_empty());
}
