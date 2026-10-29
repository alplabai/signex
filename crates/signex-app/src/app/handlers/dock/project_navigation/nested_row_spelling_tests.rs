//! Delete and Rename guard sheets whatever their extension, a corrupt sheet
//! does not block deleting itself, and a Create Sheet request does not go
//! stale (#1618).

use std::path::Path;

use signex_widgets::tree_view::TreeKey;

use super::*;
use crate::app::ProjectTreeAction;
use crate::app::RemoveChoice;
use crate::app::handlers::menu::export::tests::{add_project, app_workspace};
use crate::app::runtime::sheet_tree_tests::{settle, write_sheet};

fn key(app: &Signex, files: &[&str]) -> TreeKey {
    let mut key = vec![
        app.document_state.projects[0].id.to_string(),
        "Source Documents".to_string(),
    ];
    key.extend(files.iter().map(|f| (*f).to_string()));
    key
}

/// `top` places `mid` and `shared`, and `mid` places `shared` too: the row is
/// `top/mid/shared`, with `top` as the other parent.
fn shared_by_two(tmp: &Path, shared: &str) -> Signex {
    write_sheet(tmp, "top.snxsch", &["mid.snxsch", shared]);
    write_sheet(tmp, "mid.snxsch", &[shared]);
    write_sheet(tmp, shared, &[]);
    let dir = tmp.to_string_lossy().into_owned();
    let mut app = app_workspace(&dir, &["top.snxsch", "mid.snxsch", shared]);
    settle(&mut app, false);
    app
}

fn delete_is_refused_for(shared: &str) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut app = shared_by_two(tmp.path(), shared);
    let key = key(&app, &["top.snxsch", "mid.snxsch", shared]);
    let opened = app.handle_project_tree_action(ProjectTreeAction::OpenRemoveDialog(key));
    assert_eq!(opened.units(), 0);

    let confirmed = app.handle_remove_confirm(RemoveChoice::DeleteFile);

    assert_eq!(confirmed.units(), 0);
    assert!(tmp.path().join(shared).exists(), "{shared} stays on disk");
    assert!(app.document_state.error_notice.is_some());
}

#[test]
fn an_uppercase_extension_sheet_placed_by_two_parents_is_not_deleted() {
    delete_is_refused_for("Power.SNXSCH");
}

#[test]
fn a_non_sheet_extension_placed_by_two_parents_is_not_deleted() {
    delete_is_refused_for("power.sch");
}

fn rename_is_refused_for(shared: &str) {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_sheet(tmp.path(), "top.snxsch", &[shared]);
    write_sheet(tmp.path(), "elsewhere.snxsch", &[shared]);
    write_sheet(tmp.path(), shared, &[]);
    let dir = tmp.path().to_string_lossy().into_owned();
    let mut app = app_workspace(&dir, &["top.snxsch", shared]);
    add_project(&mut app, &dir, &["elsewhere.snxsch"]);
    settle(&mut app, false);
    let key = key(&app, &["top.snxsch", shared]);
    let opened = app.handle_project_tree_action(ProjectTreeAction::OpenRenameDialog(key));
    assert_eq!(opened.units(), 0);
    let renamed = format!("moved-{shared}");
    app.ui_state.rename_dialog.as_mut().expect("opens").buffer = renamed.clone();

    let submitted = app.handle_rename_submit();

    assert_eq!(submitted.units(), 0);
    assert!(tmp.path().join(shared).exists(), "nothing moved");
    assert!(!tmp.path().join(&renamed).exists());
    let error = app
        .ui_state
        .rename_dialog
        .as_ref()
        .and_then(|d| d.error.clone())
        .expect("the dialog says why");
    assert!(error.contains("elsewhere.snxsch"), "{error}");
}

#[test]
fn renaming_an_uppercase_extension_sheet_another_project_places_is_refused() {
    rename_is_refused_for("Power.SNXSCH");
}

#[test]
fn renaming_a_non_sheet_extension_file_another_project_places_is_refused() {
    rename_is_refused_for("power.sch");
}

