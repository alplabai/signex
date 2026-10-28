//! Remove and Rename on sheets the hierarchy makes shared, unlisted or
//! missing (#1618).

use std::path::{Path, PathBuf};

use signex_widgets::tree_view::TreeKey;

use super::*;
use crate::app::handlers::menu::export::tests::{
    add_project, app_workspace, open_with, sheet_with_net,
};
use crate::app::runtime::sheet_tree_tests::{settle, write_sheet};
use crate::app::{ExcludeDirtyChoice, ProjectTreeAction, RemoveChoice, RemoveDialogState};

fn key(app: &Signex, files: &[&str]) -> TreeKey {
    let mut key = vec![
        app.document_state.projects[0].id.to_string(),
        "Source Documents".to_string(),
    ];
    key.extend(files.iter().map(|f| (*f).to_string()));
    key
}

fn open_remove(app: &mut Signex, key: &TreeKey) {
    let task = app.handle_project_tree_action(ProjectTreeAction::OpenRemoveDialog(key.clone()));
    assert_eq!(task.units(), 0);
}

#[test]
fn an_unlisted_nested_sheet_cannot_be_excluded_and_keeps_its_unsaved_edits() {
    let dir = "/w/nested-unlisted";
    let mut app = app_workspace(dir, &["top.snxsch"]);
    let (top, child) = (
        PathBuf::from(dir).join("top.snxsch"),
        PathBuf::from(dir).join("child.snxsch"),
    );
    open_with(
        &mut app.document_state,
        &top,
        sheet_with_net("R_TOP", "N", &["child.snxsch"]),
    );
    open_with(
        &mut app.document_state,
        &child,
        sheet_with_net("R_CHILD", "M", &[]),
    );
    app.document_state.dirty_paths.insert(child.clone());
    settle(&mut app, false);
    let key = key(&app, &["top.snxsch", "child.snxsch"]);

    open_remove(&mut app, &key);
    let dialog = app.ui_state.remove_dialog.clone().expect("dialog opens");
    assert!(app.remove_blockers(&dialog).unlisted);

    let confirmed = app.handle_remove_confirm(RemoveChoice::ExcludeFromProject);
    assert_eq!(confirmed.units(), 0);
    assert!(app.ui_state.remove_dialog.is_none());
    assert!(
        app.document_state.error_notice.is_some(),
        "the refusal shows"
    );
    assert!(app.document_state.engines.contains_key(&child));
    assert!(app.document_state.dirty_paths.contains(&child));

    // The unsaved-edits stage must not be a way around it.
    app.ui_state.remove_dialog = Some(RemoveDialogState {
        unsaved_prompt: true,
        ..dialog
    });
    let discarded = app.handle_exclude_dirty_choice(ExcludeDirtyChoice::Discard);
    assert_eq!(discarded.units(), 0);
    assert!(app.document_state.engines.contains_key(&child));
    assert!(app.document_state.dirty_paths.contains(&child));
}

/// `top -> {mid, leaf}`, `mid -> leaf`, all on disk and listed: `leaf` shows
/// under `mid`, and `top` places it too.
fn shared_leaf_app(tmp: &Path) -> Signex {
    write_sheet(tmp, "top.snxsch", &["mid.snxsch", "leaf.snxsch"]);
    write_sheet(tmp, "mid.snxsch", &["leaf.snxsch"]);
    write_sheet(tmp, "leaf.snxsch", &[]);
    let dir = tmp.to_string_lossy().into_owned();
    let mut app = app_workspace(&dir, &["top.snxsch", "mid.snxsch", "leaf.snxsch"]);
    settle(&mut app, false);
    app
}

#[test]
fn a_sheet_placed_by_two_parents_is_not_deleted() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut app = shared_leaf_app(tmp.path());
    let leaf = tmp.path().join("leaf.snxsch");
    let key = key(&app, &["top.snxsch", "mid.snxsch", "leaf.snxsch"]);

    open_remove(&mut app, &key);
    let dialog = app.ui_state.remove_dialog.clone().expect("dialog opens");
    let blockers = app.remove_blockers(&dialog);
    assert_eq!(blockers.other_parents, [tmp.path().join("top.snxsch")]);

    let confirmed = app.handle_remove_confirm(RemoveChoice::DeleteFile);
    assert_eq!(confirmed.units(), 0);
    assert!(leaf.exists(), "the shared sheet stays on disk");
    assert!(app.document_state.error_notice.is_some());
    assert!(
        app.document_state.projects[0]
            .data
            .sheets
            .iter()
            .any(|s| s.filename == "leaf.snxsch"),
        "and in the project"
    );
}

/// Two projects in one folder: `ProjectA` places `leaf` from `top`, `Other`
/// from `elsewhere`.
fn two_projects_sharing_a_leaf(tmp: &Path) -> Signex {
    write_sheet(tmp, "top.snxsch", &["leaf.snxsch"]);
    write_sheet(tmp, "elsewhere.snxsch", &["leaf.snxsch"]);
    write_sheet(tmp, "leaf.snxsch", &[]);
    let dir = tmp.to_string_lossy().into_owned();
    let mut app = app_workspace(&dir, &["top.snxsch", "leaf.snxsch"]);
    add_project(&mut app, &dir, &["elsewhere.snxsch"]);
    settle(&mut app, false);
    app
}

