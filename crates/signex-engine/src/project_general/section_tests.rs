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

/// Mockup rows 31 and 33: a drop is one step, however far the row moved.
#[test]
fn a_dropped_row_is_one_undo_step() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    let dropped = apply(
        &mut history,
        &mut data,
        ProjectSectionCommand::PlaceSheet {
            filename: "a.snxsch".into(),
            to: 2,
        },
    );
    assert_eq!(dropped, Ok(true));
    assert_eq!(data.sheet_order(), ["b.snxsch", "c.snxsch", "a.snxsch"]);
    assert!(history.undo(&mut data));
    assert_eq!(data.sheet_order(), ["a.snxsch", "b.snxsch", "c.snxsch"]);
    assert!(!history.can_undo());
    let same = apply(
        &mut history,
        &mut data,
        ProjectSectionCommand::PlaceLibrary {
            path: PathBuf::from("x.snxlib"),
            to: 0,
        },
    );
    assert_eq!(same, Ok(false), "a drop on its own place records nothing");
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

/// DECISIONS 2026-11-23 (#2750): every edit of an unreadable
/// `schematic_design` section changes nothing and records no undo step.
#[test]
fn an_unreadable_schematic_design_section_refuses_every_edit() {
    let text = br#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null,"schematic_design":"garbled"}"#;
    let mut data =
        signex_types::project::parse_project_bytes(Path::new("P.snxprj"), text, |_| false).unwrap();
    assert!(data.has_unreadable_schematic_design());
    let mut history = ProjectGeneralHistory::default();
    let naming = signex_types::project::ChannelNaming {
        designator_format: "${Designator}${Channel}".into(),
        net_format: "${Channel}_${Net}".into(),
    };
    let set_naming = ProjectSectionCommand::SetChannelNaming { naming };
    assert_eq!(apply(&mut history, &mut data, set_naming), Ok(false));
    let definition = signex_types::schematic::HarnessDefinition {
        name: "UART".into(),
        description: String::new(),
        entries: Vec::new(),
    };
    let set_definitions = ProjectSectionCommand::SetHarnessDefinitions {
        definitions: vec![definition],
    };
    assert_eq!(apply(&mut history, &mut data, set_definitions), Ok(false));
    let classes = vec![signex_types::diff_pair_class::DiffPairClass::named("USB")];
    let set_classes = ProjectGeneralCommand::SetDiffPairClasses(classes);
    assert!(!history.apply(&mut data, set_classes));
    assert!(data.schematic_design.is_default());
    assert!(!history.can_undo());
}

