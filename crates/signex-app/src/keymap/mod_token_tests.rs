//! The `Mod` token, platform sections, per-OS display, recording, reserved
//! chords and the physical-key fallback (#1645). Every test pins a
//! `Platform`, so each OS's rule runs on every CI runner.

use super::catalog_golden_tests::{Render, profiles_text_for};
use super::{
    AppCommandId, CompiledKeymap, KeyStroke, KeymapEditorModel, Modifiers, Platform,
    ShortcutBinding, ShortcutBindingAction, ShortcutContext, ShortcutProfile, ShortcutProfileKind,
    ShortcutProfileSet, ShortcutTrigger, TomlShortcutProfile, export_custom_profile,
    import_custom_profile, load_profile_set_at, save_profile_set_at,
};
use iced::keyboard::{self, Key, Modifiers as Held, key::Code, key::Physical};
use std::str::FromStr;

const ALL: [Platform; 3] = [Platform::MacOs, Platform::Windows, Platform::Linux];

fn stroke(text: &str) -> KeyStroke {
    KeyStroke::from_str(text).unwrap()
}

fn live(key: &str, held: Held) -> KeyStroke {
    KeyStroke::from_iced(&Key::Character(key.into()), held).unwrap()
}

fn command(id: &str) -> AppCommandId {
    AppCommandId::new(id).unwrap()
}

fn keymap_profile(bindings: &[(&str, &str)]) -> ShortcutProfile {
    let bindings = bindings
        .iter()
        .map(|(trigger, id)| ShortcutBinding {
            action: ShortcutBindingAction::Command(command(id)),
            context: ShortcutContext::Global,
            triggers: vec![ShortcutTrigger::parse(trigger).unwrap()],
            platform: None,
        })
        .collect();
    ShortcutProfile {
        id: "t".to_string(),
        name: "T".to_string(),
        kind: ShortcutProfileKind::Custom,
        schema_version: 1,
        description: None,
        base_profile: None,
        bindings,
    }
}

fn keymap_with(platform: Platform, bindings: &[(&str, &str)]) -> CompiledKeymap {
    CompiledKeymap::compile_for(&keymap_profile(bindings), platform)
}

fn lookup(keymap: &CompiledKeymap, stroke: KeyStroke) -> Option<AppCommandId> {
    keymap.lookup(&[stroke], &[ShortcutContext::Global]).command
}

#[test]
fn mod_parses_and_writes_back_as_mod_on_every_os() {
    let parsed = stroke("Mod+Shift+S");
    assert!(parsed.modifiers.primary && parsed.modifiers.shift);
    assert!(!parsed.modifiers.ctrl && !parsed.modifiers.command);
    assert_eq!(parsed.to_config_string(), "Mod+Shift+S");
    assert_eq!(stroke(&parsed.to_config_string()), parsed);
}

#[test]
fn mod_resolves_to_command_on_macos_and_ctrl_elsewhere_never_super() {
    let m = stroke("Mod+S").modifiers;
    let mac = m.resolved_for(Platform::MacOs);
    assert!(mac.command && !mac.ctrl && !mac.primary);
    for platform in [Platform::Windows, Platform::Linux] {
        let resolved = m.resolved_for(platform);
        assert!(resolved.ctrl && !resolved.command && !resolved.primary);
    }
}

#[test]
fn explicit_ctrl_and_cmd_stay_literal_on_every_os() {
    for platform in ALL {
        assert_eq!(
            stroke("Ctrl+S").modifiers.resolved_for(platform),
            stroke("Ctrl+S").modifiers
        );
        assert_eq!(
            stroke("Cmd+S").modifiers.resolved_for(platform),
            stroke("Cmd+S").modifiers
        );
    }
}

#[test]
fn mod_s_matches_command_s_on_macos_and_ctrl_s_elsewhere() {
    let cmd_s = live("s", Held::LOGO);
    let ctrl_s = live("s", Held::CTRL);
    let mac = keymap_with(Platform::MacOs, &[("Mod+S", "save_document")]);
    assert_eq!(lookup(&mac, cmd_s.clone()), Some(command("save_document")));
    assert_eq!(
        lookup(&mac, ctrl_s.clone()),
        None,
        "Control is not Command on macOS"
    );
    for platform in [Platform::Windows, Platform::Linux] {
        let other = keymap_with(platform, &[("Mod+S", "save_document")]);
        assert_eq!(
            lookup(&other, ctrl_s.clone()),
            Some(command("save_document"))
        );
        assert_eq!(
            lookup(&other, cmd_s.clone()),
            None,
            "Super never triggers Mod"
        );
    }
}

