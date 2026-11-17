//! DRC rule ids and severities on the #1085 rule layers (#2307 step 2a):
//! built-in defaults, then a profile's `drc` section, then the project's
//! `rules.drc`, a later layer overriding an earlier one. Ids follow ERC's
//! `builtin::` / `user::` convention. See `docs/architecture/pcb-drc.md`.

use std::collections::BTreeMap;

use signex_types::project::DrcRuleSet;
use signex_types::violation::Severity;

use crate::{UserRule, ViolationKind};

impl ViolationKind {
    /// Every kind, most severe first (the report order).
    pub const ALL: [ViolationKind; 7] = [
        ViolationKind::Short,
        ViolationKind::Clearance,
        ViolationKind::Unconnected,
        ViolationKind::Keepout,
        ViolationKind::TrackWidth,
        ViolationKind::ViaDiameter,
        ViolationKind::ViaDrill,
    ];

    /// Stable rule id, stored in profiles and projects; never renamed.
    pub const fn rule_id(self) -> &'static str {
        match self {
            ViolationKind::Short => "builtin::short",
            ViolationKind::Clearance => "builtin::clearance",
            ViolationKind::Unconnected => "builtin::unconnected",
            ViolationKind::Keepout => "builtin::keepout",
            ViolationKind::TrackWidth => "builtin::track_width",
            ViolationKind::ViaDiameter => "builtin::via_diameter",
            ViolationKind::ViaDrill => "builtin::via_drill",
        }
    }

    /// The kind a rule id names, or `None` for any other id.
    pub fn from_rule_id(id: &str) -> Option<ViolationKind> {
        ViolationKind::ALL.into_iter().find(|k| k.rule_id() == id)
    }

    /// Severity when no profile or project sets one.
    pub const fn default_severity(self) -> Severity {
        match self {
            ViolationKind::Unconnected => Severity::Warning,
            ViolationKind::Short
            | ViolationKind::Clearance
            | ViolationKind::Keepout
            | ViolationKind::TrackWidth
            | ViolationKind::ViaDiameter
            | ViolationKind::ViaDrill => Severity::Error,
        }
    }
}

/// Severity per rule after the layers are applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrcSeverities {
    /// A rule switched `off` has no entry.
    by_kind: BTreeMap<ViolationKind, Severity>,
    /// Written rules by id (`user::<name>`): the rule's own severity, then
    /// any layer entry for that id; `None` when switched `off`.
    by_rule: BTreeMap<String, Option<Severity>>,
    /// Entries no layer could map (an id or severity this build lacks); they
    /// stay in the file and are ignored here.
    pub unrecognised: Vec<String>,
}

impl Default for DrcSeverities {
    fn default() -> Self {
        Self::builtin()
    }
}

impl DrcSeverities {
    /// The built-in layer alone.
    pub fn builtin() -> Self {
        DrcSeverities {
            by_kind: ViolationKind::ALL
                .into_iter()
                .map(|k| (k, k.default_severity()))
                .collect(),
            by_rule: BTreeMap::new(),
            unrecognised: Vec::new(),
        }
    }

    /// The severity of `kind`; `None` when a layer switched it off.
    pub fn of(&self, kind: ViolationKind) -> Option<Severity> {
        self.by_kind.get(&kind).copied()
    }

    /// The severity of written `rule`: resolved when these severities were
    /// resolved with it, else its own `then` severity.
    pub(crate) fn of_written(&self, rule: &UserRule) -> Option<Severity> {
        match self.by_rule.get(&rule.id()) {
            Some(resolved) => *resolved,
            None => Some(rule.severity),
        }
    }

    /// Give written `rule` its own `then` severity unless these severities
    /// already hold one for it, so [`Self::of_rule`] and
    /// [`Self::of_written`] agree for a caller that never resolved them.
    pub(crate) fn seed_written(&mut self, rule: &UserRule) {
        self.by_rule.entry(rule.id()).or_insert(Some(rule.severity));
    }

