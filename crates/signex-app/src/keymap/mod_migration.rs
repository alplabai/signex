//! Moving a custom profile's standard-set `Ctrl+<k>` rows to `Mod+<k>` for
//! the one-time macOS prompt (#1645). Pure: no IO, no prompt state.
//!
//! Only the platform-standard set moves. Chords whose Command twin is a
//! macOS system shortcut (`Ctrl+Q`, `Ctrl+W`, `Ctrl+H`, ...), chords with
//! more than one stroke, pointer gestures and platform-tagged rows stay as
//! they are, so a second run finds nothing to do.

use super::{
    KeyStroke, KeyToken, Modifiers, Platform, ShortcutBinding, ShortcutProfile,
    ShortcutProfileKind, ShortcutProfileSet, ShortcutTrigger,
};

/// The approved standard set: `(lower-case key, shift)` chords that are
/// `Mod` in the built-in profiles.
const STANDARD_SET: [(&str, bool); 25] = [
    ("n", false),
    ("o", false),
    ("s", false),
    ("p", false),
    ("z", false),
    ("x", false),
    ("c", false),
    ("v", false),
    ("a", false),
    ("f", false),
    (",", false),
    ("d", false),
    ("r", false),
    ("e", false),
    ("l", false),
    ("4", false),
    ("s", true),
    ("z", true),
    ("p", true),
    ("v", true),
    ("g", true),
    ("x", true),
    ("e", true),
    ("f", true),
    ("a", true),
];

/// `Ctrl+<k>` or `Ctrl+Shift+<k>` of the standard set, spelled with the
/// physical Control key.
fn is_standard_ctrl(stroke: &KeyStroke) -> bool {
    let m = stroke.modifiers;
    if !m.ctrl || m.primary || m.alt || m.command {
        return false;
    }
    matches!(&stroke.key, KeyToken::Character(key) if STANDARD_SET.contains(&(key.as_str(), m.shift)))
}

fn standard_stroke(trigger: &ShortcutTrigger) -> Option<&KeyStroke> {
    match trigger {
        ShortcutTrigger::KeySequence(strokes) => match strokes.as_slice() {
            [only] if is_standard_ctrl(only) => Some(only),
            _ => None,
        },
        ShortcutTrigger::PointerGesture(_) => None,
    }
}

fn is_movable(binding: &ShortcutBinding) -> bool {
    binding.platform.is_none()
        && binding
            .triggers
            .iter()
            .any(|t| standard_stroke(t).is_some())
}

/// How many bindings of `profile` still spell a standard-set chord with
/// plain `Ctrl`.
pub fn standard_ctrl_rows(profile: &ShortcutProfile) -> usize {
    profile
        .bindings
        .iter()
        .filter(|binding| is_movable(binding))
        .count()
}

fn moved(trigger: &ShortcutTrigger) -> ShortcutTrigger {
    match standard_stroke(trigger) {
        Some(stroke) => ShortcutTrigger::KeySequence(vec![KeyStroke {
            modifiers: Modifiers {
                ctrl: false,
                primary: true,
                ..stroke.modifiers
            },
            key: stroke.key.clone(),
        }]),
        None => trigger.clone(),
    }
}

/// `profile` with every standard-set `Ctrl` row (commands and unbinds
/// alike, so an unbind keeps matching its binding) respelled `Mod`.
pub fn with_standard_set_on_mod(profile: &ShortcutProfile) -> ShortcutProfile {
    let mut out = profile.clone();
    for binding in out.bindings.iter_mut().filter(|b| b.platform.is_none()) {
        binding.triggers = binding.triggers.iter().map(moved).collect();
    }
    out
}

/// The set with its active custom profile moved to `Mod`, or `None` when
/// there is nothing to move (a built-in is active, or no row is left).
pub fn migrate_active_to_mod(set: &ShortcutProfileSet) -> Option<ShortcutProfileSet> {
    let active = set.active_profile();
    if active.kind != ShortcutProfileKind::Custom || standard_ctrl_rows(active) == 0 {
        return None;
    }
    let mut out = set.clone();
    *out.active_profile_mut() = with_standard_set_on_mod(active);
    Some(out)
}

/// Whether the one-time prompt applies: macOS, and the active profile is a
/// custom one that still has standard-set rows on plain `Ctrl`.
pub fn offers_mod_migration(set: &ShortcutProfileSet, platform: Platform) -> bool {
    platform == Platform::MacOs
        && set.active_profile().kind == ShortcutProfileKind::Custom
        && standard_ctrl_rows(set.active_profile()) > 0
}
