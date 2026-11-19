//! The sheet's state through `update`: what opening it sets, the filter and
//! scope messages, and Edit in Preferences (#2262).

use super::{ShortcutScope, ShortcutSheet};
use crate::app::{Message, OverlayMsg, Signex};
use crate::menu_bar::MenuMessage;

fn app() -> Signex {
    let (app, _task) = Signex::new();
    app
}

#[test]
fn opening_the_sheet_clears_the_filter_and_takes_the_active_editors_scope() {
    let mut app = app();
    assert!(!app.has_active_pcb());
    app.ui_state.shortcut_sheet = ShortcutSheet {
        filter: "left over".into(),
        scope: ShortcutScope::All,
    };
    let _ = app.update(Message::Menu(MenuMessage::OpenKeyboardShortcuts));
    assert!(app.ui_state.keyboard_shortcuts_open);
    assert_eq!(
        app.ui_state.shortcut_sheet,
        ShortcutSheet::opened(ShortcutScope::Schematic)
    );
}

#[test]
fn the_filter_and_scope_messages_set_the_sheet() {
    let mut app = app();
    let _ = app.update(Message::Menu(MenuMessage::OpenKeyboardShortcuts));
    let _ = app.update(Message::Overlay(OverlayMsg::ShortcutsFilterChanged(
        "wire".into(),
    )));
    assert_eq!(app.ui_state.shortcut_sheet.filter, "wire");
    let _ = app.update(Message::Overlay(OverlayMsg::ShortcutsScopePicked(
        ShortcutScope::Pcb,
    )));
    assert_eq!(app.ui_state.shortcut_sheet.scope, ShortcutScope::Pcb);
    let _ = app.update(Message::Overlay(OverlayMsg::ShortcutsFilterCleared));
    assert!(app.ui_state.shortcut_sheet.filter.is_empty());
    assert_eq!(
        app.ui_state.shortcut_sheet.scope,
        ShortcutScope::Pcb,
        "clearing the filter keeps the scope"
    );
    assert!(app.ui_state.keyboard_shortcuts_open);
}

#[test]
fn edit_in_preferences_closes_the_sheet_and_opens_the_keyboard_page() {
    let mut app = app();
    let _ = app.update(Message::Menu(MenuMessage::OpenKeyboardShortcuts));
    app.ui_state.preferences_nav = crate::preferences::PrefNav::Appearance;
    let _ = app.update(Message::Overlay(OverlayMsg::ShortcutsPreferencesPressed));
    assert!(!app.ui_state.keyboard_shortcuts_open);
    assert!(app.ui_state.preferences_open);
    assert_eq!(
        app.ui_state.preferences_nav,
        crate::preferences::PrefNav::Keyboard
    );
}
