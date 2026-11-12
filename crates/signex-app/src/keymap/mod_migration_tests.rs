//! The one-time macOS move of a custom profile's standard-set `Ctrl` rows to
//! `Mod` (#1645): only those rows change, nothing else does, redo gains a
//! macOS-only row like the built-ins, and a second run is a no-op.

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

/// `Ctrl+S` is standard, but `Mod+S` is already taken by another command.
const TAKEN_TWIN: &str = r#"
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
"Mod+S" = "print"
"Ctrl+N" = "new_document"
"#;

fn profile_of(source: &str) -> ShortcutProfile {
    TomlShortcutProfile::parse(source)
        .unwrap()
        .into_profile()
        .unwrap()
}

fn legacy() -> ShortcutProfile {
    profile_of(LEGACY)
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
    let moved = with_standard_set_on_mod(&legacy());
    let after = spellings(&moved.profile);
    // Redo's macOS row is the one added row.
    assert_eq!(before.len() + 1, after.len());
    let kept: Vec<&String> = after.iter().filter(|t| *t != "Mod+Shift+Z").collect();
    let changed: Vec<(&str, &str)> = before
        .iter()
        .zip(kept)
        .filter(|(a, b)| a != b)
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    assert_eq!(changed, [("Ctrl+S", "Mod+S"), ("Ctrl+P", "Mod+P")]);
    assert_eq!(moved.moved, 3, "two respelled rows and the redo row");
    assert!(moved.skipped.is_empty());
}

#[test]
fn collisions_chords_gestures_and_platform_rows_are_untouched() {
    let moved = with_standard_set_on_mod(&legacy()).profile;
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
        "Ctrl+Shift+Z",
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

/// The built-ins' shape: `Ctrl+Shift+Z` stays and a macOS-only `Mod+Shift+Z`
/// for the same command sits right after it.
#[test]
fn redo_keeps_its_ctrl_row_and_gains_a_macos_only_mod_row_beside_it() {
    let moved = with_standard_set_on_mod(&legacy()).profile;
    let at = moved
        .bindings
        .iter()
        .position(|b| b.triggers[0].config_text() == "Ctrl+Shift+Z")
        .expect("the Ctrl+Shift+Z row stays");
    assert_eq!(moved.bindings[at].platform, None);
    let added = &moved.bindings[at + 1];
    assert_eq!(added.triggers[0].config_text(), "Mod+Shift+Z");
    assert_eq!(added.platform, Some(Platform::MacOs));
    assert_eq!(added.action, moved.bindings[at].action);
    assert_eq!(added.context, moved.bindings[at].context);
}

#[test]
fn an_unbind_moves_with_its_binding() {
    let moved = with_standard_set_on_mod(&legacy()).profile;
    let unbind = moved
        .bindings
        .iter()
        .find(|b| matches!(b.action, ShortcutBindingAction::Unbind(_)))
        .unwrap();
    assert_eq!(unbind.triggers[0].config_text(), "Mod+P");
}

#[test]
fn a_second_run_finds_nothing_to_do_and_adds_redo_once() {
    let set = set_with(legacy());
    let once = migrate_active_to_mod(&set).expect("legacy rows move").set;
    assert_eq!(standard_ctrl_rows(once.active_profile()), 0);
    assert!(migrate_active_to_mod(&once).is_none());
    assert_eq!(
        with_standard_set_on_mod(once.active_profile()).profile,
        *once.active_profile()
    );
    let mod_redo = spellings(once.active_profile())
        .iter()
        .filter(|t| *t == "Mod+Shift+Z")
        .count();
    assert_eq!(mod_redo, 1);
}

#[test]
fn a_row_whose_mod_twin_is_already_bound_is_skipped_and_reported() {
    let moved = with_standard_set_on_mod(&profile_of(TAKEN_TWIN));
    let text = spellings(&moved.profile);
    assert!(text.iter().any(|t| t == "Ctrl+S"), "Ctrl+S stays: {text:?}");
    assert!(text.iter().any(|t| t == "Mod+N"), "Ctrl+N still moves");
    assert_eq!(moved.skipped, ["Ctrl+S"]);
    assert_eq!(moved.moved, 1);
}

#[test]
fn the_prompt_applies_only_on_macos_to_a_custom_profile_with_standard_rows() {
    let set = set_with(legacy());
    assert!(offers_mod_migration(&set, Platform::MacOs));
    assert!(!offers_mod_migration(&set, Platform::Windows));
    assert!(!offers_mod_migration(&set, Platform::Linux));
    let built_in = ShortcutProfileSet::built_ins().unwrap();
    assert!(!offers_mod_migration(&built_in, Platform::MacOs));
    let moved = migrate_active_to_mod(&set).unwrap().set;
    assert!(!offers_mod_migration(&moved, Platform::MacOs));
}