#[test]
fn clearing_the_annotate_choice_over_an_unreadable_section_records_nothing() {
    let text = br#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null,"annotation":{"designator_standard":"iso9999"}}"#;
    let mut data =
        signex_types::project::parse_project_bytes(Path::new("P.snxprj"), text, |_| false).unwrap();
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

#[test]
fn a_checklist_edit_is_one_undo_step_and_a_refused_one_records_nothing() {
    use signex_types::project::{ChecklistProblem, EditStamp, ItemState};
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    let add = ProjectChecklistCommand::AddItem {
        stage: "schematic".into(),
        title: "ERC clean".into(),
        stamp: EditStamp::on("2026-11-26"),
    };
    assert_eq!(history.apply_checklist(&mut data, &add), Ok(true));
    let done = ProjectChecklistCommand::SetState {
        item: "1".into(),
        state: ItemState::Done,
        reason: None,
        stamp: EditStamp::default(),
    };
    assert_eq!(history.apply_checklist(&mut data, &done), Ok(true));
    let after_done = data.design_checklist_value().cloned();
    let refused = ProjectChecklistCommand::SetState {
        item: "1".into(),
        state: ItemState::Waived,
        reason: None,
        stamp: EditStamp::default(),
    };
    assert_eq!(
        history.apply_checklist(&mut data, &refused),
        Err(ChecklistProblem::NeedsReason)
    );
    assert_eq!(data.design_checklist_value().cloned(), after_done);
    assert_eq!(history.apply_checklist(&mut data, &done), Ok(false));
    assert!(history.undo(&mut data), "the refused edit recorded no step");
    let checklist = data.design_checklist().unwrap().unwrap();
    assert_eq!(checklist.items[0].state, ItemState::Open);
    assert!(history.undo(&mut data));
    assert_eq!(data.design_checklist_value(), None);
    assert!(history.redo(&mut data));
    assert!(history.redo(&mut data));
    assert_eq!(data.design_checklist_value().cloned(), after_done);
}

#[test]
fn a_checklist_import_and_a_reset_are_one_undo_step_each() {
    use signex_types::project::{ChecklistStage, EditStamp, NewChecklistItem};
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    let item = |title: &str| NewChecklistItem {
        stage: ChecklistStage::Layout,
        title: title.into(),
        link: None,
    };
    let import = ProjectChecklistCommand::Import {
        from: "buck-3v3".into(),
        template: Some("release_review".into()),
        items: vec![item("Hot loop"), item("Clearance")],
        stamp: EditStamp::default(),
    };
    assert_eq!(history.apply_checklist(&mut data, &import), Ok(true));
    let imported = data.design_checklist_value().cloned();
    let reset = ProjectChecklistCommand::Reset {
        template: "blank".into(),
        items: Vec::new(),
        stamp: EditStamp::default(),
    };
    assert_eq!(history.apply_checklist(&mut data, &reset), Ok(true));
    let checklist = data.design_checklist().unwrap().unwrap();
    assert!(checklist.items.is_empty());
    assert!(history.undo(&mut data));
    assert_eq!(data.design_checklist_value().cloned(), imported);
    assert!(history.undo(&mut data));
    assert_eq!(data.design_checklist_value(), None);
}

/// #1292: the numbering scheme is one General step; Continuous over an
/// unreadable section records nothing and keeps it.
#[test]
fn the_numbering_scheme_is_one_step_and_keeps_an_unreadable_section() {
    use signex_types::designator::NumberingScheme;
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    let pick = ProjectSectionCommand::SetAnnotateNumbering {
        scheme: NumberingScheme::PageHundreds,
    };
    assert_eq!(apply(&mut history, &mut data, pick.clone()), Ok(true));
    assert_eq!(data.annotate_numbering(), NumberingScheme::PageHundreds);
    assert_eq!(apply(&mut history, &mut data, pick), Ok(false), "no change");
    assert!(history.undo(&mut data));
    assert_eq!(data.annotate_numbering(), NumberingScheme::Continuous);
    assert!(data.annotation.is_none());

    let text = br#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null,"annotation":{"numbering":"per_sheet_start"}}"#;
    let mut data =
        signex_types::project::parse_project_bytes(Path::new("P.snxprj"), text, |_| false).unwrap();
    let mut history = ProjectGeneralHistory::default();
    let continuous = ProjectSectionCommand::SetAnnotateNumbering {
        scheme: NumberingScheme::Continuous,
    };
    assert_eq!(apply(&mut history, &mut data, continuous), Ok(false));
    assert!(data.has_unreadable_annotation());
    assert!(!history.can_undo());
}

/// #2943: a rails edit is one undo step, a refused one records nothing, and
/// the undo puts the stored rails back.
#[test]
fn a_rail_edit_is_one_undo_step_and_a_refused_one_records_nothing() {
    use signex_types::project::{RailEdit, RailProblem};
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    let set = |voltage: &str| ProjectRailCommand {
        net: "+3V3".into(),
        edit: RailEdit {
            voltage: Some(voltage.into()),
            ..RailEdit::default()
        },
    };
    assert_eq!(history.apply_rail(&mut data, &set("3.3")), Ok(true));
    assert_eq!(history.apply_rail(&mut data, &set("3.3")), Ok(false));
    assert_eq!(
        history.apply_rail(&mut data, &set("0")),
        Err(RailProblem::NotAVoltage("0".into()))
    );
    assert_eq!(data.rails().unwrap()["+3V3"].voltage, 3.3);
    assert!(history.undo(&mut data), "one step for the one change");
    assert_eq!(data.rails_value(), None);
    assert!(
        !history.undo(&mut data),
        "the no-op and the refusal recorded none"
    );
    assert!(history.redo(&mut data));
    assert_eq!(data.rails().unwrap()["+3V3"].voltage, 3.3);
}
