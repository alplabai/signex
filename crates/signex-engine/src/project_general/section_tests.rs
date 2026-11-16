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
