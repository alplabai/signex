//! #2314: modifier and key names come from the catalog and follow a runtime
//! language switch; profile files keep the English spellings.

use signex_i18n::LanguagePref;

use super::*;
use crate::app::UiMsg;
use crate::keymap::{
    KeyStroke, KeymapEditorModel, KeymapEditorRow, Platform, ShortcutTrigger, load_profile_set_at,
    save_profile_set_at,
};
use std::str::FromStr;

fn switch(app: &mut Signex, tag: &str) {
    let pref = LanguagePref::Locale(tag.to_owned());
    let _ = app.update(Message::Ui(UiMsg::LanguageChanged(pref)));
}

fn shortcut_in(menu: &Menu, command: &str) -> Option<String> {
    menu.entries.iter().find_map(|entry| match entry {
        MenuEntry::Item(item) => item
            .shortcut
            .as_ref()
            .filter(|s| s.command_id.as_str() == command)
            .map(|s| s.text.clone()),
        MenuEntry::Submenu(sub) => shortcut_in(sub, command),
        MenuEntry::Separator => None,
    })
}

/// The shortcut text the main window's bar shows for `command`.
fn menu_shortcut(app: &Signex, command: &str) -> String {
    let model = menu_bar_model(&menu_context(app));
    model
        .menus
        .iter()
        .find_map(|menu| shortcut_in(menu, command))
        .unwrap_or_else(|| panic!("no menu row shows a shortcut for {command}"))
}

/// The Preferences rows as the pane draws them, on Windows so the modifier
/// words show on every host.
fn preference_rows(app: &Signex, query: &str) -> Vec<KeymapEditorRow> {
    let editor = app
        .ui_state
        .preferences_keymap_editor
        .clone()
        .with_platform(Platform::Windows);
    editor.filtered_rows_in(&app.ui_state.i18n, query)
}

fn ctrl_row(rows: &[KeymapEditorRow]) -> &KeymapEditorRow {
    rows.iter()
        .find(|row| row.trigger_config.starts_with("Ctrl+"))
        .expect("a Ctrl row")
}

#[test]
fn a_language_switch_renames_the_keys_in_menus_and_preferences() {
    let mut app = bare_app();
    switch(&mut app, "en-US");
    app.ui_state.preferences_keymap_editor =
        KeymapEditorModel::new(app.ui_state.keymap_profiles.clone());
    let english_menu = menu_shortcut(&app, "delete_selection");
    assert_eq!(english_menu, "DEL");
    let rows = preference_rows(&app, "");
    let english_row = ctrl_row(&rows).clone();
    assert!(english_row.trigger.starts_with("Ctrl+"));

    switch(&mut app, "en-XA");
    let pseudo_menu = menu_shortcut(&app, "delete_selection");
    assert_ne!(
        pseudo_menu, english_menu,
        "the menu row follows the language"
    );
    let rows = preference_rows(&app, "");
    let pseudo_row = rows
        .iter()
        .find(|row| row.slot == english_row.slot)
        .expect("same row");
    assert!(
        !pseudo_row.trigger.starts_with("Ctrl+"),
        "the Preferences row follows the language: {}",
        pseudo_row.trigger
    );
    assert_eq!(pseudo_row.trigger_config, english_row.trigger_config);

    switch(&mut app, "en-US");
    assert_eq!(menu_shortcut(&app, "delete_selection"), english_menu);
    let rows = preference_rows(&app, "");
    let back = rows.iter().find(|row| row.slot == english_row.slot);
    assert_eq!(back.map(|row| &row.trigger), Some(&english_row.trigger));
}

#[test]
fn search_finds_a_key_by_its_english_and_its_shown_name() {
    let mut app = bare_app();
    switch(&mut app, "en-XA");
    app.ui_state.preferences_keymap_editor =
        KeymapEditorModel::new(app.ui_state.keymap_profiles.clone());
    let all = preference_rows(&app, "");
    let shown = ctrl_row(&all).trigger.clone();
    let shown_ctrl = shown.split('+').next().unwrap_or_default().to_owned();
    assert_ne!(shown_ctrl, "Ctrl", "en-XA renames Ctrl");

    let by_english = preference_rows(&app, "ctrl");
    assert!(by_english.iter().any(|row| row.trigger == shown));
    let by_shown = preference_rows(&app, &shown_ctrl);
    assert!(by_shown.iter().any(|row| row.trigger == shown));
    assert!(
        preference_rows(&app, "del")
            .iter()
            .any(|row| { row.trigger_config == "DEL" && row.trigger != "DEL" })
    );
}

#[test]
fn a_profile_saved_under_a_translation_keeps_english_tokens() {
    let mut app = bare_app();
    switch(&mut app, "en-XA");
    let mut editor = KeymapEditorModel::new(app.ui_state.keymap_profiles.clone())
        .with_platform(Platform::Windows);
    editor.create_custom_from_active("keys", "Keys").unwrap();
    let slot = editor.rows_in(&app.ui_state.i18n)[0].slot;
    let stroke = KeyStroke::from_str("Ctrl+Shift+Del").unwrap();
    editor
        .edit_active_trigger_parsed(slot, ShortcutTrigger::KeySequence(vec![stroke]))
        .unwrap();

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("keyboard-shortcuts.toml");
    save_profile_set_at(&path, editor.profile_set()).unwrap();
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(written.contains("\"Ctrl+Shift+DEL\""), "{written}");
    let loaded = load_profile_set_at(&path).unwrap();
    assert_eq!(&loaded, editor.profile_set());
    // The recorded row itself is shown in the language in force.
    assert!(
        !editor.rows_in(&app.ui_state.i18n)[0]
            .trigger
            .starts_with("Ctrl+")
    );
}
