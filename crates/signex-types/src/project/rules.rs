//! The project-scoped rule set stored in a `.snxprj` (#1085).
//!
//! Rules are part of the design: a project carries the full resolved rule
//! set, so ERC gives the same result on every machine instead of depending on
//! the machine-wide `prefs.json`. The section is optional. A file without it
//! loads as `None` and re-saves without a `rules` key; the section is only
//! added once the rules are stored.
//!
//! Rule ids and severities are stable strings, never enum indices, and live
//! in `BTreeMap`s so a rule change is a small, readable diff. This crate does
//! not know the ERC rule vocabulary; `signex-erc` resolves the strings. Keys a
//! newer build wrote are kept verbatim in `unknown`, as everywhere else in the
//! `.snxprj`.
//!
//! A section this build cannot interpret — a `schema` newer than
//! [`RULES_SCHEMA_V2`], or a value whose shape changed — never stops the
//! project from opening: [`crate::project::parse_project`] keeps it verbatim
//! in the project's `unknown` keys, where it is written back untouched and
//! not applied (#1180).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::UnknownJson;

/// Current version of the `rules` section, independent of the `.snxprj`
/// format token.
pub const RULES_SCHEMA_V1: u32 = 1;

/// The schema of a section that names a min slot (mockup row 179d). An
/// older build reads only 1, so it keeps a 2 section verbatim, unapplied,
/// instead of reading the rules and losing the value. Content-driven:
/// [`ProjectRules::written_schema`].
pub const RULES_SCHEMA_V2: u32 = 2;

/// The `rules` section of a `.snxprj`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectRules {
    /// Version of this section (see [`RULES_SCHEMA_V1`]). `None` when the
    /// file names none, read as version 1; kept absent on save so a section
    /// without it re-saves byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<u32>,
    /// Where the rules were seeded from. Provenance only: it never changes
    /// a result by itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<RuleProfileRef>,
    /// Electrical rule check settings.
    #[serde(default, skip_serializing_if = "ErcRules::is_empty")]
    pub erc: ErcRules,
    /// Net-level ERC findings the designer accepted: rule id -> names of the
    /// nets waived for that rule. Keyed by net *name*, not by pin, so moving
    /// pins keeps a waiver; renaming a net ends it (the old name no longer
    /// matches any net, and the entry stays listed until restored). Kept
    /// beside `erc`, not inside it, so waiving a finding never changes the
    /// rule set's hash or its profile provenance.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub erc_net_waivers: BTreeMap<String, BTreeSet<String>>,
    /// Per-finding ERC waivers with a reason (#1285): rule id + the involved
    /// objects' UUIDs + a content fingerprint, so a waiver lapses when one of
    /// those objects changes. Beside `erc` for the same reason as
    /// `erc_net_waivers`. Older builds keep the key verbatim in `unknown`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub erc_waivers: Vec<super::ErcWaiver>,
    /// Design rule check settings (#2307). An empty section is not written,
    /// so a file without one re-saves without one.
    #[serde(default, skip_serializing_if = "DrcRuleSet::is_empty")]
    pub drc: DrcRuleSet,
    /// The narrowest slot the project's fab tier or profile routes, across,
    /// in mm (mockup row 179d); the board sketch warns at a narrower slot.
    /// `None` when none was applied, skipped on save; a value that is not
    /// a positive number reads as `None`. Not part of the rule-set hash.
    /// Read and set it through [`Self::min_slot`] and [`Self::set_min_slot`].
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::min_slot::lenient_min_slot"
    )]
    pub min_slot: Option<f64>,
    /// Keys written by a newer build (`annotation`, ...); kept so a save
    /// does not drop them.
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

impl Default for ProjectRules {
    fn default() -> Self {
        Self {
            schema: Some(RULES_SCHEMA_V1),
            profile: None,
            erc: ErcRules::default(),
            erc_net_waivers: BTreeMap::new(),
            erc_waivers: Vec::new(),
            drc: DrcRuleSet::default(),
            min_slot: None,
            unknown: UnknownJson::default(),
        }
    }
}

impl ProjectRules {
    /// Whether this build understands the section's version.
    pub fn is_supported_schema(&self) -> bool {
        self.schema.unwrap_or(RULES_SCHEMA_V1) <= RULES_SCHEMA_V2
    }

    /// The min slot in mm, absent when none was applied or the stored value
    /// is not a positive number.
    pub fn min_slot(&self) -> Option<f64> {
        self.min_slot.filter(|mm| super::min_slot::is_usable(*mm))
    }

    /// Store the min slot; `None`, or a value that is not a positive
    /// number, clears it. The schema follows, so the section equals what a
    /// save reads back.
    pub fn set_min_slot(&mut self, mm: Option<f64>) {
        self.min_slot = mm.filter(|mm| super::min_slot::is_usable(*mm));
        self.schema = self.written_schema();
    }

    /// The schema the section is written at: 2 while it names a min slot,
    /// else as read (a 2 with nothing that needs it goes back to 1), so a
    /// section without the value re-saves as today.
    pub fn written_schema(&self) -> Option<u32> {
        if self.min_slot().is_some() {
            return Some(RULES_SCHEMA_V2);
        }
        match self.schema {
            Some(RULES_SCHEMA_V2) => Some(RULES_SCHEMA_V1),
            other => other,
        }
    }

