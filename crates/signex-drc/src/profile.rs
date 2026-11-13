//! The DRC half of "Update from profile" (#2307 step 2b): what a project's
//! `rules.drc` becomes when it adopts a profile's `drc`, and the lines the
//! confirmation lists. The ERC half lives in `signex-erc`; the app shows
//! both. See `docs/architecture/pcb-drc.md`, "Rule profiles".

use std::collections::{BTreeMap, BTreeSet};

use signex_types::project::{DrcRuleSet, UnknownJson};

use crate::ViolationKind;

/// The `rules.drc` a project holds after "Update from profile": the
/// profile's section, plus what the project holds that this build cannot
/// read (a severity id from a newer build, a foreign key), so an update
/// never drops it. A `user::` severity follows the project's own rule text,
/// which the profile's replaces, so it is not kept.
pub fn adopt_profile_drc(current: &DrcRuleSet, profile: &DrcRuleSet) -> DrcRuleSet {
    let mut out = profile.clone();
    for (id, sev) in &current.severity {
        if is_foreign_id(id) {
            out.severity
                .entry(id.clone())
                .or_insert_with(|| sev.clone());
        }
    }
    out.unknown.fill_missing_from(&current.unknown);
    out
}

fn is_foreign_id(id: &str) -> bool {
    ViolationKind::from_rule_id(id).is_none() && !id.starts_with(crate::USER_RULE_ID_PREFIX)
}

/// Which part of the `drc` section a change touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrcRuleSection {
    Severity,
    /// The rule-language text, compared as a whole.
    RuleText,
    /// A key under `drc` this build does not recognise.
    Other,
}

/// One entry that differs between a project's `rules.drc` and a profile's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrcRuleChange {
    pub section: DrcRuleSection,
    /// The rule id or foreign key; empty for the rule text.
    pub id: String,
    /// The project's value; `None` when absent.
    pub from: Option<String>,
    /// The value after the update; `None` when it goes away.
    pub to: Option<String>,
}

impl DrcRuleChange {
    /// One line for the confirmation dialog, e.g.
    /// `DRC severity builtin::unconnected: default -> error` or
    /// `DRC rule text: changed (3 lines -> 5 lines)`.
    pub fn describe(&self) -> String {
        match self.section {
            DrcRuleSection::Severity => {
                // An absent entry leaves a written rule at its own `then`
                // severity and a built-in one at its default.
                let absent = if self.id.starts_with(crate::USER_RULE_ID_PREFIX) {
                    "the rule's own"
                } else {
                    "default"
                };
                format!(
                    "DRC severity {}: {} -> {}",
                    self.id,
                    self.from.as_deref().unwrap_or(absent),
                    self.to.as_deref().unwrap_or(absent)
                )
            }
            DrcRuleSection::RuleText => {
                let size = |text: &str| match text.lines().count() {
                    1 => "1 line".to_string(),
                    n => format!("{n} lines"),
                };
                match (self.from.as_deref(), self.to.as_deref()) {
                    (Some(a), Some(b)) => {
                        format!("DRC rule text: changed ({} -> {})", size(a), size(b))
                    }
                    (None, Some(b)) => format!("DRC rule text: added ({})", size(b)),
                    (Some(a), None) => format!("DRC rule text: removed ({})", size(a)),
                    (None, None) => "DRC rule text: unchanged".to_string(),
                }
            }
            DrcRuleSection::Other => format!(
                "DRC other setting {}: {} -> {}",
                self.id,
                self.from.as_deref().unwrap_or("absent"),
                self.to.as_deref().unwrap_or("absent")
            ),
        }
    }
}

/// Everything that would change if `current` were replaced by
/// [`adopt_profile_drc`] of `profile`. Empty when nothing would change.
pub fn diff_drc_against_profile(current: &DrcRuleSet, profile: &DrcRuleSet) -> Vec<DrcRuleChange> {
    let after = adopt_profile_drc(current, profile);
    let mut out = diff_severity(&current.severity, &after.severity);
    if current.dsl != after.dsl {
        out.push(DrcRuleChange {
            section: DrcRuleSection::RuleText,
            id: String::new(),
            from: current.dsl.clone(),
            to: after.dsl.clone(),
        });
    }
    out.extend(diff_unknown(&current.unknown, &after.unknown));
    out
}

fn diff_severity(
    from: &BTreeMap<String, String>,
    to: &BTreeMap<String, String>,
) -> Vec<DrcRuleChange> {
    let ids: BTreeSet<_> = from.keys().chain(to.keys()).collect();
    ids.into_iter()
        .filter(|id| from.get(*id) != to.get(*id))
        .map(|id| DrcRuleChange {
            section: DrcRuleSection::Severity,
            id: id.clone(),
            from: from.get(id).cloned(),
            to: to.get(id).cloned(),
        })
        .collect()
}

/// Foreign keys the profile sets to another value; keys only the project
/// holds are kept by the update and are not changes.
fn diff_unknown(from: &UnknownJson, to: &UnknownJson) -> Vec<DrcRuleChange> {
    to.iter()
        .filter(|(id, after)| from.get(id) != Some(*after))
        .map(|(id, after)| DrcRuleChange {
            section: DrcRuleSection::Other,
            id: id.clone(),
            from: from.get(id).map(ToString::to_string),
            to: Some(after.to_string()),
        })
        .collect()
}
