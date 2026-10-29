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
    app.document_state.mark_documents_clean_where(|_| true);

    submit_rename(&mut app, sheet, "Power2.snxsch");

    let project = &app.document_state.projects[0];
    assert_eq!(project.data.sheets[0].filename, "sub/Power2.snxsch");
    assert_eq!(
        project.data.schematic_root.as_deref(),
        Some("sub/Power2.snxsch")
    );
    assert!(app.document_state.is_document_dirty(&project.path));
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
    assert!(
        app.document_state
            .is_document_dirty(&app.document_state.projects[0].path)
    );

    let data = &app.document_state.projects[0].data;
    assert!(data.sheets.is_empty(), "no dangling row: {:?}", data.sheets);
    assert_eq!(data.schematic_root, None);
    assert!(!sheet.exists());
}

#[test]
fn excluding_a_subdirectory_sheet_drops_its_entry_and_the_root() {
    let (mut app, _tmp, sheet, _pcb) = app_with_subdir_entries();

    confirm_remove(&mut app, sheet.clone(), RemoveChoice::ExcludeFromProject);
    assert!(
        app.document_state
            .is_document_dirty(&app.document_state.projects[0].path)
    );

    let data = &app.document_state.projects[0].data;
    assert!(data.sheets.is_empty());
    assert_eq!(data.schematic_root, None);
    assert!(sheet.exists(), "Exclude leaves the file");
}

#[test]
fn excluding_a_subdirectory_pcb_clears_the_pcb_file() {
    let (mut app, _tmp, _sheet, pcb) = app_with_subdir_entries();

    confirm_remove(&mut app, pcb, RemoveChoice::ExcludeFromProject);
    assert!(
        app.document_state
            .is_document_dirty(&app.document_state.projects[0].path)
    );

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

fn add_sheet(app: &mut Signex, dir: &std::path::Path, sheet: PathBuf, fold: CaseFold) {
    let id = app.document_state.projects[0].id;
    let results = vec![add_io::run_existing(add_io::plan_existing(
        dir, sheet, fold,
    ))];
    let _ = app.update(Message::Project(ProjectMsg::AddExistingFilesCopied {
        project_id: id,
        project_dir: dir.to_path_buf(),
        results: Ok(results),
    }));
}

#[test]
fn add_existing_dedups_a_case_variant_entry_only_on_a_case_insensitive_volume() {
    for (fold, expected_rows) in [
        (CaseFold::from_probe(true), 1),
        (CaseFold::from_probe(false), 2),
    ] {
        let (mut app, tmp, sheet, _pcb) = app_with_subdir_entries();
        app.document_state.projects[0].case_fold = fold;
        app.document_state.projects[0].data.sheets[0].filename = "SUB/POWER.snxsch".to_string();

        add_sheet(&mut app, tmp.path(), sheet, fold);

        assert_eq!(
            app.document_state.projects[0].data.sheets.len(),
            expected_rows,
            "{fold:?}"
        );
    }
}

// POSIX only: on a Windows host `\` is a separator, so the row is the working file.
#[cfg(not(windows))]
#[test]
fn excluding_a_dangling_backslash_row_keeps_the_working_slash_row() {
    // A Windows-written `sub\Power.snxsch` cannot open on this host, so it is
    // not the working `sub/Power.snxsch`.
    let (mut app, _tmp, sheet, _pcb) = app_with_subdir_entries();
    let project = &mut app.document_state.projects[0];
    project.case_fold = CaseFold::from_probe(false);
    project.data.sheets.push(SheetEntry {
        name: "Power".to_string(),
        filename: "sub\\Power.snxsch".to_string(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    });
    let dangling = project.data.dir.clone() + "/sub\\Power.snxsch";

    confirm_remove(
        &mut app,
        PathBuf::from(dangling),
        RemoveChoice::ExcludeFromProject,
    );

    let data = &app.document_state.projects[0].data;
    let files: Vec<_> = data.sheets.iter().map(|s| s.filename.as_str()).collect();
    assert_eq!(files, ["sub/Power.snxsch"]);
    assert_eq!(data.schematic_root.as_deref(), Some("sub/Power.snxsch"));
    assert!(sheet.exists());
}

#[test]
fn matching_uses_the_directory_of_the_opened_project_file_not_a_stale_data_dir() {
    let (mut app, _tmp, sheet, _pcb) = app_with_subdir_entries();
    app.document_state.projects[0].data.dir = "/stale/elsewhere".to_string();

    confirm_remove(&mut app, sheet, RemoveChoice::ExcludeFromProject);

    assert!(app.document_state.projects[0].data.sheets.is_empty());
}

#[test]
fn a_file_outside_the_project_is_refused_not_registered_by_bare_name() {
    let (mut app, _tmp, _sheet, _pcb) = app_with_subdir_entries();
    let elsewhere = tempfile::tempdir().expect("tempdir");
    let outside = elsewhere.path().join("Stray.snxsch");
    std::fs::write(&outside, b"x").expect("outside sheet");
    let id = app.document_state.projects[0].id;

    let _ = app.update(Message::Project(ProjectMsg::AddExistingFilesCopied {
        project_id: id,
        project_dir: std::path::PathBuf::new(),
        results: Ok(vec![Ok(outside)]),
    }));

    let files: Vec<&str> = app.document_state.projects[0]
        .data
        .sheets
        .iter()
        .map(|s| s.filename.as_str())
        .collect();
    assert_eq!(files, ["sub/Power.snxsch"]);
}
