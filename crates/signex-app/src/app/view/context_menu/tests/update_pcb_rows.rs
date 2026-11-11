//! #1312 — the Projects tree's project row offers Update PCB Document, and
//! its Add New flyout a live PCB row; each is addressed by the clicked row.

use signex_widgets::active_bar_dropdown::DropdownEntry;
use signex_widgets::tree_view::{TreeIcon, TreeNode, key_of};

use super::TID;
use crate::app::view::context_menu::submenu::add_new_entries;
use crate::app::{ContextMenuMsg, Message, ProjectTreeAction, ProjectTreeContextMenuState, Signex};

fn press_of(rows: &[DropdownEntry<Message>], label: &str) -> Option<Message> {
    rows.iter().find_map(|row| match row {
        DropdownEntry::Item(item) if item.label == label => item.on_press.clone(),
        _ => None,
    })
}

#[test]
fn the_project_row_offers_update_pcb_for_that_project() {
    let (mut app, _t) = Signex::new();
    app.document_state.panel_ctx.project_tree =
        vec![TreeNode::branch("proj", TreeIcon::Folder, Vec::new())];
    let key = key_of(&app.document_state.panel_ctx.project_tree, &[0]).expect("the project row");
    let ctx = ProjectTreeContextMenuState {
        x: 0.0,
        y: 0.0,
        key: Some(key.clone()),
        window: None,
    };
    let rows = app.project_tree_entries(&ctx).expect("the menu has rows");
    let press = press_of(&rows, "Update PCB Document").expect("a live Update PCB row");
    assert!(matches!(
        press,
        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(ProjectTreeAction::UpdatePcb(k)))
            if k == key
    ));
}

#[test]
fn add_new_offers_a_live_pcb_row() {
    let (mut app, _t) = Signex::new();
    app.document_state.panel_ctx.project_tree =
        vec![TreeNode::branch("proj", TreeIcon::Folder, Vec::new())];
    let key = key_of(&app.document_state.panel_ctx.project_tree, &[0]).expect("the project row");
    let rows = add_new_entries(TID, key.clone());
    let press = press_of(&rows, "PCB").expect("a live PCB row");
    assert!(matches!(
        press,
        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(ProjectTreeAction::AddNewPcb(k)))
            if k == key
    ));
}