#[test]
fn a_corrupt_sheet_can_be_deleted_but_blocks_deleting_others_with_its_name() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_sheet(tmp.path(), "solo.snxsch", &[]);
    std::fs::write(tmp.path().join("corrupt.snxsch"), "not a sheet").expect("write");
    let dir = tmp.path().to_string_lossy().into_owned();
    let mut app = app_workspace(&dir, &["solo.snxsch", "corrupt.snxsch"]);
    settle(&mut app, false);

    let solo = key(&app, &["solo.snxsch"]);
    let opened = app.handle_project_tree_action(ProjectTreeAction::OpenRemoveDialog(solo));
    assert_eq!(opened.units(), 0);
    let confirmed = app.handle_remove_confirm(RemoveChoice::DeleteFile);
    assert_eq!(confirmed.units(), 0);
    assert!(tmp.path().join("solo.snxsch").exists(), "refused");
    let detail = app
        .document_state
        .error_notice
        .as_ref()
        .map(|n| n.detail.clone())
        .expect("a notice");
    assert!(
        detail.contains("corrupt.snxsch"),
        "names the sheet: {detail}"
    );

    app.document_state.error_notice = None;
    let corrupt = key(&app, &["corrupt.snxsch"]);
    let opened = app.handle_project_tree_action(ProjectTreeAction::OpenRemoveDialog(corrupt));
    assert_eq!(opened.units(), 0);
    // The delete runs on a worker (#859); settle drives its completion.
    crate::app::path_ops::settle(
        &mut app,
        crate::app::Message::Remove(crate::app::RemoveMsg::Confirm(RemoveChoice::DeleteFile)),
    );
    assert!(!tmp.path().join("corrupt.snxsch").exists(), "deleted");
}

/// `top` (closed) places the missing `ghost`.
fn closed_parent_app(tmp: &Path) -> (Signex, TreeKey) {
    write_sheet(tmp, "top.snxsch", &["ghost.snxsch"]);
    let dir = tmp.to_string_lossy().into_owned();
    let mut app = app_workspace(&dir, &["top.snxsch"]);
    settle(&mut app, false);
    let key = key(&app, &["top.snxsch", "ghost.snxsch"]);
    (app, key)
}

fn double_click(app: &mut Signex, key: &TreeKey) -> Task<Message> {
    let msg =
        crate::panels::PanelMsg::Tree(signex_widgets::tree_view::TreeMsg::Select(key.clone()));
    let first = app
        .handle_dock_project_navigation_panel_message(&msg)
        .expect("handled");
    assert_eq!(first.units(), 0);
    app.handle_dock_project_navigation_panel_message(&msg)
        .expect("handled")
}

#[test]
fn a_cancelled_read_leaves_no_pending_create_sheet() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (mut app, key) = closed_parent_app(tmp.path());
    let opening = double_click(&mut app, &key);
    assert!(opening.units() > 0);
    assert!(app.ui_state.pending_create_sheet.is_some());

    app.cancel_in_flight_reads();
    assert!(app.ui_state.pending_create_sheet.is_none());
    crate::app::handlers::menu::export::write::settle(&mut app, opening);

    // A later, unrelated open of that parent shows no dialog.
    assert!(app.open_or_focus_sheet_path(tmp.path().join("top.snxsch")));
    assert!(app.ui_state.create_child_sheet_confirm.is_none());
}

#[test]
fn a_case_variant_parent_spelling_still_opens_the_dialog() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (mut app, _) = closed_parent_app(tmp.path());
    app.document_state.projects[0].case_fold = crate::app::state::scope::CaseFold::case_only();
    assert!(app.open_or_focus_sheet_path(tmp.path().join("top.snxsch")));
    app.ui_state.pending_create_sheet = Some((
        tmp.path().join("TOP.snxsch"),
        tmp.path().join("ghost.snxsch"),
    ));

    app.resume_create_sheet(&tmp.path().join("top.snxsch"), true);

    assert!(app.ui_state.create_child_sheet_confirm.is_some());
    assert!(app.ui_state.pending_create_sheet.is_none());
}
