//! #2232 — the Projects panel's empty-area menu offers New Project, which
//! opens the setup dialog.

use signex_widgets::active_bar_dropdown::DropdownEntry;
use signex_widgets::tree_view::TreeMsg;

use crate::app::{Message, Signex};
use crate::menu_bar::MenuMessage;

#[test]
fn the_empty_area_menu_has_new_project_and_it_opens_the_dialog() {
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Dock(crate::dock::DockMessage::Panel(
        crate::panels::PanelMsg::Tree(TreeMsg::BackgroundContextMenu),
    )));
    let ctx = app
        .interaction_state
        .project_tree_context_menu
        .clone()
        .expect("the empty-area menu opened");
    assert!(ctx.key.is_none());
    let rows = app.project_tree_entries(&ctx).expect("the menu has rows");
    let press = rows
        .iter()
        .find_map(|row| match row {
            DropdownEntry::Item(item) if item.label == "New Project..." => item.on_press.clone(),
            _ => None,
        })
        .expect("a live New Project row");
    assert!(matches!(press, Message::Menu(MenuMessage::NewProject)));
    let _ = app.update(press);
    assert!(app.ui_state.new_project.is_some(), "the dialog is open");
}
