//! #1367 — a sheet or PCB registered from a project subdirectory is listed as
//! `sub/Power.snxsch`; Rename, Remove and Add Existing must find it by its
//! resolved path, not by bare file name.

use std::path::PathBuf;

use super::add_io;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::scope::CaseFold;
use crate::app::{
    Message, ProjectMsg, RemoveChoice, RemoveDialogState, RemoveMsg, RenameDialogState, RenameMsg,
    Signex,
};
use signex_types::project::SheetEntry;

/// An app whose project lists `sub/Power.snxsch` (also the root) and
/// `sub/Board.snxpcb`, both present on disk under a temp dir.
fn app_with_subdir_entries() -> (Signex, tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    std::fs::create_dir_all(dir.join("sub")).expect("subdir");
    let sheet = dir.join("sub").join("Power.snxsch");
    let pcb = dir.join("sub").join("Board.snxpcb");
    std::fs::write(&sheet, b"x").expect("sheet");
    std::fs::write(&pcb, b"x").expect("pcb");
    let mut app = app_workspace(&dir.to_string_lossy(), &["sub/Power.snxsch"]);
    app.document_state.projects[0].data.pcb_file = Some("sub/Board.snxpcb".to_string());
    (app, tmp, sheet, pcb)
}

fn submit_rename(app: &mut Signex, target: PathBuf, new_name: &str) {
    let project_id = Some(app.document_state.projects[0].id);
    app.ui_state.rename_dialog = Some(RenameDialogState {
        target_path: target,
        tree_path: vec![0],
        project_id,
        buffer: new_name.to_string(),
        error: None,
        is_project_rename: false,
    });
    let _ = app.update(Message::Rename(RenameMsg::Submit));
}

fn confirm_remove(app: &mut Signex, target: PathBuf, choice: RemoveChoice) {
    let project_id = Some(app.document_state.projects[0].id);
    app.ui_state.remove_dialog = Some(RemoveDialogState {
        target_path: target,
        tree_path: vec![0],
        project_id,
        display_name: String::new(),
        is_outside_project: false,
        unsaved_prompt: false,
    });
    let _ = app.update(Message::Remove(RemoveMsg::Confirm(choice)));
}

#[test]
fn renaming_a_subdirectory_sheet_keeps_its_prefix_and_dirties_the_project() {
    let (mut app, _tmp, sheet, _pcb) = app_with_subdir_entries();
    app.document_state.dirty_paths.clear();

    submit_rename(&mut app, sheet, "Power2.snxsch");

    let project = &app.document_state.projects[0];
    assert_eq!(project.data.sheets[0].filename, "sub/Power2.snxsch");
    assert_eq!(
        project.data.schematic_root.as_deref(),
        Some("sub/Power2.snxsch")
    );
    assert!(app.document_state.dirty_paths.contains(&project.path));
}

#[test]
fn renaming_a_subdirectory_pcb_keeps_its_prefix() {
    let (mut app, _tmp, _sheet, pcb) = app_with_subdir_entries();

    submit_rename(&mut app, pcb, "Board2.snxpcb");

    let data = &app.document_state.projects[0].data;
    assert_eq!(data.pcb_file.as_deref(), Some("sub/Board2.snxpcb"));
}

#[test]
fn deleting_a_subdirectory_sheet_drops_its_entry_and_the_root() {
    let (mut app, _tmp, sheet, _pcb) = app_with_subdir_entries();

    confirm_remove(&mut app, sheet.clone(), RemoveChoice::DeleteFile);

    let data = &app.document_state.projects[0].data;
    assert!(data.sheets.is_empty(), "no dangling row: {:?}", data.sheets);
    assert_eq!(data.schematic_root, None);
    assert!(!sheet.exists());
}

#[test]
fn excluding_a_subdirectory_sheet_drops_its_entry_and_the_root() {
    let (mut app, _tmp, sheet, _pcb) = app_with_subdir_entries();

    confirm_remove(&mut app, sheet.clone(), RemoveChoice::ExcludeFromProject);

    let data = &app.document_state.projects[0].data;
    assert!(data.sheets.is_empty());
    assert_eq!(data.schematic_root, None);
    assert!(sheet.exists(), "Exclude leaves the file");
}

#[test]
fn excluding_a_subdirectory_pcb_clears_the_pcb_file() {
    let (mut app, _tmp, _sheet, pcb) = app_with_subdir_entries();

    confirm_remove(&mut app, pcb, RemoveChoice::ExcludeFromProject);

    assert_eq!(app.document_state.projects[0].data.pcb_file, None);
}

#[test]
fn removing_a_same_named_root_sheet_leaves_the_subdirectory_one() {
    let (mut app, tmp, _sheet, _pcb) = app_with_subdir_entries();
    let root_sheet = tmp.path().join("Power.snxsch");
    std::fs::write(&root_sheet, b"x").expect("root sheet");
    app.document_state.projects[0].data.sheets.push(SheetEntry {
        name: "Power".to_string(),
        filename: "Power.snxsch".to_string(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    });

    confirm_remove(&mut app, root_sheet, RemoveChoice::ExcludeFromProject);

    let files: Vec<_> = app.document_state.projects[0]
        .data
        .sheets
        .iter()
        .map(|s| s.filename.as_str())
        .collect();
    assert_eq!(files, ["sub/Power.snxsch"]);
}

#[test]
fn add_existing_does_not_list_a_subdirectory_sheet_twice_under_another_spelling() {
    let (mut app, tmp, sheet, _pcb) = app_with_subdir_entries();
    let id = app.document_state.projects[0].id;
    // The entry is the hand-edited backslash spelling of the same file.
    app.document_state.projects[0].data.sheets[0].filename = "sub\\Power.snxsch".to_string();

    let dir = tmp.path().to_path_buf();
    let results = vec![add_io::run_existing(add_io::plan_existing(
        &dir,
        sheet,
        CaseFold::default(),
    ))];
    let _ = app.update(Message::Project(ProjectMsg::AddExistingFilesCopied {
        project_id: id,
        results: Ok(results),
    }));

    assert_eq!(app.document_state.projects[0].data.sheets.len(), 1);
}