#[test]
fn a_platform_section_applies_only_on_its_platform() {
    let source = r#"
[signex_settings]
application = "signex"
file_kind = "keyboard_shortcuts"
version = 1

[keyboard_shortcuts]
schema_version = 1
profile_id = "t"
profile_name = "T"
profile_kind = "custom"

[[keyboard_shortcuts.sections]]
context = "global"
[keyboard_shortcuts.sections.bindings]
"F7" = "run_erc"

[[keyboard_shortcuts.sections]]
platform = "macos"
context = "global"
[keyboard_shortcuts.sections.bindings]
"Mod+W" = "close_active_document"
"#;
    let profile = TomlShortcutProfile::parse(source)
        .unwrap()
        .into_profile()
        .unwrap();
    let ids = |platform| {
        CompiledKeymap::compile_for(&profile, platform)
            .resolved_table()
            .into_iter()
            .map(|(_, _, action)| action)
            .collect::<Vec<_>>()
    };
    let (mac, windows) = (ids(Platform::MacOs), ids(Platform::Windows));
    assert_eq!((mac.len(), windows.len()), (2, 1));
    assert!(mac[1].contains("close_active_document"));
    let ids = |platform| {
        profile
            .bindings
            .iter()
            .filter(|b| b.applies_on(platform))
            .filter_map(|b| b.action.command().map(|c| c.as_str().to_string()))
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(Platform::MacOs), ["run_erc", "close_active_document"]);
    assert_eq!(ids(Platform::Windows), ["run_erc"]);
    assert_eq!(ids(Platform::Linux), ["run_erc"]);
}

#[test]
fn macos_shows_apple_symbols_in_apple_order_without_plus() {
    let show = |text: &str| stroke(text).display_for(Platform::MacOs);
    assert_eq!(show("Mod+Shift+S"), "\u{21e7}\u{2318}S");
    assert_eq!(show("Ctrl+Q"), "\u{2303}Q");
    assert_eq!(show("Mod+Alt+F"), "\u{2325}\u{2318}F");
    assert_eq!(
        show("Shift+Alt+Ctrl+Cmd+K"),
        "\u{2303}\u{2325}\u{21e7}\u{2318}K"
    );
}

#[test]
fn other_oses_keep_the_text_form() {
    for platform in [Platform::Windows, Platform::Linux] {
        assert_eq!(stroke("Mod+Shift+S").display_for(platform), "Ctrl+Shift+S");
        assert_eq!(stroke("Ctrl+Q").display_for(platform), "Ctrl+Q");
    }
    assert_eq!(stroke("Cmd+W").display_for(Platform::Windows), "Win+W");
    assert_eq!(stroke("Cmd+W").display_for(Platform::Linux), "Super+W");
}

/// #1378: what a profile file says parses back to the same stroke on every
/// OS; the text shown on Windows and Linux does too. The macOS label cannot
/// tell `Cmd` from `Mod`, so it is never parsed: the UI carries the file
/// spelling or the parsed stroke instead.
#[test]
fn what_is_shown_or_saved_parses_back_to_the_same_stroke() {
    for text in ["Mod+Shift+S", "Ctrl+Q", "Cmd+K", "Shift+F1", "Alt+Left"] {
        let original = stroke(text);
        assert_eq!(stroke(&original.to_config_string()), original, "{text}");
        for platform in [Platform::Windows, Platform::Linux] {
            let shown = original.display_for(platform);
            assert_eq!(stroke(&shown), original.resolved_for(platform), "{shown}");
        }
    }
    let shown = stroke("Cmd+K").display_for(Platform::MacOs);
    assert!(
        KeyStroke::from_str(&shown).is_err(),
        "{shown} must not parse"
    );
}

