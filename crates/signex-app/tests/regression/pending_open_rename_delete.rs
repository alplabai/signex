//! #1096 — Rename or Delete of a file whose open is still pending would let
//! the late open result land on a path that has moved or no longer exists,
//! and a save of that tab would write the old path again. Both are refused
//! until the open has finished.

use signex_app::app::{
    LoadedProject, Message, RemoveChoice, RemoveDialogState, RemoveMsg, RenameDialogState,
    RenameMsg, Signex,
};
use signex_types::project::{ProjectData, SheetEntry};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// A project at `dir` whose only sheet is `X.snxsch`, written to disk.
fn project_with_sheet(dir: &Path) -> (Signex, PathBuf) {
    let sheet = dir.join("X.snxsch");
    std::fs::write(&sheet, b"x").expect("write sheet");
    let (mut app, _boot) = Signex::new();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("P.snxprj"),
        data: ProjectData {
            description: None,
            name: "P".to_string(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: Some("X.snxsch".to_string()),
            pcb_file: None,
            sheets: vec![SheetEntry {
                name: "X".to_string(),
                filename: "X.snxsch".to_string(),
                symbols_count: 0,
                wires_count: 0,
                labels_count: 0,
                unknown: Default::default(),
            }],
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            unknown: Default::default(),
        },
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    (app, sheet)
}

fn logged(needle: &str) -> bool {
    let _ = signex_app::diagnostics::init_logging();
    signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains(needle))
}

fn rename_dialog(target: &Path, to: &str) -> RenameDialogState {
    RenameDialogState {
        project_id: None,
        target_path: target.to_path_buf(),
        tree_path: vec![0, 0],
        buffer: to.to_string(),
        error: None,
        is_project_rename: false,
    }
}

fn delete_dialog(target: &Path) -> RemoveDialogState {
    RemoveDialogState {
        project_id: None,
        target_path: target.to_path_buf(),
        tree_path: vec![0, 0],
        display_name: target.file_name().unwrap().to_string_lossy().into_owned(),
        is_outside_project: false,
        unsaved_prompt: false,
    }
}

#[test]
fn renaming_a_file_that_is_still_opening_is_refused() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, sheet) = project_with_sheet(tmp.path());
    app.document_state.pending_opens.insert(sheet.clone());
    app.ui_state.rename_dialog = Some(rename_dialog(&sheet, "Y.snxsch"));

    let _ = app.update(Message::Rename(RenameMsg::Submit));

    assert!(sheet.exists(), "the file was not moved");
    assert!(!tmp.path().join("Y.snxsch").exists());
    let dialog = app
        .ui_state
        .rename_dialog
        .as_ref()
        .expect("the dialog stays open");
    assert!(
        dialog
            .error
            .as_deref()
            .is_some_and(|e| e.contains("still opening"))
    );
    assert!(logged(&format!("Rename of {} refused", sheet.display())));
}

#[test]
fn deleting_a_file_that_is_still_opening_is_refused() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, sheet) = project_with_sheet(tmp.path());
    app.document_state.pending_opens.insert(sheet.clone());
    app.ui_state.remove_dialog = Some(delete_dialog(&sheet));

    let _ = app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::DeleteFile,
    )));

    assert!(sheet.exists(), "the file was not deleted");
    assert!(
        app.ui_state.remove_dialog.is_some(),
        "the dialog stays open"
    );
    assert!(
        app.document_state.error_notice.is_some(),
        "the refusal is shown"
    );
    assert_eq!(app.document_state.projects[0].data.sheets.len(), 1);
}

#[test]
fn deleting_a_folder_with_a_file_still_opening_is_refused() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, _sheet) = project_with_sheet(tmp.path());
    let folder = tmp.path().join("sub");
    std::fs::create_dir(&folder).unwrap();
    let inner = folder.join("Z.snxsch");
    std::fs::write(&inner, b"z").unwrap();
    app.document_state.pending_opens.insert(inner.clone());
    app.ui_state.remove_dialog = Some(delete_dialog(&folder));

    let _ = app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::DeleteFile,
    )));

    assert!(inner.exists(), "nothing under the folder was deleted");
    assert!(app.ui_state.remove_dialog.is_some());
}