#[test]
fn a_sheet_shared_by_two_projects_is_not_deleted() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut app = two_projects_sharing_a_leaf(tmp.path());
    let key = key(&app, &["top.snxsch", "leaf.snxsch"]);

    open_remove(&mut app, &key);
    let dialog = app.ui_state.remove_dialog.clone().expect("dialog opens");
    assert_eq!(
        app.remove_blockers(&dialog).other_parents,
        [tmp.path().join("elsewhere.snxsch")]
    );
    let confirmed = app.handle_remove_confirm(RemoveChoice::DeleteFile);
    assert_eq!(confirmed.units(), 0);
    assert!(tmp.path().join("leaf.snxsch").exists());
}

#[test]
fn a_rename_that_would_strand_another_projects_parent_is_refused() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut app = two_projects_sharing_a_leaf(tmp.path());
    let key = key(&app, &["top.snxsch", "leaf.snxsch"]);

    let opened = app.handle_project_tree_action(ProjectTreeAction::OpenRenameDialog(key));
    assert_eq!(opened.units(), 0);
    app.ui_state.rename_dialog.as_mut().expect("opens").buffer = "renamed.snxsch".to_string();
    let submitted = app.handle_rename_submit();
    assert_eq!(submitted.units(), 0);

    assert!(tmp.path().join("leaf.snxsch").exists(), "nothing moved");
    assert!(!tmp.path().join("renamed.snxsch").exists());
    let error = app
        .ui_state
        .rename_dialog
        .as_ref()
        .and_then(|d| d.error.clone())
        .expect("the dialog says why");
    assert!(error.contains("elsewhere.snxsch"), "{error}");
}

#[test]
fn remove_and_rename_are_not_offered_on_a_missing_symbol_row() {
    let dir = "/w/nested-missing-ops";
    let mut app = app_workspace(dir, &["top.snxsch"]);
    open_with(
        &mut app.document_state,
        &PathBuf::from(dir).join("top.snxsch"),
        sheet_with_net("R_TOP", "N", &["ghost-1618b.snxsch"]),
    );
    settle(&mut app, false);
    let key = key(&app, &["top.snxsch", "ghost-1618b.snxsch"]);
    let before = crate::diagnostics::logged_count("the sheet does not exist yet");

    open_remove(&mut app, &key);
    let renaming = app.handle_project_tree_action(ProjectTreeAction::OpenRenameDialog(key));
    assert_eq!(renaming.units(), 0);

    assert!(app.ui_state.remove_dialog.is_none());
    assert!(app.ui_state.rename_dialog.is_none());
    assert_eq!(
        crate::diagnostics::logged_count("the sheet does not exist yet"),
        before + 2,
        "each refusal says why"
    );
}

#[test]
fn a_key_that_no_longer_resolves_is_reported_not_dropped_silently() {
    let (mut app, _) = {
        let app = crate::app::hierarchy_fixture::hierarchical_app();
        (app, ())
    };
    settle(&mut app, false);
    let before = crate::diagnostics::logged_count("Project tree action dropped");
    let gone = key(&app, &["top.snxsch", "not-a-row.snxsch"]);

    let task = app.handle_project_tree_action(ProjectTreeAction::OpenRenameDialog(gone));

    assert_eq!(task.units(), 0);
    assert!(app.ui_state.rename_dialog.is_none());
    assert_eq!(
        crate::diagnostics::logged_count("Project tree action dropped"),
        before + 1
    );
}

#[test]
fn an_unreadable_child_row_still_resolves_to_its_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_sheet(tmp.path(), "top.snxsch", &["broken.snxsch"]);
    std::fs::write(tmp.path().join("broken.snxsch"), "not a sheet").expect("write");
    let dir = tmp.path().to_string_lossy().into_owned();
    let mut app = app_workspace(&dir, &["top.snxsch"]);
    settle(&mut app, false);
    let key = key(&app, &["top.snxsch", "broken.snxsch"]);

    let path = app.resolve_tree_row(&key).expect("row");
    assert_eq!(
        app.tree_path_to_file_path(&path),
        Some(tmp.path().join("broken.snxsch"))
    );
}

#[test]
fn a_symbol_naming_a_file_without_the_sheet_extension_is_still_activatable() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_sheet(tmp.path(), "top.snxsch", &["notes-1618.txt"]);
    let dir = tmp.path().to_string_lossy().into_owned();
    let mut app = app_workspace(&dir, &["top.snxsch"]);
    assert!(app.open_or_focus_sheet_path(tmp.path().join("top.snxsch")));
    settle(&mut app, false);
    let key = key(&app, &["top.snxsch", "notes-1618.txt"]);

    for _ in 0..2 {
        let msg =
            crate::panels::PanelMsg::Tree(signex_widgets::tree_view::TreeMsg::Select(key.clone()));
        let task = app
            .handle_dock_project_navigation_panel_message(&msg)
            .expect("handled");
        assert_eq!(task.units(), 0);
    }

    assert!(
        app.ui_state.create_child_sheet_confirm.is_some(),
        "the double-click is answered, not ignored"
    );
}