#[test]
fn the_recorder_stores_mod_for_command_on_macos_and_ctrl_on_windows() {
    let cmd_s = live("s", Held::LOGO);
    let ctrl_s = live("s", Held::CTRL);
    assert_eq!(
        cmd_s.recorded_for(Platform::MacOs).to_config_string(),
        "Mod+S"
    );
    assert_eq!(
        ctrl_s.recorded_for(Platform::MacOs).to_config_string(),
        "Ctrl+S"
    );
    assert_eq!(
        ctrl_s.recorded_for(Platform::Windows).to_config_string(),
        "Ctrl+S"
    );
    assert_eq!(
        cmd_s.recorded_for(Platform::Windows).to_config_string(),
        "Cmd+S"
    );
    let held = Modifiers::from_iced(Held::LOGO | Held::SHIFT);
    assert!(held.recorded_for(Platform::MacOs).primary);
    assert!(!held.recorded_for(Platform::Windows).primary);
}

#[test]
fn macos_flags_the_chords_the_system_owns() {
    let keymap = keymap_with(
        Platform::MacOs,
        &[
            ("Mod+Q", "cycle_unit"),
            ("Mod+H", "find_and_replace"),
            ("Mod+M", "measure_distance"),
            ("Mod+Space", "cycle_wiring_mode"),
            ("Mod+Tab", "next_document_tab"),
            ("Mod+`", "place_text"),
            ("Mod+,", "find"),
            ("Mod+S", "save_document"),
        ],
    );
    let flagged: Vec<String> = keymap
        .conflicts()
        .into_iter()
        .filter(|c| c.system_reserved)
        .map(|c| c.trigger)
        .collect();
    assert_eq!(
        flagged,
        [
            "\u{2318}Q",
            "\u{2318}H",
            "\u{2318}M",
            "\u{2318}SPACE",
            "\u{2318}TAB",
            "\u{2318}`",
            "\u{2318},"
        ]
    );
}

#[test]
fn comma_is_allowed_for_preferences_and_nothing_is_flagged_off_macos() {
    let prefs = keymap_with(Platform::MacOs, &[("Mod+,", "open_preferences")]);
    assert!(prefs.conflicts().is_empty());
    for platform in [Platform::Windows, Platform::Linux] {
        let other = keymap_with(platform, &[("Mod+Q", "cycle_unit"), ("Mod+H", "find")]);
        assert!(other.conflicts().is_empty());
    }
}

fn fallback(logical: &str, code: Code, held: Held) -> Option<KeyStroke> {
    KeyStroke::physical_fallback(&Key::Character(logical.into()), Physical::Code(code), held)
}

fn shown(stroke: Option<KeyStroke>) -> Option<String> {
    stroke.map(|stroke| stroke.to_config_string())
}

/// #1645 D1: the physical key is only a retry, offered with Ctrl, Cmd or Alt
/// held and a non-ASCII logical key.
#[test]
fn a_non_ascii_logical_key_offers_the_physical_us_key_while_a_command_modifier_is_held() {
    assert_eq!(
        shown(fallback("\u{f6}", Code::Comma, Held::LOGO)).as_deref(),
        Some("Cmd+,")
    );
    assert_eq!(
        shown(fallback("\u{131}", Code::KeyI, Held::CTRL)).as_deref(),
        Some("Ctrl+I")
    );
    assert_eq!(
        shown(fallback("\u{444}", Code::KeyA, Held::ALT)).as_deref(),
        Some("Alt+A")
    );
    let ctrl_shift = Held::CTRL | Held::SHIFT;
    assert_eq!(
        shown(fallback("\u{c7}", Code::Period, ctrl_shift)).as_deref(),
        Some("Ctrl+Shift+.")
    );
}

#[test]
fn bare_and_shift_only_keys_are_never_remapped_to_the_physical_key() {
    assert_eq!(fallback("\u{e9}", Code::Digit2, Held::empty()), None);
    assert_eq!(fallback("\u{e9}", Code::Digit2, Held::SHIFT), None);
}

#[test]
fn an_ascii_logical_key_is_never_remapped() {
    // AZERTY: the key at physical KeyW types `z`.
    assert_eq!(fallback("z", Code::KeyW, Held::CTRL), None);
}

#[test]
fn an_unidentified_physical_key_offers_no_fallback() {
    let none = KeyStroke::physical_fallback(
        &Key::Character("\u{f6}".into()),
        Physical::Unidentified(keyboard::key::NativeCode::Unidentified),
        Held::CTRL,
    );
    assert_eq!(none, None);
}

