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
//! [`RULES_SCHEMA_V1`], or a value whose shape changed — never stops the
//! project from opening: [`crate::project::parse_project`] keeps it verbatim
//! in the project's `unknown` keys, where it is written back untouched and
//! not applied (#1180).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::UnknownJson;

/// Current version of the `rules` section, independent of the `.snxprj`
/// format token.
pub const RULES_SCHEMA_V1: u32 = 1;

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
    /// Keys written by a newer build (`drc`, `annotation`, ...); kept so a
    /// save does not drop them.
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

impl Default for ProjectRules {
    fn default() -> Self {
        Self {
            schema: Some(RULES_SCHEMA_V1),
            profile: None,
            erc: ErcRules::default(),
            unknown: UnknownJson::default(),
        }
    }
}

impl ProjectRules {
    /// Whether this build understands the section's version.
    pub fn is_supported_schema(&self) -> bool {
        self.schema.unwrap_or(RULES_SCHEMA_V1) <= RULES_SCHEMA_V1
    }
}

/// Provenance of a stored rule set. Every field is optional on load and
/// omitted on save when empty, so a partial entry round-trips as written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
}

#[cfg(test)]
mod tests;