    /// The severity of the rule `id`: a written rule's (`user::<name>`) or
    /// a built-in one's; `None` when switched off or unknown.
    pub fn of_rule(&self, id: &str) -> Option<Severity> {
        match ViolationKind::from_rule_id(id) {
            Some(kind) => self.of(kind),
            None => self.by_rule.get(id).copied().flatten(),
        }
    }

    fn apply(&mut self, layer: &DrcRuleSet) {
        for (id, sev) in &layer.severity {
            match (ViolationKind::from_rule_id(id), parse_severity(sev)) {
                (Some(kind), Some(Some(sev))) => {
                    self.by_kind.insert(kind, sev);
                }
                (Some(kind), Some(None)) => {
                    self.by_kind.remove(&kind);
                }
                (None, Some(sev)) if self.by_rule.contains_key(id) => {
                    self.by_rule.insert(id.clone(), sev);
                }
                _ => self.unrecognised.push(format!("severity {id} = {sev}")),
            }
        }
    }
}

/// Resolve severities: built-in defaults, then `profile`, then `project`.
pub fn resolve_severities(
    profile: Option<&DrcRuleSet>,
    project: Option<&DrcRuleSet>,
) -> DrcSeverities {
    resolve_severities_with(profile, project, &[])
}

/// [`resolve_severities`] with written rules: each starts at its own
/// severity and a layer entry for its id overrides it, as for a built-in
/// rule. An entry for a `user::` id no rule has stays unrecognised.
pub fn resolve_severities_with(
    profile: Option<&DrcRuleSet>,
    project: Option<&DrcRuleSet>,
    rules: &[UserRule],
) -> DrcSeverities {
    let mut out = DrcSeverities::builtin();
    for rule in rules {
        out.by_rule.insert(rule.id(), Some(rule.severity));
    }
    for layer in [profile, project].into_iter().flatten() {
        out.apply(layer);
    }
    out
}

/// What an edit did to written rule `name`, so that the layer entry for its
/// id (`user::<name>`), which outranks the rule's own `then` word, follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrittenRuleEdit<'a> {
    /// Its `then` severity was set: its own word governs again.
    SeveritySet(&'a str),
    /// Switched on: an `off` entry would keep it off.
    SwitchedOn(&'a str),
    Renamed {
        from: &'a str,
        to: &'a str,
    },
    /// Removed, or newly added: no entry speaks for it.
    Dropped(&'a str),
}

fn written_id(name: &str) -> String {
    format!("{}{name}", crate::USER_RULE_ID_PREFIX)
}

/// Make `layer`'s entry for the rule `edit` names follow the edit.
pub fn follow_written_rule_edit(layer: &mut DrcRuleSet, edit: WrittenRuleEdit<'_>) {
    let map = &mut layer.severity;
    match edit {
        WrittenRuleEdit::SeveritySet(name) | WrittenRuleEdit::Dropped(name) => {
            map.remove(&written_id(name));
        }
        WrittenRuleEdit::SwitchedOn(name) => {
            let id = written_id(name);
            if map.get(&id).is_some_and(|word| word == "off") {
                map.remove(&id);
            }
        }
        WrittenRuleEdit::Renamed { from, to } if from != to => {
            if let Some(word) = map.remove(&written_id(from)) {
                map.insert(written_id(to), word);
            }
        }
        WrittenRuleEdit::Renamed { .. } => {}
    }
}

/// `layer`'s entry for written rule `name`: `Some(None)` for `off`, `None`
/// with no entry or one this build cannot read.
pub fn written_rule_override(layer: &DrcRuleSet, name: &str) -> Option<Option<Severity>> {
    layer
        .severity
        .get(&written_id(name))
        .and_then(|word| parse_severity(word))
}

/// `Some(None)` for `off`, `None` for a word this build does not know.
fn parse_severity(word: &str) -> Option<Option<Severity>> {
    match word {
        "error" => Some(Some(Severity::Error)),
        "warning" => Some(Some(Severity::Warning)),
        "info" => Some(Some(Severity::Info)),
        "off" => Some(None),
        _ => None,
    }
}
