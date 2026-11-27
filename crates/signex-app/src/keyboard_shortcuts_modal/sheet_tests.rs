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
    // A schematic in front opens on Schematic; no editor at all on All.
    let expected = if app.has_active_schematic() {
        ShortcutScope::Schematic
    } else {
        ShortcutScope::All
    };
    let _ = app.update(Message::Menu(MenuMessage::OpenKeyboardShortcuts));
    assert!(app.ui_state.keyboard_shortcuts_open);
    assert_eq!(app.ui_state.shortcut_sheet, ShortcutSheet::opened(expected));
}

/// A footprint editor in front: neither Schematic nor PCB lists its rows,
/// so the sheet opens on All and the footprint shortcuts are there.
#[test]
fn a_footprint_editor_in_front_opens_the_sheet_on_all() {
    use crate::app::{FootprintDocument, TabInfo, TabKind};
    let mut app = app();
    let path = std::path::PathBuf::from("/w/part.snxfpt");
    let file =
        signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("fp"));
    app.document_state
        .footprint_editors
        .insert(path.clone(), FootprintDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "part".to_string(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::FootprintEditor(path),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    let _ = app.update(Message::Menu(MenuMessage::OpenKeyboardShortcuts));
    assert_eq!(app.ui_state.shortcut_sheet.scope, ShortcutScope::All);
}

/// Mockup row 13, #3047: the "wire" filter typed through `update` lists
/// each command once, with every key it has, and counts commands.
#[test]
fn a_filter_lists_each_command_once_with_all_its_keys() {
    let mut app = app();
    let _ = app.update(Message::Menu(MenuMessage::OpenKeyboardShortcuts));
    let _ = app.update(Message::Overlay(OverlayMsg::ShortcutsFilterChanged(
        "wire".into(),
    )));
    let ui = &app.ui_state;
    let sections =
        super::build_sections_with(&ui.i18n, &ui.keymap_profiles, &ui.extension_commands);
    let shown = super::filter::listing(&ui.i18n, &sections, &ui.shortcut_sheet);
    let entries: Vec<_> = shown.sections.iter().flat_map(|s| &s.entries).collect();
    assert!(!entries.is_empty(), "wire matches a command");
    assert_eq!(shown.shown, entries.len(), "the count counts commands");
    let mut commands = std::collections::BTreeSet::new();
    for entry in &entries {
        let id = entry.rows[0]
            .command
            .as_ref()
            .map(|c| c.as_str().to_string());
        assert!(
            commands.insert((id, entry.label().to_string())),
            "{} is listed twice",
            entry.label()
        );
        assert!(
            entry
                .rows
                .iter()
                .all(|row| row.command == entry.rows[0].command),
            "one entry holds one command's keys"
        );
    }
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
