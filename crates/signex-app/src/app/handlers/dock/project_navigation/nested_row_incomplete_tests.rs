//! Delete and Rename refuse when "nobody else places this sheet" cannot be
//! told, and a placed sheet cannot be discarded by excluding it (#1618).

use std::path::Path;

use signex_widgets::tree_view::TreeKey;

use super::*;
use crate::app::handlers::menu::export::tests::{
    add_project, app_workspace, open_with, sheet_with_net,
};
use crate::app::runtime::sheet_tree_tests::{settle, write_sheet};
use crate::app::{ExcludeDirtyChoice, ProjectTreeAction, RemoveChoice};

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

/// Two projects in one folder: `ProjectA` lists `solo` and `victim`; `Other`
/// is rooted at `second`.
fn app_with_victim(tmp: &Path, second: &str) -> Signex {
    write_sheet(tmp, "solo.snxsch", &[]);
    write_sheet(tmp, "victim.snxsch", &[]);
    let dir = tmp.to_string_lossy().into_owned();
    let mut app = app_workspace(&dir, &["solo.snxsch", "victim.snxsch"]);
    add_project(&mut app, &dir, &[second]);
    app
}

fn delete_is_refused(app: &mut Signex, victim: &Path) {
    let key = key(app, &["victim.snxsch"]);
    open_remove(app, &key);
    let dialog = app.ui_state.remove_dialog.clone().expect("dialog opens");
    assert!(app.remove_blockers(&dialog).blocks_delete(false));
    let confirmed = app.handle_remove_confirm(RemoveChoice::DeleteFile);
    assert_eq!(confirmed.units(), 0);
    assert!(victim.exists(), "the file stays on disk");
    assert!(
        app.document_state.error_notice.is_some(),
        "and the user is told"
    );
}

#[test]
fn delete_is_refused_before_the_first_walk_lands() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_sheet(tmp.path(), "second.snxsch", &["victim.snxsch"]);
    let mut app = app_with_victim(tmp.path(), "second.snxsch");
    app.refresh_panel_ctx();
    let dialog_key = key(&app, &["victim.snxsch"]);
    open_remove(&mut app, &dialog_key);
    let dialog = app.ui_state.remove_dialog.clone().expect("dialog opens");
    let blockers = app.remove_blockers(&dialog);
    assert!(blockers.unknown.is_some(), "{blockers:?}");
    delete_is_refused(&mut app, &tmp.path().join("victim.snxsch"));
}

#[test]
fn delete_is_refused_while_another_project_has_an_unreadable_sheet() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(tmp.path().join("second.snxsch"), "not a sheet").expect("write");
    let mut app = app_with_victim(tmp.path(), "second.snxsch");
    settle(&mut app, false);
    delete_is_refused(&mut app, &tmp.path().join("victim.snxsch"));
}

#[test]
fn delete_is_refused_for_an_unsaved_symbol_that_places_the_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_sheet(tmp.path(), "second.snxsch", &[]);
    let mut app = app_with_victim(tmp.path(), "second.snxsch");
    settle(&mut app, false);
    // An edit after the last walk: the open sheet now carries the symbol.
    open_with(
        &mut app.document_state,
        &tmp.path().join("second.snxsch"),
        sheet_with_net("R", "N", &["victim.snxsch"]),
    );
    delete_is_refused(&mut app, &tmp.path().join("victim.snxsch"));
}

#[test]
fn delete_is_refused_for_an_open_sheet_outside_any_project() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_sheet(tmp.path(), "second.snxsch", &[]);
    let mut app = app_with_victim(tmp.path(), "second.snxsch");
    settle(&mut app, false);
    let stray = tmp.path().join("stray.snxsch");
    open_with(
        &mut app.document_state,
        &stray,
        sheet_with_net("R", "N", &["victim.snxsch"]),
    );
    assert!(app.document_state.document_project(&stray).is_none());
    delete_is_refused(&mut app, &tmp.path().join("victim.snxsch"));
}

#[test]
fn rename_is_refused_when_other_projects_cannot_be_ruled_out() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_sheet(tmp.path(), "second.snxsch", &[]);
    let mut app = app_with_victim(tmp.path(), "second.snxsch");
    app.refresh_panel_ctx();
    let key = key(&app, &["victim.snxsch"]);
    let opened = app.handle_project_tree_action(ProjectTreeAction::OpenRenameDialog(key));
    assert_eq!(opened.units(), 0);
    app.ui_state.rename_dialog.as_mut().expect("opens").buffer = "moved.snxsch".to_string();

    let submitted = app.handle_rename_submit();

    assert_eq!(submitted.units(), 0);
    assert!(tmp.path().join("victim.snxsch").exists(), "nothing moved");
    let error = app
        .ui_state
        .rename_dialog
        .as_ref()
        .and_then(|d| d.error.clone())
        .expect("the dialog says why");
    assert!(error.contains("cannot be ruled out"), "{error}");
}

#[test]
fn a_placed_sheet_with_unsaved_edits_cannot_be_discarded_by_excluding_it() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_sheet(tmp.path(), "top.snxsch", &["mid.snxsch"]);
    write_sheet(tmp.path(), "mid.snxsch", &[]);
    let dir = tmp.path().to_string_lossy().into_owned();
    let mut app = app_workspace(&dir, &["top.snxsch", "mid.snxsch"]);
    let mid = tmp.path().join("mid.snxsch");
    open_with(
        &mut app.document_state,
        &mid,
        sheet_with_net("R_MID", "N", &[]),
    );
    app.document_state.mark_document_dirty(&mid);
    settle(&mut app, false);
    let key = key(&app, &["top.snxsch", "mid.snxsch"]);

    open_remove(&mut app, &key);
    let confirmed = app.handle_remove_confirm(RemoveChoice::ExcludeFromProject);
    assert_eq!(confirmed.units(), 0);
    let dialog = app
        .ui_state
        .remove_dialog
        .clone()
        .expect("the prompt stays");
    assert!(dialog.unsaved_prompt);
    assert_eq!(
        app.discard_block(&dialog),
        Some(super::exclude_dirty::DiscardBlock::Placed)
    );
    let (_, can_discard) = app.exclude_dirty_prompt(&dialog);
    assert!(!can_discard, "Save or Cancel only");

    let discarded = app.handle_exclude_dirty_choice(ExcludeDirtyChoice::Discard);
    assert_eq!(discarded.units(), 0);
    assert!(app.document_state.engines.contains_key(&mid));
    assert!(app.document_state.is_document_dirty(&mid));
}