    /// The section as the `.snxprj` writes it, at [`Self::written_schema`].
    pub fn to_written_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        let written = self.written_schema();
        if written == self.schema {
            return serde_json::to_value(self);
        }
        let mut copy = self.clone();
        copy.schema = written;
        serde_json::to_value(&copy)
    }
}

/// Provenance of a stored rule set. Every field is optional on load and
/// omitted on save when empty, so a partial entry round-trips as written.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RuleProfileRef {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// `"builtin"`, `"user"` or `"file"`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    /// Hash of the profile content the project was seeded from.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sha256: String,
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

/// ERC settings of a project.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ErcRules {
    /// Rule id -> severity (`error` / `warning` / `info` / `off`). A snapshot
    /// of every rule id known when it was written.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub severity: BTreeMap<String, String>,
    /// `"<row axis>/<column axis>"` -> severity, for pin connection matrix
    /// cells that differ from the factory matrix.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pin_matrix: BTreeMap<String, String>,
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

impl ErcRules {
    /// No severities, no pin-matrix cells and no foreign keys.
    pub fn is_empty(&self) -> bool {
        self.severity.is_empty() && self.pin_matrix.is_empty() && self.unknown.is_empty()
    }

    /// Lower-case hex SHA-256 of the rule set's canonical JSON. The maps are
    /// ordered, so equal rule sets hash equal on every machine; the profile
    /// provenance and the ERC report header both record this value.
    pub fn sha256_hex(&self) -> String {
        canonical_sha256(self)
    }
}

/// DRC settings of a project or a profile (#2307). Like [`ErcRules`], this
/// crate stores strings; `signex-drc` owns what they mean.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DrcRuleSet {
    /// Rule id (`builtin::clearance`, ...) -> severity (`error` / `warning` /
    /// `info` / `off`), for rules set away from the built-in default.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub severity: BTreeMap<String, String>,
    /// Text in the DRC rule language (#2307 step 2b), compiled by
    /// `signex-drc-dsl` for each run. Stored and written back as is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dsl: Option<String>,
    /// Named rule sets (#2398), by name: limits written once and applied to
    /// net classes or nets as rules in `dsl`. `signex-drc-dsl` keeps those
    /// rules in step when a set changes.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sets: BTreeMap<String, NamedRuleSet>,
    /// The names of the rules in `dsl` that Update PCB wrote from the
    /// schematic's directives (#1489), so the Rules tab shows their source.
    /// Not written while empty; a build without the key keeps it in
    /// `unknown`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub from_schematic: Vec<String>,
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

/// One named rule set (#2398): its limits as written (`min_clearance` ->
/// `0.8mm`), the severity its rules take, and where it is applied.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NamedRuleSet {
    /// Limit field id (`min_clearance`, `min_width`, ...) -> length as
    /// written.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub limits: BTreeMap<String, String>,
    /// `error` / `warning` / `info`; absent is `error`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    /// Each scope the set is applied to and the rules written for it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uses: Vec<RuleSetUse>,
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

/// Where a named rule set is applied: a scope as the rule text writes it
/// (`class == "HV"`), and the names of the rules written for it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RuleSetUse {
    pub scope: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<String>,
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

impl DrcRuleSet {
    /// No severities, no rule text, no rule sets, no schematic sources and
    /// no foreign keys.
    pub fn is_empty(&self) -> bool {
        self.severity.is_empty()
            && self.dsl.is_none()
            && self.sets.is_empty()
            && self.from_schematic.is_empty()
            && self.unknown.is_empty()
    }

    /// Whether Update PCB wrote rule `name` from the schematic.
    pub fn is_from_schematic(&self, name: &str) -> bool {
        self.from_schematic.iter().any(|n| n == name)
    }

    /// Lower-case hex SHA-256 of the section's canonical JSON alone. Profile
    /// provenance records [`rule_set_sha256`], which covers this section.
    pub fn sha256_hex(&self) -> String {
        canonical_sha256(self)
    }
}

/// The rule-set hash profile provenance records (#2307 step 2b): the ERC hash
/// alone while `drc` is empty, so every hash recorded before DRC rules
/// existed stays valid; otherwise the hash of `{"erc": .., "drc": ..}`.
/// See `docs/architecture/snxrules-format.md`, "What the hash covers".
pub fn rule_set_sha256(erc: &ErcRules, drc: &DrcRuleSet) -> String {
    #[derive(Serialize)]
    struct RuleSet<'a> {
        erc: &'a ErcRules,
        drc: &'a DrcRuleSet,
    }
    if drc.is_empty() {
        erc.sha256_hex()
    } else {
        canonical_sha256(&RuleSet { erc, drc })
    }
}

fn canonical_sha256(value: &impl Serialize) -> String {
    use sha2::{Digest, Sha256};
    // A map of strings and JSON values always serialises.
    let canonical = serde_json::to_vec(value).unwrap_or_default();
    format!("{:x}", Sha256::digest(canonical))
}

#[cfg(test)]
mod tests;
