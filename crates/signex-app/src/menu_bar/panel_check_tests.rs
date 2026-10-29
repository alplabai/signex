//! The View menu's panel rows show a check mark that follows the panel (#103).

use super::MenuContext;
use super::model::{MenuEntry, menu_bar_model};
use crate::app::Signex;
use crate::keymap::AppCommandId;
use crate::panels::PanelKind;

const PANEL_ROWS: [(&str, PanelKind); 7] = [
    ("Projects", PanelKind::Projects),
    ("Components", PanelKind::Components),
    ("Navigator", PanelKind::Navigator),
    ("Properties", PanelKind::Properties),
    ("ERC", PanelKind::Erc),
    ("Messages", PanelKind::Messages),
    ("Signal", PanelKind::Signal),
];

pub(crate) fn context(app: &Signex) -> MenuContext<'static> {
    MenuContext {
        open_panels: app.open_panels(),
        ..MenuContext::default()
    }
}

/// The `checked` state of the View-menu row labelled `label`.
pub(crate) fn view_row_checked(ctx: &MenuContext, label: &str) -> Option<bool> {
    let model = menu_bar_model(ctx);
    let view = model.menus.iter().find(|m| m.title == "View").unwrap();
    view.entries.iter().find_map(|entry| match entry {
        MenuEntry::Item(item) if item.label == label => Some(item.checked),
        _ => None,
    })?
}

fn run(app: &mut Signex, id: &str) {
    let command = AppCommandId::new(id).unwrap();
    let message = crate::app::command::bridge::core_to_message(&command).unwrap();
    let _ = app.update(message);
}

#[test]
fn every_panel_row_is_a_check_row_that_follows_the_panel() {
    let (mut app, _task) = Signex::new();
    for (label, kind) in PANEL_ROWS {
        app.document_state.dock.close_panel(kind);
        assert_eq!(
            view_row_checked(&context(&app), label),
            Some(false),
            "{label}"
        );
        let _ = app.show_panel(kind);
        assert_eq!(
            view_row_checked(&context(&app), label),
            Some(true),
            "{label}"
        );
        app.document_state.dock.close_panel(kind);
        assert_eq!(
            view_row_checked(&context(&app), label),
            Some(false),
            "{label}"
        );
    }
}

#[test]
fn the_shortcut_toggles_the_check_mark() {
    let (mut app, _task) = Signex::new();
    app.document_state.dock.close_panel(PanelKind::Properties);
    app.document_state.dock.close_panel(PanelKind::Messages);

    run(&mut app, "toggle_properties_panel");
    assert_eq!(view_row_checked(&context(&app), "Properties"), Some(true));
    assert_eq!(view_row_checked(&context(&app), "Messages"), Some(false));

    run(&mut app, "toggle_properties_panel");
    assert_eq!(view_row_checked(&context(&app), "Properties"), Some(false));
}

#[test]
fn a_panel_hidden_behind_another_tab_stays_checked() {
    let (mut app, _task) = Signex::new();
    app.document_state.dock.close_panel(PanelKind::Properties);
    let _ = app.show_panel(PanelKind::Properties);
    // Same home region as Properties; showing it takes the front.
    app.document_state.dock.show_panel(PanelKind::Filter);
    assert!(!app.document_state.dock.is_frontmost(PanelKind::Properties));

    assert_eq!(view_row_checked(&context(&app), "Properties"), Some(true));
}

#[test]
fn other_view_rows_carry_no_check_mark() {
    let model = menu_bar_model(&MenuContext::default());
    let view = model.menus.iter().find(|m| m.title == "View").unwrap();
    let checked = view
        .entries
        .iter()
        .filter(|e| matches!(e, MenuEntry::Item(i) if i.checked.is_some()))
        .count();
    assert_eq!(checked, PANEL_ROWS.len());
}