/// Windows and Linux resolve both built-in profiles exactly as trunk did:
/// `builtin_profiles_resolved.golden.tsv` is that table (it was
/// `builtin_profiles.golden.tsv` on trunk, and #1645 leaves it alone).
#[test]
fn windows_and_linux_resolve_the_built_ins_exactly_as_trunk() {
    let golden = include_str!("testdata/builtin_profiles_resolved.golden.tsv");
    for platform in [Platform::Windows, Platform::Linux] {
        assert_eq!(
            profiles_text_for(Render::Resolved(platform)),
            golden,
            "{platform:?}"
        );
    }
    let table = |platform| {
        let set = ShortcutProfileSet::built_ins().unwrap();
        set.profiles()
            .map(|p| CompiledKeymap::compile_for(p, platform).resolved_table())
            .collect::<Vec<_>>()
    };
    assert_eq!(table(Platform::Windows), table(Platform::Linux));
}

/// A user profile on disk keeps its literal `Ctrl+…` / `Cmd+…` meaning on
/// every OS (nothing is rewritten to `Mod`).
#[test]
fn user_profiles_with_ctrl_and_cmd_resolve_as_on_trunk() {
    let source = include_str!("testdata/user_profile_ctrl_cmd.toml");
    for platform in ALL {
        let profile = TomlShortcutProfile::parse(source)
            .unwrap()
            .into_profile()
            .unwrap();
        let keymap = CompiledKeymap::compile_for(&profile, platform);
        assert_eq!(
            lookup(&keymap, live("s", Held::CTRL)),
            Some(command("save_document"))
        );
        assert_eq!(
            lookup(&keymap, live("o", Held::LOGO)),
            Some(command("open_document"))
        );
        assert_eq!(lookup(&keymap, live("s", Held::LOGO)), None);
        assert_eq!(lookup(&keymap, live("o", Held::CTRL)), None);
        let table = keymap.resolved_table();
        assert!(
            table.iter().all(|(_, s, _)| !s.contains("Mod+")),
            "{table:?}"
        );
    }
}

const WITH_MACOS_SECTION: &str = r#"
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
"F7" = "run_erc"

[[keyboard_shortcuts.sections]]
platform = "macos"
context = "global"
[keyboard_shortcuts.sections.bindings]
"Mod+W" = "close_active_document"
"#;

/// S1: a section for another OS is kept, listed as inactive, and written
/// back with its tag, so saving on Windows cannot delete the macOS bindings.
#[test]
fn a_platform_section_survives_load_save_load_and_a_copy() {
    let loaded = import_custom_profile(WITH_MACOS_SECTION).unwrap();
    let saved = export_custom_profile(&loaded).unwrap();
    assert!(saved.contains("platform = \"macos\""), "{saved}");
    let resaved = export_custom_profile(&import_custom_profile(&saved).unwrap()).unwrap();
    assert_eq!(saved, resaved, "load, save, load, save is stable");

    let copy = loaded.copy_as_custom("copy", "Copy").unwrap();
    let tagged = |profile: &ShortcutProfile| {
        profile
            .bindings
            .iter()
            .filter(|b| b.platform == Some(Platform::MacOs))
            .count()
    };
    assert_eq!((tagged(&loaded), tagged(&copy)), (1, 1));
    for platform in ALL {
        let rows = CompiledKeymap::compile_for(&loaded, platform).resolved_table();
        assert_eq!(rows.len(), if platform == Platform::MacOs { 2 } else { 1 });
    }
}

/// The same through the user's file: save then load keeps the tagged section.
#[test]
fn a_platform_section_survives_the_user_file() {
    let loaded = import_custom_profile(WITH_MACOS_SECTION).unwrap();
    let mut set = ShortcutProfileSet::built_ins().unwrap();
    set.insert_custom_profile(loaded).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("keyboard_shortcuts.toml");
    save_profile_set_at(&path, &set).unwrap();
    let first = std::fs::read(&path).unwrap();
    let reloaded = load_profile_set_at(&path).unwrap();
    save_profile_set_at(&path, &reloaded).unwrap();
    assert_eq!(first, std::fs::read(&path).unwrap());
    assert!(
        String::from_utf8(first)
            .unwrap()
            .contains("platform = \"macos\"")
    );
    let mine = reloaded.profile("mine").unwrap();
    assert!(
        mine.bindings
            .iter()
            .any(|b| b.platform == Some(Platform::MacOs))
    );
}

