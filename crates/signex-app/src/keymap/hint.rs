//! Hard-coded shortcut hints (menu fallbacks, help text) shown per OS.

use crate::keymap::{KeyStroke, KeyToken, Platform, ShortcutTrigger};

/// Keys of the platform-standard set (the ones the built-in profiles spell
/// `Mod+…`), with whether Shift is held.
const STANDARD_KEYS: [(&str, bool); 25] = [
    ("n", false),
    ("o", false),
    ("s", false),
    ("p", false),
    ("s", true),
    ("z", false),
    ("z", true),
    ("x", false),
    ("c", false),
    ("v", false),
    ("a", false),
    ("f", false),
    (",", false),
    ("p", true),
    ("d", false),
    ("r", false),
    ("e", false),
    ("l", false),
    ("4", false),
    ("v", true),
    ("g", true),
    ("x", true),
    ("e", true),
    ("f", true),
    ("a", true),
];

/// Whether `key` (lower case) with `shift` is in the platform-standard set.
pub fn is_standard_chord(key: &str, shift: bool) -> bool {
    STANDARD_KEYS.contains(&(key, shift))
}

/// Whether `stroke` is a Ctrl chord of the platform-standard set, which
/// macOS shows (and runs) as Cmd.
fn is_standard_ctrl_chord(stroke: &KeyStroke) -> bool {
    let m = stroke.modifiers;
    if !m.ctrl || m.alt || m.command || m.primary {
        return false;
    }
    let KeyToken::Character(key) = &stroke.key else {
        return false;
    };
    is_standard_chord(key, m.shift)
}

/// A fallback hint written for Windows/Linux (`Ctrl+S`) as `platform` shows
/// it. Off macOS the text is returned unchanged; on macOS the standard set
/// reads as Cmd and every other chord keeps its Control, in Apple symbols.
/// Text that is not a key sequence is returned unchanged.
pub fn fallback_hint(text: &str, platform: Platform) -> String {
    if platform != Platform::MacOs {
        return text.to_owned();
    }
    let Ok(ShortcutTrigger::KeySequence(strokes)) = ShortcutTrigger::parse(text) else {
        return text.to_owned();
    };
    let strokes = strokes
        .into_iter()
        .map(|mut stroke| {
            if is_standard_ctrl_chord(&stroke) {
                stroke.modifiers.ctrl = false;
                stroke.modifiers.primary = true;
            }
            stroke
        })
        .collect();
    ShortcutTrigger::KeySequence(strokes).display_text_for(platform)
}
