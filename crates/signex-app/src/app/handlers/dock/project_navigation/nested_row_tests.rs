//! Tree actions on a sheet nested under its parent (#1618).

use std::path::PathBuf;

use signex_widgets::tree_view::{TreeKey, TreeMsg};

use super::*;
use crate::app::ProjectTreeAction;
use crate::app::handlers::menu::export::tests::tree_nesting_pins::{DIR, hierarchical_app};
use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};
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

#[test]
fn activating_a_missing_row_offers_to_create_the_sheet_instead_of_opening() {
    let dir = "/w/tree-create-sheet";
    let mut app = app_workspace(dir, &["top.snxsch"]);
    open_with(
        &mut app.document_state,
        &PathBuf::from(dir).join("top.snxsch"),
        sheet_with_net("R_TOP", "N", &["ghost-1618.snxsch"]),
    );
    settle(&mut app, false);
    let key = vec![
        app.document_state.projects[0].id.to_string(),
        "Source Documents".to_string(),
        "top.snxsch".to_string(),
        "ghost-1618.snxsch".to_string(),
    ];
    let before = crate::diagnostics::logged_count("ghost-1618.snxsch is referenced by");

    let first = select(&mut app, &key);
    assert_eq!(first.units(), 0);
    let activated = select(&mut app, &key);

    assert_eq!(activated.units(), 0, "no read of a file that is not there");
    assert!(
        app.pending_open_under(&PathBuf::from(dir).join("ghost-1618.snxsch"))
            .is_none()
    );
    assert_eq!(
        crate::diagnostics::logged_count("ghost-1618.snxsch is referenced by"),
        before + 1
    );
}
