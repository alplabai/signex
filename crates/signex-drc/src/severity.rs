//! DRC rule ids and severities on the #1085 rule layers (#2307 step 2a):
//! built-in defaults, then a profile's `drc` section, then the project's
//! `rules.drc`, the most specific layer winning. Ids follow ERC's
//! `builtin::` / `user::` convention. See `docs/architecture/pcb-drc.md`.

use std::collections::BTreeMap;

use signex_types::project::DrcRuleSet;
use signex_types::violation::Severity;

use crate::ViolationKind;

impl ViolationKind {
    /// Every kind, most severe first (the report order).
    pub const ALL: [ViolationKind; 4] = [
        ViolationKind::Short,
        ViolationKind::Clearance,
        ViolationKind::Unconnected,
        ViolationKind::Keepout,
    ];

    /// Stable rule id, stored in profiles and projects; never renamed.
    pub const fn rule_id(self) -> &'static str {
        match self {
            ViolationKind::Short => "builtin::short",
            ViolationKind::Clearance => "builtin::clearance",
            ViolationKind::Unconnected => "builtin::unconnected",
            ViolationKind::Keepout => "builtin::keepout",
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
            ViolationKind::Short | ViolationKind::Clearance | ViolationKind::Keepout => {
                Severity::Error
            }
        }
    }
}

/// Severity per rule after the layers are applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrcSeverities {
    /// A rule switched `off` has no entry.
    by_kind: BTreeMap<ViolationKind, Severity>,
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
            unrecognised: Vec::new(),
        }
    }

    /// The severity of `kind`; `None` when a layer switched it off.
    pub fn of(&self, kind: ViolationKind) -> Option<Severity> {
        self.by_kind.get(&kind).copied()
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
    let mut out = DrcSeverities::builtin();
    for layer in [profile, project].into_iter().flatten() {
        out.apply(layer);
    }
    out
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
