//! The spellings saved keymap profiles carry (`context = "…"`,
//! `platform = "…"`) and the logo-key names shown and parsed. A rename here
//! breaks every profile on disk, so each string is written out, not derived.

use crate::{Modifiers, Platform, ShortcutContext};

/// The profile spelling of `context`; the match fails to compile when a
/// variant is added, so the list below cannot fall behind.
fn context_spelling(context: ShortcutContext) -> &'static str {
    match context {
        ShortcutContext::Global => "global",
        ShortcutContext::Schematic => "schematic",
        ShortcutContext::Footprint => "footprint",
        ShortcutContext::Pcb => "pcb",
        ShortcutContext::Library => "library",
        ShortcutContext::Modal => "modal",
        ShortcutContext::TextInput => "text_input",
        ShortcutContext::CommandPalette => "command_palette",
        ShortcutContext::Placement => "placement",
    }
}

const CONTEXTS: [ShortcutContext; 9] = [
    ShortcutContext::Global,
    ShortcutContext::Schematic,
    ShortcutContext::Footprint,
    ShortcutContext::Pcb,
    ShortcutContext::Library,
    ShortcutContext::Modal,
    ShortcutContext::TextInput,
    ShortcutContext::CommandPalette,
    ShortcutContext::Placement,
];

/// The profile spelling of `platform`, exhaustive for the same reason.
fn platform_spelling(platform: Platform) -> &'static str {
    match platform {
        Platform::MacOs => "macos",
        Platform::Windows => "windows",
        Platform::Linux => "linux",
    }
}

const PLATFORMS: [Platform; 3] = [Platform::MacOs, Platform::Windows, Platform::Linux];

#[test]
fn every_shortcut_context_keeps_its_profile_spelling() {
    for context in CONTEXTS {
        let quoted = format!("\"{}\"", context_spelling(context));
        assert_eq!(serde_json::to_string(&context).unwrap(), quoted);
        let back: ShortcutContext = serde_json::from_str(&quoted).unwrap();
        assert_eq!(back, context);
    }
}

#[test]
fn a_section_without_a_context_is_global() {
    assert_eq!(ShortcutContext::default(), ShortcutContext::Global);
}

#[test]
fn every_platform_keeps_its_profile_spelling() {
    for platform in PLATFORMS {
        let quoted = format!("\"{}\"", platform_spelling(platform));
        assert_eq!(serde_json::to_string(&platform).unwrap(), quoted);
        let back: Platform = serde_json::from_str(&quoted).unwrap();
        assert_eq!(back, platform);
    }
}

#[test]
fn each_platform_names_its_logo_key() {
    assert_eq!(Platform::MacOs.logo_key_name(), "Cmd");
    assert_eq!(Platform::Windows.logo_key_name(), "Win");
    assert_eq!(Platform::Linux.logo_key_name(), "Super");
}

#[test]
fn the_command_key_name_is_the_logo_key_of_this_os() {
    let expected = if cfg!(target_os = "macos") {
        "Cmd"
    } else if cfg!(windows) {
        "Win"
    } else {
        "Super"
    };
    assert_eq!(Modifiers::command_key_name(), expected);
    assert_eq!(Modifiers::CONFIG_COMMAND_NAME, "Cmd");
}
