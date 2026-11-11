//! The `Mod` token, platform sections, per-OS display, recording, reserved
//! chords and the physical-key fallback (#1645). Every test pins a
//! `Platform`, so each OS's rule runs on every CI runner.

use super::catalog_golden_tests::profiles_text_for;
use super::{
    AppCommandId, CompiledKeymap, KeyStroke, Modifiers, Platform, ShortcutBinding,
    ShortcutBindingAction, ShortcutContext, ShortcutProfile, ShortcutProfileKind,
    ShortcutProfileSet, ShortcutTrigger, TomlShortcutProfile,
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

fn keymap_with(platform: Platform, bindings: &[(&str, &str)]) -> CompiledKeymap {
    let bindings = bindings
        .iter()
        .map(|(trigger, id)| ShortcutBinding {
            action: ShortcutBindingAction::Command(command(id)),
            context: ShortcutContext::Global,
            triggers: vec![ShortcutTrigger::parse(trigger).unwrap()],
        })
        .collect();
    let profile = ShortcutProfile {
        id: "t".to_string(),
        name: "T".to_string(),
        kind: ShortcutProfileKind::Custom,
        schema_version: 1,
        description: None,
        base_profile: None,
        bindings,
    };
    CompiledKeymap::compile_for(&profile, platform)
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
    let ids = |platform| {
        TomlShortcutProfile::parse(source)
            .unwrap()
            .into_profile_for(platform)
            .unwrap()
            .bindings
            .iter()
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

#[test]
fn a_displayed_macos_label_parses_back_as_mod() {
    let shown = stroke("Mod+Shift+S").display_for(Platform::MacOs);
    assert_eq!(stroke(&shown), stroke("Mod+Shift+S"));
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

#[test]
fn a_non_ascii_logical_key_falls_back_to_the_physical_us_key() {
    let from = |logical: &str, code| {
        KeyStroke::from_key_event(
            &Key::Character(logical.into()),
            Physical::Code(code),
            Held::LOGO,
        )
        .unwrap()
    };
    assert_eq!(from("\u{f6}", Code::Comma).key.to_string(), ",");
    assert_eq!(from("\u{131}", Code::KeyI).key.to_string(), "I");
    assert_eq!(from("\u{444}", Code::KeyA).key.to_string(), "A");
    assert_eq!(from("\u{c7}", Code::Period).key.to_string(), ".");
}

#[test]
fn an_ascii_logical_key_beats_the_physical_one() {
    // AZERTY: the key at physical KeyW types `z`.
    let azerty = KeyStroke::from_key_event(
        &Key::Character("z".into()),
        Physical::Code(Code::KeyW),
        Held::CTRL,
    )
    .unwrap();
    assert_eq!(azerty.key.to_string(), "Z");
}

#[test]
fn an_unidentified_physical_key_keeps_the_logical_character() {
    let stroke = KeyStroke::from_key_event(
        &Key::Character("\u{f6}".into()),
        Physical::Unidentified(keyboard::key::NativeCode::Unidentified),
        Held::empty(),
    )
    .unwrap();
    assert_eq!(stroke.key.to_string(), "\u{d6}");
}

/// Windows and Linux resolve both built-in profiles exactly as trunk did:
/// `builtin_profiles.golden.tsv` is that table (unchanged by #1645).
#[test]
fn windows_and_linux_resolve_the_built_ins_exactly_as_trunk() {
    let golden = include_str!("testdata/builtin_profiles.golden.tsv");
    for platform in [Platform::Windows, Platform::Linux] {
        assert_eq!(profiles_text_for(platform), golden, "{platform:?}");
    }
    let table = |platform| {
        let set = ShortcutProfileSet::built_ins_for(platform).unwrap();
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
            .into_profile_for(platform)
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
