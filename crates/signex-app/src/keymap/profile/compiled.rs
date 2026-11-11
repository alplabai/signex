//! A profile compiled for one OS: `Mod` resolved, other-OS bindings left
//! out, ready for lookup, labels and conflict checks.

use super::{ShortcutProfile, ShortcutProfileKind};
use crate::keymap::{
    AppCommandId, KeyBindingSource, KeyStroke, Platform, ShortcutBindingAction, ShortcutContext,
    ShortcutTrigger,
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompiledKeymap {
    /// The platform every `Mod` was resolved for and every label is shown
    /// for; lookups compare resolved strokes only.
    platform: Platform,
    bindings: Vec<CompiledBinding>,
    /// Per-command menu label, derived from `bindings` at compile time so
    /// `shortcut_label` is a map hit instead of a reverse scan plus a
    /// `String` join per menu row per frame (#1380).
    labels: BTreeMap<AppCommandId, String>,
}

/// Per command, the label of its last `Command` binding that no later
/// `Unbind` of the same command, context and strokes shadows. A command
/// with no such binding has no entry.
///
/// The hint is context-free: it does not know which context the menu is
/// shown in, and an `Unbind` shadows a binding only in its own context
/// (the same rule `shortcut_label_linear` states independently).
fn label_index(bindings: &[CompiledBinding], platform: Platform) -> BTreeMap<AppCommandId, String> {
    let mut labels = BTreeMap::new();
    // Reverse scan; `unbinds` holds the unbinds that come after the
    // binding under inspection.
    let mut unbinds: BTreeSet<(&AppCommandId, ShortcutContext, &[KeyStroke])> = BTreeSet::new();
    for binding in bindings.iter().rev() {
        match &binding.action {
            ShortcutBindingAction::Unbind(command) => {
                unbinds.insert((command, binding.context, &binding.sequence));
            }
            ShortcutBindingAction::Command(command) => {
                let shadowed =
                    unbinds.contains(&(command, binding.context, binding.sequence.as_slice()));
                if !shadowed && !labels.contains_key(command) {
                    labels.insert(
                        command.clone(),
                        display_sequence(&binding.sequence, platform),
                    );
                }
            }
            ShortcutBindingAction::NoAction => {}
        }
    }
    labels
}

fn display_sequence(sequence: &[KeyStroke], platform: Platform) -> String {
    sequence
        .iter()
        .map(|stroke| stroke.display_for(platform))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Strokes macOS keeps for itself: Quit, Hide, Minimize, Spotlight, the app
/// switcher and window cycling. Settings (`⌘,`) is allowed for Preferences.
fn is_macos_reserved(stroke: &KeyStroke, command: &AppCommandId) -> bool {
    use crate::keymap::KeyToken;
    let m = stroke.modifiers;
    if !m.command || m.ctrl || m.alt || m.shift {
        return false;
    }
    match &stroke.key {
        KeyToken::Space | KeyToken::Tab => true,
        KeyToken::Character(c) => match c.as_str() {
            "q" | "h" | "m" | "`" => true,
            "," => command.as_str() != "open_preferences",
            _ => false,
        },
        _ => false,
    }
}

impl CompiledKeymap {
    pub fn compile(profile: &ShortcutProfile) -> Self {
        Self::compile_for(profile, Platform::current())
    }

    /// Compile with every `Mod` resolved for `platform`.
    pub fn compile_for(profile: &ShortcutProfile, platform: Platform) -> Self {
        let source = match profile.kind {
            ShortcutProfileKind::BuiltIn => KeyBindingSource::BuiltIn(profile.id.clone()),
            ShortcutProfileKind::Custom => KeyBindingSource::Custom(profile.id.clone()),
        };
        let mut bindings = Vec::new();
        for binding in profile
            .bindings
            .iter()
            .filter(|binding| binding.applies_on(platform))
        {
            for trigger in &binding.triggers {
                if let ShortcutTrigger::KeySequence(sequence) = trigger {
                    bindings.push(CompiledBinding {
                        action: binding.action.clone(),
                        context: binding.context,
                        sequence: sequence
                            .iter()
                            .map(|stroke| stroke.resolved_for(platform))
                            .collect(),
                        source: source.clone(),
                    });
                }
            }
        }
        let labels = label_index(&bindings, platform);
        Self {
            platform,
            bindings,
            labels,
        }
    }

    pub fn lookup(&self, input: &[KeyStroke], contexts: &[ShortcutContext]) -> KeyLookup {
        let mut matches = self
            .bindings
            .iter()
            .enumerate()
            .filter_map(|(index, binding)| {
                let pending = binding.matches_input(input)?;
                let depth = context_depth(binding.context, contexts)?;
                Some((pending, depth, index, binding))
            })
            .collect::<Vec<_>>();

        // #1378: only a live command can make a shorter chord wait; an
        // `Unbind` / `NoAction` that starts with the same strokes must not.
        let pending = matches.iter().any(|(pending, _, _, binding)| {
            *pending && matches!(binding.action, ShortcutBindingAction::Command(_))
        });
        matches.retain(|(pending, ..)| !*pending);
        let matched = !matches.is_empty();
        matches.sort_by(|(_, depth_a, index_a, _), (_, depth_b, index_b, _)| {
            depth_b.cmp(depth_a).then(index_b.cmp(index_a))
        });

        let command = resolve_matched_command(matches);
        KeyLookup {
            command,
            pending,
            matched,
        }
    }

    pub fn shortcut_label(&self, command: &AppCommandId) -> Option<String> {
        self.labels.get(command).cloned()
    }

    /// The label for a command given by its string id. An unparsable,
    /// unbound or unbound-by-the-user id has no hint, so the label is empty
    /// (#1378).
    pub fn shortcut_label_for_id(&self, command_id: &str) -> String {
        AppCommandId::new(command_id)
            .ok()
            .and_then(|command| self.shortcut_label(&command))
            .unwrap_or_default()
    }

    /// The label for `command_id`, but `preferred` when the profile binds the
    /// command to exactly that sequence (and it is not unbound later); else
    /// the usual label. The chrome search field uses this to show `Ctrl+K`
    /// on presets that bind it and fall back to the bound chord otherwise.
    pub fn shortcut_label_preferring(&self, command_id: &str, preferred: &str) -> String {
        let Ok(command) = AppCommandId::new(command_id) else {
            return String::new();
        };
        let Ok(ShortcutTrigger::KeySequence(wanted)) = ShortcutTrigger::parse(preferred) else {
            return self.shortcut_label(&command).unwrap_or_default();
        };
        let wanted: Vec<KeyStroke> = wanted
            .iter()
            .map(|stroke| stroke.resolved_for(self.platform))
            .collect();
        let bound = self.bindings.iter().enumerate().any(|(index, binding)| {
            let ShortcutBindingAction::Command(c) = &binding.action else {
                return false;
            };
            c == &command
                && binding.sequence == wanted
                && !self.bindings[index + 1..].iter().any(|later| {
                    matches!(&later.action, ShortcutBindingAction::Unbind(u) if u == &command)
                        && later.context == binding.context
                        && later.sequence == binding.sequence
                })
        });
        if bound {
            display_sequence(&wanted, self.platform)
        } else {
            self.shortcut_label(&command).unwrap_or_default()
        }
    }

    /// Every compiled binding as `(context, resolved stroke text, action)`,
    /// in compile order: the table a lookup actually compares against.
    #[cfg(test)]
    pub(crate) fn resolved_table(&self) -> Vec<(ShortcutContext, String, String)> {
        self.bindings
            .iter()
            .map(|binding| {
                let strokes = binding
                    .sequence
                    .iter()
                    .map(KeyStroke::to_config_string)
                    .collect::<Vec<_>>()
                    .join(" ");
                (binding.context, strokes, format!("{:?}", binding.action))
            })
            .collect()
    }

    /// A quadratic restatement of the label rule, kept so a test can prove
    /// the index agrees with it.
    #[cfg(test)]
    pub(crate) fn shortcut_label_linear(&self, command: &AppCommandId) -> Option<String> {
        self.bindings
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, binding)| {
                let ShortcutBindingAction::Command(bound) = &binding.action else {
                    return None;
                };
                let shadowed = self.bindings[index + 1..].iter().any(|later| {
                    matches!(&later.action, ShortcutBindingAction::Unbind(c) if c == bound)
                        && later.context == binding.context
                        && later.sequence == binding.sequence
                });
                (bound == command && !shadowed)
                    .then(|| display_sequence(&binding.sequence, self.platform))
            })
    }

    pub fn conflicts(&self) -> Vec<BindingConflict> {
        let mut seen: BTreeMap<(ShortcutContext, Vec<KeyStroke>), &CompiledBinding> =
            BTreeMap::new();
        let mut conflicts = Vec::new();
        for binding in self
            .bindings
            .iter()
            .filter(|binding| matches!(binding.action, ShortcutBindingAction::Command(_)))
        {
            let key = (binding.context, binding.sequence.clone());
            if let Some(existing) = seen.insert(key, binding)
                && existing.action != binding.action
            {
                let Some(first_command) = existing.action.command().cloned() else {
                    continue;
                };
                let Some(second_command) = binding.action.command().cloned() else {
                    continue;
                };
                conflicts.push(BindingConflict {
                    context: binding.context,
                    trigger: display_sequence(&binding.sequence, self.platform),
                    sequence: binding.sequence.clone(),
                    first_command,
                    second_command,
                    system_reserved: false,
                });
            }
        }
        conflicts.extend(self.reserved_conflicts());
        conflicts
    }

    /// On macOS, each binding whose first stroke the system owns (⌘Q, ⌘H,
    /// ⌘M, ⌘Space, ⌘Tab, ⌘`, and ⌘, outside Preferences). The command
    /// is named on both sides of the conflict.
    fn reserved_conflicts(&self) -> Vec<BindingConflict> {
        if self.platform != Platform::MacOs {
            return Vec::new();
        }
        self.bindings
            .iter()
            .filter_map(|binding| {
                let ShortcutBindingAction::Command(command) = &binding.action else {
                    return None;
                };
                let first = binding.sequence.first()?;
                is_macos_reserved(first, command).then(|| BindingConflict {
                    context: binding.context,
                    trigger: display_sequence(&binding.sequence, self.platform),
                    sequence: binding.sequence.clone(),
                    first_command: command.clone(),
                    second_command: command.clone(),
                    system_reserved: true,
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CompiledBinding {
    action: ShortcutBindingAction,
    context: ShortcutContext,
    sequence: Vec<KeyStroke>,
    source: KeyBindingSource,
}

impl CompiledBinding {
    fn matches_input(&self, input: &[KeyStroke]) -> Option<bool> {
        if input.len() > self.sequence.len() {
            return None;
        }
        self.sequence
            .iter()
            .zip(input)
            .all(|(expected, actual)| expected == actual)
            .then_some(input.len() < self.sequence.len())
    }
}

fn resolve_matched_command(
    matches: Vec<(bool, usize, usize, &CompiledBinding)>,
) -> Option<AppCommandId> {
    let mut unbound = Vec::new();
    for (_, _, _, binding) in matches {
        match &binding.action {
            ShortcutBindingAction::Command(command) => {
                if !unbound.iter().any(|blocked| blocked == command) {
                    return Some(command.clone());
                }
            }
            ShortcutBindingAction::Unbind(command) => unbound.push(command.clone()),
            ShortcutBindingAction::NoAction => return None,
        }
    }
    None
}

fn context_depth(context: ShortcutContext, contexts: &[ShortcutContext]) -> Option<usize> {
    if context == ShortcutContext::Global {
        return Some(0);
    }
    contexts
        .iter()
        .position(|candidate| *candidate == context)
        .map(|position| position + 1)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyLookup {
    pub command: Option<AppCommandId>,
    pub pending: bool,
    pub matched: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingConflict {
    pub context: ShortcutContext,
    pub trigger: String,
    /// The strokes the conflict is on, resolved for the platform: what a
    /// row compares against, so `Mod+K` and `Ctrl+K` still match.
    pub sequence: Vec<KeyStroke>,
    pub first_command: AppCommandId,
    pub second_command: AppCommandId,
    /// The trigger belongs to the operating system (macOS ⌘Q, ⌘H, …), not to
    /// another command; both command fields name the one command.
    pub system_reserved: bool,
}