/// A typo in a section for another OS fails to load on every OS.
#[test]
fn a_typo_in_another_platforms_section_fails_everywhere() {
    let broken = WITH_MACOS_SECTION.replace("Mod+W", "Mod+NoSuchKey");
    assert!(import_custom_profile(&broken).is_err());
}

/// B2: a macOS profile with an explicit `Cmd+K`; opening the recorder on
/// that row and applying without recording keeps `Cmd+K`, not `Mod+K`.
#[test]
fn applying_the_recorder_untouched_keeps_an_explicit_cmd_on_macos() {
    let mut set = ShortcutProfileSet::built_ins().unwrap();
    let mine = custom_with(&[("Cmd+K", "find")]);
    set.insert_custom_profile(mine).unwrap();
    set.set_active_profile("t").unwrap();
    let mut editor = KeymapEditorModel::new(set).with_platform(Platform::MacOs);
    let row = editor.rows().remove(0);
    assert_eq!(row.trigger, "\u{2318}K", "shown as the Mac shows it");
    assert_eq!(row.trigger_config, "Cmd+K");

    let recorder = crate::app::KeymapRecorderState::new(
        row.slot,
        row.command.clone().unwrap(),
        row.label.clone(),
        row.context,
        row.trigger_config.clone(),
    );
    editor
        .edit_active_trigger_parsed(recorder.slot, recorder.trigger())
        .unwrap();
    let saved = export_custom_profile(editor.active_profile()).unwrap();
    assert!(saved.contains("\"Cmd+K\""), "{saved}");
    assert!(!saved.contains("Mod+K"), "{saved}");
    assert_eq!(editor.rows()[0].trigger, "\u{2318}K");
}

/// An `Unbind` in a macOS-only section must not mark the row Unbound on
/// Windows, where it is not compiled and Ctrl+S still saves.
#[test]
fn an_unbind_for_another_os_does_not_mark_the_row_unbound() {
    let source = WITH_MACOS_SECTION
        .replace("\"F7\" = \"run_erc\"", "\"Ctrl+S\" = \"save_document\"")
        .replace(
            "\"Mod+W\" = \"close_active_document\"",
            "\"Mod+W\" = \"close_active_document\"\n[keyboard_shortcuts.sections.unbind]\n\"Ctrl+S\" = \"save_document\"",
        );
    let profile = import_custom_profile(&source).unwrap();
    let mut set = ShortcutProfileSet::built_ins().unwrap();
    set.insert_custom_profile(profile).unwrap();
    set.set_active_profile("mine").unwrap();
    let unbound_on = |platform| {
        let editor = KeymapEditorModel::new(set.clone()).with_platform(platform);
        let row = editor
            .rows()
            .into_iter()
            .find(|row| row.command.as_ref().map(|c| c.as_str()) == Some("save_document"))
            .expect("the Save row is listed");
        row.unbound
    };
    assert!(!unbound_on(Platform::Windows));
    assert!(!unbound_on(Platform::Linux));
    assert!(unbound_on(Platform::MacOs));
}

/// AltGr arrives as Ctrl+Alt on Windows: `ą` is a character, not a chord.
#[test]
fn altgr_is_not_retried_as_a_physical_chord_but_ctrl_alone_still_is() {
    assert_eq!(
        fallback("\u{105}", Code::KeyA, Held::CTRL | Held::ALT),
        None
    );
    assert_eq!(
        shown(fallback("\u{f6}", Code::Comma, Held::CTRL)).as_deref(),
        Some("Ctrl+,")
    );
    // With Cmd also held the chord is deliberate and still retried.
    assert!(fallback("\u{f6}", Code::Comma, Held::CTRL | Held::ALT | Held::LOGO).is_some());
}

fn custom_with(bindings: &[(&str, &str)]) -> ShortcutProfile {
    let mut profile = keymap_profile(bindings);
    profile.id = "t".to_string();
    profile
}
