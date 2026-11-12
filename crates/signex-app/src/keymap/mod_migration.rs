//! Moving a custom profile's standard-set `Ctrl+<k>` rows to `Mod+<k>` for
//! the one-time macOS prompt (#1645). Pure: no IO, no prompt state.
//!
//! Only the platform-standard set moves. Chords whose Command twin is a
//! macOS system shortcut (`Ctrl+Q`, `Ctrl+W`, `Ctrl+H`, ...), chords with
//! more than one stroke, pointer gestures and platform-tagged rows stay as
//! they are. A row whose `Mod` twin is already bound in the same context is
//! skipped and reported rather than doubled. Redo is the built-ins' shape:
//! `Ctrl+Shift+Z` stays, and a `platform = "macos"` `Mod+Shift+Z` row for the
//! same command is added next to it. A second run finds nothing to do.

use super::{
    KeyStroke, KeyToken, Modifiers, Platform, ShortcutBinding, ShortcutBindingAction,
    ShortcutContext, ShortcutProfile, ShortcutProfileKind, ShortcutProfileSet, ShortcutTrigger,
};

/// The approved standard set: `(lower-case key, shift)` chords that are
/// `Mod` in the built-in profiles. `Shift+Z` (redo) is not here: it keeps its
/// `Ctrl` row and gains a macOS-only `Mod` row instead.
const STANDARD_SET: [(&str, bool); 24] = [
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
    ("p", true),
    ("v", true),
    ("g", true),
    ("x", true),
    ("e", true),
    ("f", true),
    ("a", true),
];

/// What moving one profile does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModMove {
    pub profile: ShortcutProfile,
    /// Rows respelled plus rows added; zero means there is nothing to do.
    pub moved: usize,
    /// `Ctrl` chords left alone because their `Mod` twin is already bound.
    pub skipped: Vec<String>,
}

/// The set with its active profile moved, and what was skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModMigration {
    pub set: ShortcutProfileSet,
    pub skipped: Vec<String>,
}

/// A stroke of `Ctrl` (physical Control) plus, optionally, Shift.
fn plain_ctrl(stroke: &KeyStroke) -> bool {
    let m = stroke.modifiers;
    m.ctrl && !m.primary && !m.alt && !m.command
}

/// `Ctrl+<k>` or `Ctrl+Shift+<k>` of the standard set.
fn is_standard_ctrl(stroke: &KeyStroke) -> bool {
    plain_ctrl(stroke)
        && matches!(&stroke.key, KeyToken::Character(key)
            if STANDARD_SET.contains(&(key.as_str(), stroke.modifiers.shift)))
}

fn is_ctrl_shift_z(stroke: &KeyStroke) -> bool {
    plain_ctrl(stroke)
        && stroke.modifiers.shift
        && matches!(&stroke.key, KeyToken::Character(key) if key == "z")
}

fn single_stroke(trigger: &ShortcutTrigger) -> Option<&KeyStroke> {
    match trigger {
        ShortcutTrigger::KeySequence(strokes) => match strokes.as_slice() {
            [only] => Some(only),
            _ => None,
        },
        ShortcutTrigger::PointerGesture(_) => None,
    }
}

fn mod_stroke(stroke: &KeyStroke) -> KeyStroke {
    KeyStroke {
        modifiers: Modifiers {
            ctrl: false,
            primary: true,
            ..stroke.modifiers
        },
        key: stroke.key.clone(),
    }
}

/// A binding that already puts the Command twin of `stroke` (`Mod` or `Cmd`,
/// same key and shift) in `context`, on macOS: untagged or tagged macOS.
fn command_twin<'a>(
    profile: &'a ShortcutProfile,
    context: ShortcutContext,
    stroke: &KeyStroke,
) -> Option<&'a ShortcutBinding> {
    profile.bindings.iter().find(|binding| {
        binding.context == context
            && matches!(binding.platform, None | Some(Platform::MacOs))
            && binding
                .triggers
                .iter()
                .filter_map(single_stroke)
                .any(|other| {
                    let m = other.modifiers;
                    (m.primary || m.command)
                        && !m.ctrl
                        && !m.alt
                        && m.shift == stroke.modifiers.shift
                        && other.key == stroke.key
                })
    })
}

/// `profile` moved to `Mod`: every untagged standard-set `Ctrl` row (command
/// and unbind rows alike, so an unbind keeps matching its binding) respelled,
/// and the redo row added.
pub fn with_standard_set_on_mod(profile: &ShortcutProfile) -> ModMove {
    let mut out = profile.clone();
    let (mut moved, mut skipped) = (0, Vec::new());
    for binding in out.bindings.iter_mut().filter(|b| b.platform.is_none()) {
        for trigger in binding.triggers.iter_mut() {
            let Some(stroke) = single_stroke(trigger).filter(|s| is_standard_ctrl(s)) else {
                continue;
            };
            if command_twin(profile, binding.context, stroke).is_some() {
                skipped.push(trigger.config_text());
                continue;
            }
            *trigger = ShortcutTrigger::KeySequence(vec![mod_stroke(stroke)]);
            moved += 1;
        }
    }
    let mut additions = Vec::new();
    for (index, binding) in profile.bindings.iter().enumerate() {
        let ShortcutBindingAction::Command(_) = &binding.action else {
            continue;
        };
        if binding.platform.is_some() {
            continue;
        }
        let Some(stroke) = binding
            .triggers
            .iter()
            .filter_map(single_stroke)
            .find(|s| is_ctrl_shift_z(s))
        else {
            continue;
        };
        match command_twin(profile, binding.context, stroke) {
            Some(twin) if twin.action == binding.action => {}
            Some(_) => skipped.push("Ctrl+Shift+Z".to_string()),
            None => additions.push((
                index + 1,
                ShortcutBinding {
                    action: binding.action.clone(),
                    context: binding.context,
                    triggers: vec![ShortcutTrigger::KeySequence(vec![mod_stroke(stroke)])],
                    platform: Some(Platform::MacOs),
                },
            )),
        }
    }
    moved += additions.len();
    for (at, row) in additions.into_iter().rev() {
        out.bindings.insert(at, row);
    }
    ModMove {
        profile: out,
        moved,
        skipped,
    }
}

/// How many rows moving `profile` would change.
pub fn standard_ctrl_rows(profile: &ShortcutProfile) -> usize {
    with_standard_set_on_mod(profile).moved
}

/// The set with its active custom profile moved to `Mod`, or `None` when
/// there is nothing to move (a built-in is active, or no row is left).
pub fn migrate_active_to_mod(set: &ShortcutProfileSet) -> Option<ModMigration> {
    let active = set.active_profile();
    if active.kind != ShortcutProfileKind::Custom {
        return None;
    }
    let ModMove {
        profile,
        moved,
        skipped,
    } = with_standard_set_on_mod(active);
    if moved == 0 {
        return None;
    }
    let mut out = set.clone();
    *out.active_profile_mut() = profile;
    Some(ModMigration { set: out, skipped })
}

/// Whether the one-time prompt applies: macOS, and the active profile is a
/// custom one that still has rows to move.
pub fn offers_mod_migration(set: &ShortcutProfileSet, platform: Platform) -> bool {
    platform == Platform::MacOs
        && set.active_profile().kind == ShortcutProfileKind::Custom
        && standard_ctrl_rows(set.active_profile()) > 0
}
