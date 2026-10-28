//! #1618 — every Projects-tree action names its row by key, so a reordered
//! or rebuilt tree can never retarget it at a neighbour. These pin that each
//! action resolves the same document before and after the rows change order.

use std::path::PathBuf;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{ContextMenuMsg, Message, ProjectTreeAction, Signex};
use signex_widgets::tree_view::{TreeKey, key_of};

const SHEETS: [&str; 3] = ["a.snxsch", "b.snxsch", "c.snxsch"];

fn app_with_sheets() -> Signex {
    let mut app = app_workspace("/proj", &SHEETS);
    app.refresh_panel_ctx();
    app
}

/// The key of the sheet row for `filename`, found by label so the lookup does
/// not depend on row order.
fn sheet_key(app: &Signex, filename: &str) -> TreeKey {
    let tree = app.document_state.panel_ctx.project_tree.as_slice();
    let docs = &tree[0].children[0];
    let idx = docs
        .children
        .iter()
        .position(|n| n.label == filename)
        .expect("sheet row");
    key_of(tree, &[0, 0, idx]).expect("key")
}

/// Reverse the project's sheet list and rebuild the tree.
fn reorder(app: &mut Signex) {
    app.document_state.projects[0].data.sheets.reverse();
    app.refresh_panel_ctx();
}

fn pick(app: &mut Signex, action: ProjectTreeAction) {
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
        action,
    )));
}

/// Open, rename, remove and reveal all resolve a row through
/// `resolve_tree_row` + `tree_path_to_file_path`; the file must follow the key.
#[test]
fn a_sheet_key_resolves_to_its_file_after_the_rows_reorder() {
    let mut app = app_with_sheets();
    let keys: Vec<TreeKey> = SHEETS.iter().map(|f| sheet_key(&app, f)).collect();

    reorder(&mut app);

    for (key, name) in keys.iter().zip(SHEETS) {
        let path = app.resolve_tree_row(key).expect("row still exists");
        assert_eq!(
            app.tree_path_to_file_path(&path),
            Some(PathBuf::from("/proj").join(name)),
            "{name}"
        );
    }
}

#[test]
fn rename_dialog_targets_the_keyed_sheet_after_a_reorder() {
    let mut app = app_with_sheets();
    let key = sheet_key(&app, "b.snxsch");
    reorder(&mut app);

    pick(&mut app, ProjectTreeAction::OpenRenameDialog(key));

    let dialog = app.ui_state.rename_dialog.as_ref().expect("dialog");
    assert_eq!(dialog.target_path, PathBuf::from("/proj/b.snxsch"));
}

#[test]
fn remove_dialog_targets_the_keyed_sheet_after_a_reorder() {
    let mut app = app_with_sheets();
    let key = sheet_key(&app, "c.snxsch");
    reorder(&mut app);

    pick(&mut app, ProjectTreeAction::OpenRemoveDialog(key));

    let dialog = app.ui_state.remove_dialog.as_ref().expect("dialog");
    assert_eq!(dialog.target_path, PathBuf::from("/proj/c.snxsch"));
}

#[test]
fn open_selects_the_keyed_sheet_after_a_reorder() {
    let mut app = app_with_sheets();
    let key = sheet_key(&app, "a.snxsch");
    reorder(&mut app);

    pick(&mut app, ProjectTreeAction::OpenNode(key.clone()));

    assert_eq!(
        app.document_state.panel_ctx.project_tree_selected,
        Some(key)
    );
}

#[test]
fn toggle_flips_the_keyed_branch_after_a_reorder() {
    let mut app = app_with_sheets();
    let key = vec![
        app.document_state.projects[0].id.to_string(),
        "Source Documents".to_string(),
    ];
    reorder(&mut app);
    let before = app.document_state.panel_ctx.project_tree[0].children[0].expanded;

    pick(&mut app, ProjectTreeAction::ToggleNode(key));

    let after = app.document_state.panel_ctx.project_tree[0].children[0].expanded;
    assert_ne!(before, after);
}

/// A sheet that left the project must drop the action, not fall onto the
/// sheet that slid into its place.
#[test]
fn a_stale_sheet_key_drops_the_action() {
    let mut app = app_with_sheets();
    let key = sheet_key(&app, "b.snxsch");
    app.document_state.projects[0]
        .data
        .sheets
        .retain(|s| s.filename != "b.snxsch");
    app.refresh_panel_ctx();

    assert_eq!(app.resolve_tree_row(&key), None);
    pick(&mut app, ProjectTreeAction::OpenRemoveDialog(key.clone()));
    pick(&mut app, ProjectTreeAction::OpenRenameDialog(key));

    assert!(app.ui_state.remove_dialog.is_none());
    assert!(app.ui_state.rename_dialog.is_none());
}

/// Project-level actions (rename, close, validate, options, add, reveal,
/// version control) resolve the project row by its id, not its position.
#[test]
fn a_project_root_key_follows_its_project_when_projects_reorder() {
    let mut app = app_with_sheets();
    let mut second = app.document_state.projects[0].clone();
    second.id = app.document_state.mint_project_id();
    second.path = PathBuf::from("/other/ProjectB.snxprj");
    second.data.name = "ProjectB".to_string();
    app.document_state.projects.push(second);
    app.refresh_panel_ctx();
    let b_id = app.document_state.projects[1].id;
    let b_key = vec![b_id.to_string()];

    app.document_state.projects.reverse();
    app.refresh_panel_ctx();

    assert_eq!(app.resolve_tree_row(&b_key), Some(vec![0]));
    pick(&mut app, ProjectTreeAction::OpenProjectRenameDialog(b_key));
    let dialog = app.ui_state.rename_dialog.as_ref().expect("dialog");
    assert_eq!(dialog.project_id, Some(b_id));
    assert!(dialog.is_project_rename);
}
