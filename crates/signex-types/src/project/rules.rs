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

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::UnknownJson;

/// Current version of the `rules` section, independent of the `.snxprj`
/// format token.
pub const RULES_SCHEMA_V1: u32 = 1;

fn schema_v1() -> u32 {
    RULES_SCHEMA_V1
}

/// The `rules` section of a `.snxprj`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectRules {
    /// Version of this section (see [`RULES_SCHEMA_V1`]).
    #[serde(default = "schema_v1")]
    pub schema: u32,
    /// Where the rules were seeded from. Provenance only: it never changes
    /// a result by itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<RuleProfileRef>,
    /// Electrical rule check settings.
    #[serde(default)]
    pub erc: ErcRules,
    /// Keys written by a newer build (`drc`, `annotation`, ...); kept so a
    /// save does not drop them.
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

impl Default for ProjectRules {
    fn default() -> Self {
        Self {
            schema: RULES_SCHEMA_V1,
            profile: None,
            erc: ErcRules::default(),
            unknown: UnknownJson::default(),
        }
    }
}

/// Provenance of a stored rule set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleProfileRef {
    pub name: String,
    /// `"builtin"`, `"user"` or `"file"`.
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub version: String,
    /// Hash of the profile content the project was seeded from.
    #[serde(default)]
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

#[cfg(test)]
mod tests;
