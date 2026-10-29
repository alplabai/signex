//! A per-violation ERC waiver (#1285), stored in `rules.erc_waivers` of a
//! `.snxprj`. This crate only stores it; `signex-erc` owns what the
//! identity and fingerprint mean (`signex_erc::waivers::violation`).

use serde::{Deserialize, Serialize};

use super::UnknownJson;

/// One accepted ERC finding. Every field loads with a default so a partial
/// or hand-edited entry still opens; one without a reason or author is kept
/// on disk but never honoured.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ErcWaiver {
    /// Stable rule id (`builtin::unused_pin`, `user::my_rule`).
    #[serde(default)]
    pub rule_id: String,
    /// UUIDs of the objects the finding involves, sorted. This, with the rule
    /// id, is the waiver's identity: positions never are.
    #[serde(default)]
    pub objects: Vec<String>,
    /// Hash of the involved objects' content when the waiver was made. Any
    /// edit to one of them changes it, so the waiver lapses.
    #[serde(default)]
    pub fingerprint: String,
    /// Why the finding is acceptable. Required.
    #[serde(default)]
    pub reason: String,
    /// Who accepted it. Required.
    #[serde(default)]
    pub author: String,
    /// When, as `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub date: String,
    /// The finding's message when waived, so a lapsed waiver still says what
    /// it was about.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub message: String,
    /// Keys written by a newer build.
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

impl ErcWaiver {
    /// Whether the waiver may hide a finding: it names a rule and objects
    /// and carries a non-blank reason and author.
    pub fn is_complete(&self) -> bool {
        !self.rule_id.is_empty()
            && !self.objects.is_empty()
            && !self.fingerprint.is_empty()
            && !self.reason.trim().is_empty()
            && !self.author.trim().is_empty()
    }

    /// Same rule and same objects as `other`: the two waive one finding.
    pub fn same_identity(&self, rule_id: &str, objects: &[String]) -> bool {
        self.rule_id == rule_id && self.objects == objects
    }
}
