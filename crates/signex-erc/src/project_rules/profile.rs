//! Rule profiles (#1085): the shipped profile, the provenance record a
//! project keeps about the profile it was seeded from, and the diff shown
//! before "Update from profile" replaces a project's rules.
//!
//! Everything here is pure. Reading and writing `.snxrules` files is the
//! app's job; this module owns what the strings mean.

use std::collections::HashMap;

use signex_types::project::{ErcRules, RuleProfile, RuleProfileRef};

use super::{parse_pin_matrix_key, seed_from_overrides};
use crate::RuleKind;

/// Name of the profile Signex ships.
pub const BUILTIN_PROFILE_NAME: &str = "Signex Default";
/// Content version of the shipped profile; bump when its rules change.
pub const BUILTIN_PROFILE_VERSION: &str = "1.0";
/// `RuleProfileRef::source` of the shipped profile.
pub const SOURCE_BUILTIN: &str = "builtin";
/// `RuleProfileRef::source` of a profile in the user's library.
pub const SOURCE_USER: &str = "user";

/// The shipped "Signex Default" profile: every rule at its built-in severity
/// and the factory pin matrix.
pub fn builtin_profile() -> RuleProfile {
    RuleProfile::new(
        BUILTIN_PROFILE_NAME,
        BUILTIN_PROFILE_VERSION,
        seed_from_overrides(&HashMap::new(), &HashMap::new()),
    )
}

/// The provenance record for a project seeded from `profile`.
pub fn profile_ref(profile: &RuleProfile, source: &str) -> RuleProfileRef {
    RuleProfileRef {
        name: profile.name.clone(),
        source: source.to_string(),
        version: profile.version.clone(),
        sha256: profile.content_sha256(),
        unknown: Default::default(),
    }
}

/// Whether `profile` is a newer or different state than what `project_ref`
/// recorded: the project was seeded from a profile of this name whose content
/// has since changed. A project that never recorded a hash cannot drift.
pub fn has_drifted(project_ref: &RuleProfileRef, local: &RuleProfile) -> bool {
    !project_ref.sha256.is_empty() && project_ref.sha256 != local.content_sha256()
}

/// The rule set a project holds after "Update from profile": the profile's
/// rules, plus whatever the project holds that this build cannot read (a rule
/// id from a newer build the profile lacks), so an update never drops it.
pub fn adopt_profile(current: &ErcRules, profile: &ErcRules) -> ErcRules {
    let mut out = profile.clone();
    for (id, sev) in &current.severity {
        if RuleKind::from_key(id).is_none() {
            out.severity
                .entry(id.clone())
                .or_insert_with(|| sev.clone());
        }
    }
    for (id, sev) in &current.pin_matrix {
        if parse_pin_matrix_key(id).is_none() {
            out.pin_matrix
                .entry(id.clone())
                .or_insert_with(|| sev.clone());
        }
    }
    out.unknown.fill_missing_from(&current.unknown);
    out
}

/// Which part of the rule set a change touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSection {
    Severity,
    PinMatrix,
    /// A key under `erc` this build does not recognise (from a newer build).
    Other,
}

/// One entry that differs between a project's rules and a profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleChange {
    pub section: RuleSection,
    pub id: String,
    /// The project's value; `None` when the entry is absent (a pin matrix
    /// cell at the factory level).
    pub from: Option<String>,
    /// The value after the update; `None` when the entry goes away.
    pub to: Option<String>,
}

impl RuleChange {
    /// One line for the confirmation dialog, e.g.
    /// `severity unused_pin: warning -> error`.
    pub fn describe(&self) -> String {
        let section = match self.section {
            RuleSection::Severity => "severity",
            RuleSection::PinMatrix => "pin matrix",
            RuleSection::Other => "other setting",
        };
        let absent = match self.section {
            RuleSection::Other => "absent",
            _ => "factory",
        };
        let show = |v: &Option<String>| match v {
            Some(v) => v.clone(),
            None => absent.to_string(),
        };
        format!(
            "{section} {}: {} -> {}",
            self.id,
            show(&self.from),
            show(&self.to)
        )
    }
}

/// Everything that would change if `current` were replaced by [`adopt_profile`]
/// of `profile`. Empty when the update would change nothing.
pub fn diff_against_profile(current: &ErcRules, profile: &ErcRules) -> Vec<RuleChange> {
    let after = adopt_profile(current, profile);
    let mut out = Vec::new();
    diff_map(
        RuleSection::Severity,
        &current.severity,
        &after.severity,
        &mut out,
    );
    diff_map(
        RuleSection::PinMatrix,
        &current.pin_matrix,
        &after.pin_matrix,
        &mut out,
    );
    diff_unknown(&current.unknown, &after.unknown, &mut out);
    out
}

/// Foreign keys under `erc` that both sides hold with different values (the
/// profile's wins on update) or that only the profile holds. Keys only the
/// project holds are kept and are not changes.
fn diff_unknown(
    from: &signex_types::project::UnknownJson,
    to: &signex_types::project::UnknownJson,
    out: &mut Vec<RuleChange>,
) {
    for (id, after) in to.iter() {
        let before = from.get(id);
        if before != Some(after) {
            out.push(RuleChange {
                section: RuleSection::Other,
                id: id.clone(),
                from: before.map(ToString::to_string),
                to: Some(after.to_string()),
            });
        }
    }
}

fn diff_map(
    section: RuleSection,
    from: &std::collections::BTreeMap<String, String>,
    to: &std::collections::BTreeMap<String, String>,
    out: &mut Vec<RuleChange>,
) {
    let ids: std::collections::BTreeSet<_> = from.keys().chain(to.keys()).collect();
    for id in ids {
        let (a, b) = (from.get(id), to.get(id));
        if a != b {
            out.push(RuleChange {
                section,
                id: id.clone(),
                from: a.cloned(),
                to: b.cloned(),
            });
        }
    }
}

#[cfg(test)]
mod tests;
