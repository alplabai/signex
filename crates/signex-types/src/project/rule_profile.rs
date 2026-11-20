//! Portable rule profiles: the `.snxrules` file (#1085).
//!
//! A profile is a named rule set that lives outside any project, in the
//! user's profile library or as a shared file, and seeds the `rules` section
//! of a `.snxprj`. The file is JSON with the same ordered, string-keyed shape
//! as the project section, so a profile and a project compare by content:
//!
//! ```json
//! { "schema": 1, "name": "Signex Default", "version": "1.0",
//!   "erc": { "severity": { ... }, "pin_matrix": { ... } } }
//! ```
//!
//! An optional `drc` section (#2307) has the shape of the project's
//! `rules.drc`. Keys this build does not know (a section from a newer build,
//! an unknown rule id) are kept verbatim and written back, as everywhere
//! else. A `schema` newer than [`RULE_PROFILE_SCHEMA_V2`] is refused rather
//! than half-read, because applying part of a rule set silently changes
//! results. The schema is content-driven ([`RuleProfile::written_schema`],
//! `docs/architecture/snxrules-format.md`).

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{DrcRuleSet, ErcRules, UnknownJson};

/// Current version of the profile file, independent of the project's
/// `rules.schema`.
pub const RULE_PROFILE_SCHEMA_V1: u32 = 1;

/// The schema of a profile whose `fab` section names a router tool
/// diameter (#179d). `fab` keeps no unknown keys, so an older build would
/// drop the value on re-save; it refuses this schema by name instead.
pub const RULE_PROFILE_SCHEMA_V2: u32 = 2;

/// File extension of a profile, without the dot.
pub const RULE_PROFILE_EXTENSION: &str = "snxrules";

/// A named, portable rule set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleProfile {
    /// Version of the file format. `None` reads as version 1 and stays
    /// absent on save, so a file without it re-saves as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<u32>,
    #[serde(default)]
    pub name: String,
    /// Version of the profile's content, chosen by its author.
    #[serde(default)]
    pub version: String,
    #[serde(default, skip_serializing_if = "ErcRules::is_empty")]
    pub erc: ErcRules,
    /// DRC settings (#2307); not written when empty.
    #[serde(default, skip_serializing_if = "DrcRuleSet::is_empty")]
    pub drc: DrcRuleSet,
    /// For a fab profile (#2311): whose limits its `drc` rules copy and
    /// from where; not written when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fab: Option<FabProfileInfo>,
    /// Keys written by a newer build; kept so a save does not drop them.
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

/// What a fab profile's limits are (#2311), so they can be audited and
/// updated: the maker, its process or service tier, and the published
/// source and date the numbers were copied from. Every field is free text;
/// an empty one is not known.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FabProfileInfo {
    /// Empty for a generic tier that names no maker.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub manufacturer: String,
    /// The process or service tier: layer count, copper weight, class.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub process: String,
    /// Where the limits were published.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// When the limits were published or last checked, `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub published: String,
    /// The router bit that cuts the outline, in mm (#179d); absent when the
    /// profile names none. Present, the file is written at
    /// [`RULE_PROFILE_SCHEMA_V2`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub router_tool_diameter: Option<f64>,
}

/// Why a profile file could not be used.
#[derive(Debug, Error)]
pub enum RuleProfileError {
    #[error("not a valid rule profile: {0}")]
    Parse(String),
    #[error("profile schema {0} is newer than this Signex understands")]
    UnsupportedSchema(u32),
}

impl RuleProfile {
    /// A fresh profile at the current schema.
    pub fn new(name: impl Into<String>, version: impl Into<String>, erc: ErcRules) -> Self {
        Self {
            schema: Some(RULE_PROFILE_SCHEMA_V1),
            name: name.into(),
            version: version.into(),
            erc,
            drc: DrcRuleSet::default(),
            fab: None,
            unknown: UnknownJson::default(),
        }
    }

    /// Read a `.snxrules` file's bytes.
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, RuleProfileError> {
        let profile: Self =
            serde_json::from_slice(bytes).map_err(|e| RuleProfileError::Parse(e.to_string()))?;
        match profile.schema {
            Some(v) if v > RULE_PROFILE_SCHEMA_V2 => Err(RuleProfileError::UnsupportedSchema(v)),
            _ => Ok(profile),
        }
    }

    /// The router tool diameter the `fab` section names, if any.
    pub fn router_tool_diameter(&self) -> Option<f64> {
        self.fab.as_ref().and_then(|fab| fab.router_tool_diameter)
    }

    /// The schema the file is written at: 2 while the `fab` section names a
    /// router tool diameter, else as read (a 2 with nothing that needs it
    /// goes back to 1), so a file without the value re-saves as today.
    pub fn written_schema(&self) -> Option<u32> {
        if self.router_tool_diameter().is_some() {
            return Some(RULE_PROFILE_SCHEMA_V2);
        }
        match self.schema {
            Some(RULE_PROFILE_SCHEMA_V2) => Some(RULE_PROFILE_SCHEMA_V1),
            other => other,
        }
    }

    /// The bytes of the `.snxrules` file: pretty JSON, ordered keys, one
    /// trailing newline, so a rule change is a small git diff. The schema
    /// is [`Self::written_schema`].
    pub fn to_json_bytes(&self) -> Vec<u8> {
        let written = self.written_schema();
        // Strings and JSON values always serialise.
        let mut bytes = if written == self.schema {
            serde_json::to_vec_pretty(self).unwrap_or_default()
        } else {
            let mut copy = self.clone();
            copy.schema = written;
            serde_json::to_vec_pretty(&copy).unwrap_or_default()
        };
        bytes.push(b'\n');
        bytes
    }

    /// Hash of the content that decides check results: the `erc` rule set
    /// and, when present, the `drc` section ([`super::rule_set_sha256`]). The
    /// name and version are labels and do not enter it, so renaming a profile
    /// is not drift.
    pub fn content_sha256(&self) -> String {
        super::rule_set_sha256(&self.erc, &self.drc)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod router_tests;
