//! The result of a design check run on demand (#2199, decision
//! https://github.com/alplabai/signex/issues/2199#issuecomment-6798772157):
//! one shape for every check (`check_erc` now, DRC and the requirement checks
//! later). What only one check knows goes in a finding's `evidence`. Shape and
//! rules: `docs/CLI.md`, "Checks on demand".

use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use crate::violation::Severity;

/// One finding of a check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    /// The rule that fired, as the check names it (`builtin::unused_pin`).
    pub rule: String,
    pub severity: Severity,
    pub message: String,
    /// What the check knows about the finding beyond the fields every check
    /// has (a location, a net); an object, by the check's own field names.
    pub evidence: Value,
    /// The ids of the objects the finding is about.
    pub items: Vec<String>,
    /// The document the finding is filed under.
    pub sheet: String,
    /// The same text for the same finding in every run, unique in a report.
    pub key: String,
}

/// A finding a waiver hides, and that waiver.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaivedFinding {
    pub finding: Finding,
    pub waiver: Value,
}

/// What changed since an earlier run of the same check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaselineDiff {
    /// The earlier run.
    pub run: u64,
    /// Keys of this run's findings that the earlier run did not have.
    pub new: Vec<String>,
    /// Keys the earlier run had and this one does not.
    pub gone: Vec<String>,
}

/// One run of a check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckReport {
    /// Which check ran (`erc`).
    pub check: String,
    /// This run's id; a later run names it as its baseline.
    pub run: u64,
    pub findings: Vec<Finding>,
    pub waived: Vec<WaivedFinding>,
    /// Waivers that no longer hide their finding: it changed since it was
    /// waived, and it is in `findings` again.
    pub lapsed: Vec<Value>,
    /// What the check could not do completely (a sheet it could not read).
    pub warnings: Vec<String>,
    /// The rules the run used, as the check names them (for ERC: where the
    /// rule set came from, its profile and its hash).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_set: Option<Value>,
    /// Present when the run was compared with an earlier one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<BaselineDiff>,
}

impl CheckReport {
    /// A report of `check`, run 0 until it is numbered. Findings are put in
    /// one order whatever order the check gave them in: by sheet, then
    /// errors, warnings and info, then rule, message, key and evidence. A key
    /// that repeats then gets `#2`, `#3`, … in that order, so every key is
    /// unique and the same findings get the same keys.
    pub fn new(
        check: &str,
        findings: Vec<Finding>,
        waived: Vec<WaivedFinding>,
        lapsed: Vec<Value>,
        warnings: Vec<String>,
    ) -> Self {
        let mut findings = findings;
        findings.sort_by_cached_key(order);
        unique_keys(findings.iter_mut());
        let mut waived = waived;
        waived.sort_by_cached_key(|w| order(&w.finding));
        unique_keys(waived.iter_mut().map(|w| &mut w.finding));
        Self {
            check: check.to_string(),
            run: 0,
            findings,
            waived,
            lapsed,
            warnings,
            rule_set: None,
            baseline: None,
        }
    }

    /// This report naming the rules its run used.
    pub fn with_rule_set(self, rule_set: Value) -> Self {
        Self {
            rule_set: Some(rule_set),
            ..self
        }
    }

    /// The keys of the findings, in report order.
    pub fn keys(&self) -> Vec<String> {
        self.findings.iter().map(|f| f.key.clone()).collect()
    }

    /// This report compared with the run `run`, whose finding keys were
    /// `earlier`.
    pub fn compared_with(self, run: u64, earlier: &[String]) -> Self {
        let now: BTreeSet<&String> = self.findings.iter().map(|f| &f.key).collect();
        let before: BTreeSet<&String> = earlier.iter().collect();
        let new = self
            .findings
            .iter()
            .filter(|f| !before.contains(&f.key))
            .map(|f| f.key.clone())
            .collect();
        let gone = earlier
            .iter()
            .filter(|key| !now.contains(key))
            .cloned()
            .collect();
        Self {
            baseline: Some(BaselineDiff { run, new, gone }),
            ..self
        }
    }
}

/// Where `finding` goes in a report: see [`CheckReport::new`].
fn order(finding: &Finding) -> (String, u8, String, String, String, String) {
    let severity = match finding.severity {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
    };
    (
        finding.sheet.clone(),
        severity,
        finding.rule.clone(),
        finding.message.clone(),
        finding.key.clone(),
        finding.evidence.to_string(),
    )
}

/// Give every repeat of a key its occurrence number.
fn unique_keys<'a>(findings: impl Iterator<Item = &'a mut Finding>) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    for finding in findings {
        let count = seen.entry(finding.key.clone()).or_default();
        *count += 1;
        if *count > 1 {
            finding.key = format!("{}#{count}", finding.key);
        }
    }
}

#[cfg(test)]
mod tests;