#[test]
fn once_the_open_has_finished_the_delete_goes_ahead() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, sheet) = project_with_sheet(tmp.path());
    app.ui_state.remove_dialog = Some(delete_dialog(&sheet));

    // The delete runs on a worker (#859); settle drives its completion.
    crate::settle_support::settle(
        &mut app,
        Message::Remove(RemoveMsg::Confirm(RemoveChoice::DeleteFile)),
    );

    assert!(!sheet.exists(), "no pending open: the delete runs");
}

// ---- #859: a History restore still running owns the file too.

#[test]
fn deleting_a_file_whose_restore_is_still_running_is_refused() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, sheet) = project_with_sheet(tmp.path());
    // A cancelled restore keeps its claim until the worker reports.
    app.document_state.pending_restores.begin(sheet.clone());
    app.document_state.pending_restores.cancel_retaining(&sheet);
    app.ui_state.remove_dialog = Some(delete_dialog(&sheet));

    let _ = app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::DeleteFile,
    )));

    assert!(sheet.exists(), "the file was not deleted");
    assert!(
        app.ui_state.remove_dialog.is_some(),
        "the dialog stays open"
    );
    assert!(logged("A History restore of "), "the refusal is logged");
    assert!(
        app.document_state.error_notice.is_some(),
        "the refusal is shown"
    );
}

#[test]
fn deleting_a_folder_with_a_file_whose_restore_is_running_is_refused() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, _sheet) = project_with_sheet(tmp.path());
    let folder = tmp.path().join("sub");
    std::fs::create_dir(&folder).unwrap();
    let inner = folder.join("Z.snxsch");
    std::fs::write(&inner, b"z").unwrap();
    app.document_state.pending_restores.begin(inner.clone());
    app.ui_state.remove_dialog = Some(delete_dialog(&folder));

    let _ = app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::DeleteFile,
    )));

    assert!(inner.exists(), "nothing under the folder was deleted");
    assert!(app.ui_state.remove_dialog.is_some());
}

#[test]
fn renaming_a_file_whose_restore_is_still_running_is_refused() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, sheet) = project_with_sheet(tmp.path());
    // A restore's atomic write would recreate the old path after the move.
    app.document_state.pending_restores.begin(sheet.clone());
    app.ui_state.rename_dialog = Some(rename_dialog(&sheet, "Y.snxsch"));

    let _ = app.update(Message::Rename(RenameMsg::Submit));

    assert!(sheet.exists(), "the file was not moved");
    assert!(!tmp.path().join("Y.snxsch").exists());
    let dialog = app
        .ui_state
        .rename_dialog
        .as_ref()
        .expect("the dialog stays open");
    assert!(
        dialog
            .error
            .as_deref()
            .is_some_and(|e| e.contains("History restore"))
    );
    assert!(logged(
        "Rename refused: a History restore of X.snxsch is still running."
    ));
}

#[test]
fn renaming_a_project_folder_with_a_restore_still_running_is_refused() {
    let tmp = TempDir::new().expect("tempdir");
    // The folder is named after the project, so the rename would move it.
    let folder = tmp.path().join("P");
    std::fs::create_dir(&folder).expect("project folder");
    let (mut app, sheet) = project_with_sheet(&folder);
    app.document_state.pending_restores.begin(sheet.clone());
    app.ui_state.rename_dialog = Some(RenameDialogState {
        project_id: None,
        target_path: folder.join("P.snxprj"),
        tree_path: vec![0],
        buffer: "Q".to_string(),
        error: None,
        is_project_rename: true,
    });

    let _ = app.update(Message::Rename(RenameMsg::Submit));

    assert!(folder.exists(), "the folder was not moved");
    assert!(!tmp.path().join("Q").exists());
    assert!(
        app.document_state.project_folder_renames.is_empty(),
        "no folder move was started"
    );
    let dialog = app
        .ui_state
        .rename_dialog
        .as_ref()
        .expect("the dialog stays open");
    assert!(
        dialog
            .error
            .as_deref()
            .is_some_and(|e| e.contains("History restore"))
    );
}
