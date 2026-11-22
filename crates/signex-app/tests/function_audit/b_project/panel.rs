//! Rows m19 (Projects panel) and m20 (the tree's context menu).

use signex_app::app::{ContextMenuMsg, Message, ProjectTabMsg, ProjectTreeAction};

use super::support::{bench, bench_missing};

const TWO_SHEETS: &[(&str, &str)] = &[("root.snxsch", ""), ("b.snxsch", "")];

#[test]
fn m19_the_tree_lists_the_project_with_its_sheets() {
    let b = bench(TWO_SHEETS);
    let tree = &b.app.document_state.panel_ctx.project_tree;
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].label, "Proj");
    assert_eq!(b.node("root.snxsch").label, "root.snxsch");
    assert_eq!(b.node("b.snxsch").label, "b.snxsch");
}

#[test]
fn m19_the_open_sheet_is_the_active_row_and_the_closed_one_is_not() {
    let b = bench(TWO_SHEETS);
    assert!(b.node("root.snxsch").is_active);
    assert!(b.node("root.snxsch").is_open);
    assert!(!b.node("b.snxsch").is_open);
}

#[test]
fn m19_a_project_edit_marks_the_project_row_and_undo_clears_it() {
    let mut b = bench(TWO_SHEETS);
    b.open_section(signex_app::app::ProjectSection::General);
    let project_id = b.id;
    b.tab(ProjectTabMsg::DescriptionEdited {
        project_id,
        text: "Buck converter".into(),
    });
    assert!(b.app.document_state.panel_ctx.project_tree[0].is_dirty);
    b.undo();
    assert!(!b.app.document_state.panel_ctx.project_tree[0].is_dirty);
}

#[test]
fn m19_a_listed_file_gone_from_disk_is_marked_missing() {
    let b = bench_missing(TWO_SHEETS, &["b.snxsch"]);
    assert!(
        b.node("b.snxsch").label.contains("(missing)"),
        "row reads {:?}",
        b.node("b.snxsch").label
    );
    assert!(!b.node("root.snxsch").label.contains("(missing)"));
}

#[test]
fn m19_project_options_opens_the_project_tab_and_a_second_click_focuses_it() {
    let mut b = bench(TWO_SHEETS);
    let root = b.root_key();
    b.action(ProjectTreeAction::OpenProjectOptions(root.clone()));
    let count = |b: &super::support::Bench| {
        b.app
            .document_state
            .tabs
            .iter()
            .filter(|tab| tab.kind.as_project() == Some(b.id))
            .count()
    };
    assert_eq!(count(&b), 1);
    let active = &b.app.document_state.tabs[b.app.document_state.active_tab];
    assert_eq!(active.kind.as_project(), Some(b.id));
    b.action(ProjectTreeAction::OpenProjectOptions(root));
    assert_eq!(count(&b), 1, "the second click adds no tab");
}

#[test]
fn m20_right_click_opens_the_menu_and_close_dismisses_it() {
    let mut b = bench(TWO_SHEETS);
    let key = b.row("b.snxsch");
    b.send(Message::ContextMenu(ContextMenuMsg::ShowProjectTree(Some(
        key,
    ))));
    assert!(b.app.interaction_state.project_tree_context_menu.is_some());
    b.send(Message::ContextMenu(ContextMenuMsg::CloseProjectTree));
    assert!(b.app.interaction_state.project_tree_context_menu.is_none());
    assert_eq!(b.listed(), ["root.snxsch", "b.snxsch"]);
}

#[test]
fn m20_picking_a_row_closes_the_menu() {
    let mut b = bench(TWO_SHEETS);
    let key = b.row("b.snxsch");
    b.send(Message::ContextMenu(ContextMenuMsg::ShowProjectTree(Some(
        key.clone(),
    ))));
    b.action(ProjectTreeAction::OpenRenameDialog(key));
    assert!(b.app.interaction_state.project_tree_context_menu.is_none());
    assert!(b.app.ui_state.rename_dialog.is_some());
}

#[test]
fn m20_open_opens_the_document_of_the_row() {
    let mut b = bench(TWO_SHEETS);
    let key = b.row("b.snxsch");
    b.action(ProjectTreeAction::OpenNode(key));
    let path = b.file("b.snxsch");
    assert!(b.app.document_state.tabs.iter().any(|tab| tab.path == path));
}

#[test]
fn m20_collapse_and_expand_change_the_tree_rows() {
    let mut b = bench(TWO_SHEETS);
    b.action(ProjectTreeAction::CollapseAll);
    assert!(!b.app.document_state.panel_ctx.project_tree[0].expanded);
    b.action(ProjectTreeAction::ExpandAll);
    assert!(b.app.document_state.panel_ctx.project_tree[0].expanded);
    let root = b.root_key();
    b.action(ProjectTreeAction::ToggleNode(root));
    assert!(!b.app.document_state.panel_ctx.project_tree[0].expanded);
}

#[test]
fn m20_close_project_documents_closes_the_tabs_and_keeps_the_project() {
    let mut b = bench(TWO_SHEETS);
    b.action(ProjectTreeAction::CloseAllDocuments);
    assert!(b.app.document_state.tabs.is_empty());
    assert_eq!(b.app.document_state.projects.len(), 1);
}

#[test]
fn m20_a_row_that_no_longer_exists_opens_nothing() {
    let mut b = bench(TWO_SHEETS);
    let gone = vec![b.id.to_string(), "no-such-row".to_string()];
    b.action(ProjectTreeAction::OpenRenameDialog(gone.clone()));
    b.action(ProjectTreeAction::OpenRemoveDialog(gone));
    assert!(b.app.ui_state.rename_dialog.is_none());
    assert!(b.app.ui_state.remove_dialog.is_none());
}
