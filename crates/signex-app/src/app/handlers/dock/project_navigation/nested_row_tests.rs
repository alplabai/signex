//! Tree actions on a sheet nested under its parent (#1618).

use std::path::PathBuf;

use signex_widgets::tree_view::{TreeKey, TreeMsg};

use super::*;
use crate::app::ProjectTreeAction;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::hierarchy_fixture::{DIR, hierarchical_app};
use crate::app::runtime::sheet_tree_tests::settle;

/// The key of `leaf.snxsch`, nested `top -> mid -> leaf`.
fn nested_app() -> (Signex, TreeKey) {
    let mut app = hierarchical_app();
    settle(&mut app, false);
    let key = vec![
        app.document_state.projects[0].id.to_string(),
        "Source Documents".to_string(),
        "top.snxsch".to_string(),
        "mid.snxsch".to_string(),
        "leaf.snxsch".to_string(),
    ];
    assert!(app.resolve_tree_row(&key).is_some(), "the row is nested");
    (app, key)
}

fn leaf_path() -> PathBuf {
    PathBuf::from(DIR).join("leaf.snxsch")
}

fn select(app: &mut Signex, key: &TreeKey) -> Task<Message> {
    let msg = crate::panels::PanelMsg::Tree(TreeMsg::Select(key.clone()));
    app.handle_dock_project_navigation_panel_message(&msg)
        .expect("a tree message is handled")
}

#[test]
fn a_double_click_opens_the_nested_sheet() {
    let (mut app, key) = nested_app();
    let first = select(&mut app, &key);
    assert_eq!(first.units(), 0, "a click starts no work by itself");
    let opening = select(&mut app, &key);
    assert!(opening.units() > 0, "the read starts");
    assert!(app.pending_open_under(&leaf_path()).is_some());
}

#[test]
fn rename_remove_and_reveal_target_the_nested_file() {
    let (mut app, key) = nested_app();

    let renaming = app.handle_project_tree_action(ProjectTreeAction::OpenRenameDialog(key.clone()));
    assert_eq!(renaming.units(), 0);
    let rename = app.ui_state.rename_dialog.as_ref().expect("rename opens");
    assert_eq!(rename.target_path, leaf_path());
    assert_eq!(rename.buffer, "leaf.snxsch");

    let removing = app.handle_project_tree_action(ProjectTreeAction::OpenRemoveDialog(key.clone()));
    assert_eq!(removing.units(), 0);
    let remove = app.ui_state.remove_dialog.as_ref().expect("remove opens");
    assert_eq!(remove.target_path, leaf_path());
    assert!(!remove.is_outside_project);

    // Reveal resolves through the same path; the action itself would spawn
    // the file manager.
    let path = app.resolve_tree_row(&key).expect("row");
    assert_eq!(app.tree_path_to_file_path(&path), Some(leaf_path()));
}

#[test]
fn a_nested_row_survives_a_new_row_inserted_before_it() {
    let (mut app, key) = nested_app();
    app.document_state.panel_ctx.project_tree[0].children[0]
        .children
        .insert(
            0,
            signex_widgets::tree_view::TreeNode::leaf(
                "new.snxsch",
                signex_widgets::tree_view::TreeIcon::SnxSchematic,
            ),
        );
    let path = app.resolve_tree_row(&key).expect("still resolves");
    assert_eq!(app.tree_path_to_file_path(&path), Some(leaf_path()));
}

/// `top` (open, on disk) places `child`, which is not on disk.
fn app_with_missing_child(tmp: &std::path::Path, child: &str) -> (Signex, TreeKey) {
    crate::app::runtime::sheet_tree_tests::write_sheet(tmp, "top.snxsch", &[child]);
    let dir = tmp.to_string_lossy().into_owned();
    let mut app = app_workspace(&dir, &["top.snxsch"]);
    assert!(app.open_or_focus_sheet_path(tmp.join("top.snxsch")));
    settle(&mut app, false);
    let key = vec![
        app.document_state.projects[0].id.to_string(),
        "Source Documents".to_string(),
        "top.snxsch".to_string(),
        child.to_string(),
    ];
    (app, key)
}

fn double_click(app: &mut Signex, key: &TreeKey) -> Task<Message> {
    let first = select(app, key);
    assert_eq!(first.units(), 0);
    select(app, key)
}

#[test]
fn activating_a_missing_row_opens_the_create_sheet_dialog() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (mut app, key) = app_with_missing_child(tmp.path(), "ghost.snxsch");

    let activated = double_click(&mut app, &key);

    assert_eq!(activated.units(), 0, "no read of a file that is not there");
    assert!(
        app.pending_open_under(&tmp.path().join("ghost.snxsch"))
            .is_none()
    );
    let dialog = app
        .ui_state
        .create_child_sheet_confirm
        .as_ref()
        .expect("the Create Sheet dialog opens");
    assert_eq!(dialog.parent, tmp.path().join("top.snxsch"));
    assert_eq!(dialog.path_text, "ghost.snxsch");
}

#[test]
fn a_missing_row_whose_file_now_exists_cannot_be_created_over() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (mut app, key) = app_with_missing_child(tmp.path(), "later.snxsch");
    open_dialog_by_double_click(&mut app, &key);
    crate::app::runtime::sheet_tree_tests::write_sheet(tmp.path(), "later.snxsch", &[]);
    let before = std::fs::read(tmp.path().join("later.snxsch")).expect("read");

    let created = app.handle_create_child_sheet_answered(true);

    assert_eq!(created.units(), 0, "nothing is written");
    let dialog = app
        .ui_state
        .create_child_sheet_confirm
        .as_ref()
        .expect("the dialog stays open");
    assert!(dialog.exists, "and offers to open the file instead");
    assert_eq!(
        std::fs::read(tmp.path().join("later.snxsch")).expect("read"),
        before,
        "left untouched"
    );
}

fn open_dialog_by_double_click(app: &mut Signex, key: &TreeKey) {
    let activated = double_click(app, key);
    assert_eq!(activated.units(), 0);
    assert!(app.ui_state.create_child_sheet_confirm.is_some());
}

#[test]
fn a_missing_row_under_a_closed_parent_opens_it_first_then_the_dialog() {
    let tmp = tempfile::tempdir().expect("tempdir");
    crate::app::runtime::sheet_tree_tests::write_sheet(tmp.path(), "top.snxsch", &["ghost.snxsch"]);
    let dir = tmp.path().to_string_lossy().into_owned();
    let mut app = app_workspace(&dir, &["top.snxsch"]);
    settle(&mut app, false);
    let key = vec![
        app.document_state.projects[0].id.to_string(),
        "Source Documents".to_string(),
        "top.snxsch".to_string(),
        "ghost.snxsch".to_string(),
    ];

    let opening = double_click(&mut app, &key);

    assert!(opening.units() > 0, "the parent is read on a worker");
    assert!(app.ui_state.create_child_sheet_confirm.is_none());
    crate::app::handlers::menu::export::write::settle(&mut app, opening);
    let dialog = app
        .ui_state
        .create_child_sheet_confirm
        .as_ref()
        .expect("the dialog follows the open");
    assert_eq!(dialog.parent, tmp.path().join("top.snxsch"));
    assert!(app.ui_state.pending_create_sheet.is_none());
}
