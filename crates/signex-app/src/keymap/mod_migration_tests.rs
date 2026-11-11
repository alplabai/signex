//! The one-time macOS move of a custom profile's standard-set `Ctrl` rows to
//! `Mod` (#1645): only those rows change, nothing else does, and a second
//! run is a no-op.

use super::{
    Platform, ShortcutBindingAction, ShortcutProfile, ShortcutProfileSet, ShortcutTrigger,
    TomlShortcutProfile, migrate_active_to_mod, offers_mod_migration, standard_ctrl_rows,
    with_standard_set_on_mod,
};

const LEGACY: &str = r#"
[signex_settings]
application = "signex"
file_kind = "keyboard_shortcuts"
version = 1

[keyboard_shortcuts]
schema_version = 1
profile_id = "mine"
profile_name = "Mine"
profile_kind = "custom"

[[keyboard_shortcuts.sections]]
context = "global"

[keyboard_shortcuts.sections.bindings]
"Ctrl+S" = "save_document"
"Ctrl+Shift+Z" = "redo"
"Ctrl+Q" = "cycle_unit"
"Ctrl+W" = "place_wire"
"Ctrl+H" = "find_and_replace"
"Ctrl+Alt+S" = "save_document_as"
"Ctrl+K S" = "open_preferences"
"P W" = "place_wire"
"Mod+Z" = "undo"

[keyboard_shortcuts.sections.unbind]
"Ctrl+P" = "print"

[[keyboard_shortcuts.sections]]
platform = "windows"
context = "global"

[keyboard_shortcuts.sections.bindings]
"Ctrl+O" = "open_document"
"#;

fn legacy() -> ShortcutProfile {
    TomlShortcutProfile::parse(LEGACY)
        .unwrap()
        .into_profile()
        .unwrap()
}

fn set_with(profile: ShortcutProfile) -> ShortcutProfileSet {
    let mut set = ShortcutProfileSet::built_ins().unwrap();
    let id = profile.id.clone();
    set.insert_custom_profile(profile).unwrap();
    set.set_active_profile(id).unwrap();
    set
}

fn spellings(profile: &ShortcutProfile) -> Vec<String> {
    profile
        .bindings
        .iter()
        .flat_map(|b| b.triggers.iter().map(ShortcutTrigger::config_text))
        .collect()
}

#[test]
fn only_standard_set_rows_move_to_mod() {
    let before = spellings(&legacy());
    let after = spellings(&with_standard_set_on_mod(&legacy()));
    assert_eq!(before.len(), after.len());
    let changed: Vec<(&str, &str)> = before
        .iter()
        .zip(&after)
        .filter(|(a, b)| a != b)
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    assert_eq!(
        changed,
        [
            ("Ctrl+S", "Mod+S"),
            ("Ctrl+Shift+Z", "Mod+Shift+Z"),
            ("Ctrl+P", "Mod+P"),
        ]
    );
}

#[test]
fn collisions_chords_gestures_and_platform_rows_are_untouched() {
    let moved = with_standard_set_on_mod(&legacy());
    let text = spellings(&moved);
    for kept in [
        "Ctrl+Q",
        "Ctrl+W",
        "Ctrl+H",
        "Ctrl+Alt+S",
        "Ctrl+K S",
        "P W",
        "Mod+Z",
        "Ctrl+O",
    ] {
        assert!(text.iter().any(|t| t == kept), "{kept} changed: {text:?}");
    }
    let windows_row = moved
        .bindings
        .iter()
        .find(|b| b.platform == Some(Platform::Windows))
        .expect("the platform section survives");
    assert_eq!(windows_row.triggers[0].config_text(), "Ctrl+O");
}

#[test]
fn an_unbind_moves_with_its_binding() {
    let moved = with_standard_set_on_mod(&legacy());
    let unbind = moved
        .bindings
        .iter()
        .find(|b| matches!(b.action, ShortcutBindingAction::Unbind(_)))
        .unwrap();
    assert_eq!(unbind.triggers[0].config_text(), "Mod+P");
}

#[test]
fn a_second_run_finds_nothing_to_do() {
    let set = set_with(legacy());
    let once = migrate_active_to_mod(&set).expect("legacy rows move");
    assert_eq!(standard_ctrl_rows(once.active_profile()), 0);
    assert!(migrate_active_to_mod(&once).is_none());
    assert_eq!(
        with_standard_set_on_mod(once.active_profile()),
        *once.active_profile()
    );
}

#[test]
fn the_prompt_applies_only_on_macos_to_a_custom_profile_with_standard_rows() {
    let set = set_with(legacy());
    assert!(offers_mod_migration(&set, Platform::MacOs));
    assert!(!offers_mod_migration(&set, Platform::Windows));
    assert!(!offers_mod_migration(&set, Platform::Linux));
    let built_in = ShortcutProfileSet::built_ins().unwrap();
    assert!(!offers_mod_migration(&built_in, Platform::MacOs));
    let moved = migrate_active_to_mod(&set).unwrap();
    assert!(!offers_mod_migration(&moved, Platform::MacOs));
}
