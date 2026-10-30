//! Rename Project that moves only the `.snxprj` (the folder keeps its name)
//! runs the move on a worker (#859): nothing moves in `update`, the paths are
//! held until it reports, and a failure or a closed project is reported.

use std::path::{Path, PathBuf};

use super::project_rename_tests::{Fixture, fixture, run_task, submit_rename};
use crate::app::Message;
use crate::persist::run_task::outputs;

/// A project whose folder is not named after it, so only the file moves.
fn file_only(root: &Path) -> (Fixture, PathBuf) {
    let mut fx = fixture(root, "Foo", false);
    let renamed = root.join("work");
    std::fs::rename(&fx.folder, &renamed).expect("move folder");
    let old_file = renamed.join("Foo.snxprj");
    let project = &mut fx.app.document_state.projects[0];
    project.path = old_file.clone();
    project.data.dir = renamed.to_string_lossy().into_owned();
    (fx, old_file)
}

fn completion(task: iced::Task<Message>) -> Message {
    let mut messages = outputs(task);
    assert_eq!(messages.len(), 1, "one completion per worker");
    messages.remove(0)
}

#[test]
fn the_file_moves_when_the_worker_runs_not_in_update() {
    let root = tempfile::tempdir().expect("tempdir");
    let (mut fx, old_file) = file_only(root.path());
    let new_file = old_file.with_file_name("Bar.snxprj");

    let task = submit_rename(&mut fx.app, &old_file, "Bar");

    assert!(old_file.exists() && !new_file.exists(), "update moved it");
    assert_eq!(fx.app.document_state.projects[0].path, old_file);
    assert!(fx.app.ui_state.rename_dialog.is_some(), "dialog stays open");
    let done = completion(task);
    assert!(new_file.exists() && !old_file.exists());
    assert_eq!(
        fx.app.document_state.projects[0].path, old_file,
        "memory follows only the completion"
    );
    run_task(&mut fx.app, iced::Task::done(done));
    assert_eq!(fx.app.document_state.projects[0].path, new_file);
    assert!(fx.app.ui_state.rename_dialog.is_none());
    assert!(fx.app.document_state.pending_path_ops.is_empty());
}

#[test]
fn the_paths_are_held_until_the_move_is_reported() {
    let root = tempfile::tempdir().expect("tempdir");
    let (mut fx, old_file) = file_only(root.path());
    let new_file = old_file.with_file_name("Bar.snxprj");

    let task = submit_rename(&mut fx.app, &old_file, "Bar");

    let held = (
        fx.app.refuse_while_path_op(&old_file, "saved").is_err(),
        fx.app.refuse_while_path_op(&new_file, "saved").is_err(),
        fx.app
            .refuse_start_while_path_op(&old_file, "renamed")
            .is_err(),
    );
    run_task(&mut fx.app, task);
    assert_eq!(held, (true, true, true));
    assert!(fx.app.document_state.pending_path_ops.is_empty());
    assert!(fx.app.refuse_while_path_op(&new_file, "saved").is_ok());
}

#[test]
fn a_move_that_fails_leaves_the_project_and_says_why() {
    let _ = crate::diagnostics::init_logging();
    let root = tempfile::tempdir().expect("tempdir");
    let (mut fx, old_file) = file_only(root.path());
    let task = submit_rename(&mut fx.app, &old_file, "Bar");
    // The file vanishes before the worker moves it.
    std::fs::remove_file(&old_file).expect("remove");
    let since = crate::diagnostics::newest_entry_id();

    run_task(&mut fx.app, task);

    let dialog_error = fx
        .app
        .ui_state
        .rename_dialog
        .as_ref()
        .and_then(|d| d.error.clone());
    assert!(
        dialog_error.is_some_and(|e| e.starts_with("Rename failed")),
        "the dialog says why"
    );
    assert_eq!(fx.app.document_state.projects[0].path, old_file);
    assert!(fx.app.document_state.pending_path_ops.is_empty());
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| { Some(e.id) > since && e.message.contains("was not renamed") })
    );
}

#[test]
fn a_project_closed_while_the_file_moves_is_reported_not_rekeyed() {
    let _ = crate::diagnostics::init_logging();
    let root = tempfile::tempdir().expect("tempdir");
    let (mut fx, old_file) = file_only(root.path());
    let new_file = old_file.with_file_name("Bar.snxprj");
    let task = submit_rename(&mut fx.app, &old_file, "Bar");
    fx.app.document_state.projects.clear();
    let since = crate::diagnostics::newest_entry_id();

    run_task(&mut fx.app, task);

    assert!(new_file.exists(), "the move itself happened");
    assert!(fx.app.ui_state.rename_dialog.is_none());
    assert!(fx.app.document_state.pending_path_ops.is_empty());
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| { Some(e.id) > since && e.message.contains("closed or moved meanwhile") })
    );
}

/// Cancel the project rename, then open Rename on a sheet (another file).
fn reopen_dialog_for_another_file(fx: &mut Fixture, other: &Path) {
    fx.app.ui_state.rename_dialog = Some(crate::app::RenameDialogState {
        project_id: None,
        target_path: other.to_path_buf(),
        tree_path: vec![0],
        buffer: "half_typed".to_string(),
        error: None,
        is_project_rename: false,
    });
}

#[test]
fn a_failed_move_does_not_blame_a_dialog_reopened_for_another_file() {
    let root = tempfile::tempdir().expect("tempdir");
    let (mut fx, old_file) = file_only(root.path());
    let task = submit_rename(&mut fx.app, &old_file, "Bar");
    let sheet = fx.sheet.clone();
    reopen_dialog_for_another_file(&mut fx, &sheet);
    std::fs::remove_file(&old_file).expect("remove");

    run_task(&mut fx.app, task);

    let dialog = fx
        .app
        .ui_state
        .rename_dialog
        .as_ref()
        .expect("sheet dialog");
    assert!(dialog.error.is_none(), "{:?}", dialog.error);
    let notice = fx.app.document_state.error_notice.as_ref().expect("notice");
    assert!(notice.detail.contains("Rename failed"), "{}", notice.detail);
}

#[test]
fn a_finished_move_does_not_close_a_dialog_reopened_for_another_file() {
    let root = tempfile::tempdir().expect("tempdir");
    let (mut fx, old_file) = file_only(root.path());
    let task = submit_rename(&mut fx.app, &old_file, "Bar");
    let sheet = fx.sheet.clone();
    reopen_dialog_for_another_file(&mut fx, &sheet);

    run_task(&mut fx.app, task);

    let dialog = fx
        .app
        .ui_state
        .rename_dialog
        .as_ref()
        .expect("sheet dialog kept");
    assert_eq!(dialog.buffer, "half_typed");
    assert_eq!(
        fx.app.document_state.projects[0].path,
        old_file.with_file_name("Bar.snxprj")
    );
}
