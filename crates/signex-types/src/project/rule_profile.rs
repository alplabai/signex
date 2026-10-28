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
//! Keys this build does not know (a `drc` section from a newer build, an
//! unknown rule id) are kept verbatim and written back, as everywhere else.
//! A `schema` newer than [`RULE_PROFILE_SCHEMA_V1`] is refused rather than
//! half-read, because applying part of a rule set silently changes results.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{ErcRules, UnknownJson};

/// Current version of the profile file, independent of the project's
/// `rules.schema`.
pub const RULE_PROFILE_SCHEMA_V1: u32 = 1;

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
    /// Keys written by a newer build; kept so a save does not drop them.
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
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
            unknown: UnknownJson::default(),
        }
    }

    /// Read a `.snxrules` file's bytes.
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, RuleProfileError> {
        let profile: Self =
            serde_json::from_slice(bytes).map_err(|e| RuleProfileError::Parse(e.to_string()))?;
        match profile.schema {
            Some(v) if v > RULE_PROFILE_SCHEMA_V1 => Err(RuleProfileError::UnsupportedSchema(v)),
            _ => Ok(profile),
        }
    }

    /// The bytes of the `.snxrules` file: pretty JSON, ordered keys, one
    /// trailing newline, so a rule change is a small git diff.
    pub fn to_json_bytes(&self) -> Vec<u8> {
        // Strings and JSON values always serialise.
        let mut bytes = serde_json::to_vec_pretty(self).unwrap_or_default();
        bytes.push(b'\n');
        bytes
    }

    /// Hash of the content that decides results (the rule set). The name and
    /// version are labels and do not enter it, so renaming a profile is not
    /// drift.
    pub fn content_sha256(&self) -> String {
        self.erc.sha256_hex()
    }
}

#[cfg(test)]
mod tests;
