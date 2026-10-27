//! The Re-prefix to standard confirm (#1315): what the user is shown before
//! anything is rewritten. Built from a plan, pure text and counts.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use signex_types::designator::DesignatorStandard;
use uuid::Uuid;

use super::reprefix_plan::{ReprefixPlan, SkipReason};
use crate::library::reprefix_lookup::ReprefixLookup;

/// Changes named in the confirm; the rest are counted ("+K more"), not hidden.
const NAMED_CHANGES: usize = 60;

/// An open confirm. It keeps the library answer so Confirm can re-plan
/// against the state of THAT moment without asking the libraries again.
#[derive(Debug, Clone)]
pub struct ReprefixConfirm {
    /// Token of the request this confirm answers; Confirm applies only while
    /// it is still the newest one.
    pub generation: u64,
    pub project: Option<PathBuf>,
    pub lookup: Arc<ReprefixLookup>,
    pub standard: DesignatorStandard,
    /// The changes shown, so a re-plan that differs is shown again instead of
    /// being applied unseen.
    pub shown: HashMap<Uuid, String>,
    pub headline: String,
    /// `D5: D -> V`, up to [`NAMED_CHANGES`] of them.
    pub change_lines: Vec<String>,
    /// Changes beyond the named ones.
    pub more: usize,
    /// How many parts stay as they are, and why.
    pub skip_lines: Vec<String>,
    /// The Confirm button's label.
    pub button_label: String,
}

impl ReprefixConfirm {
    pub(super) fn from_plan(
        generation: u64,
        project: Option<PathBuf>,
        lookup: Arc<ReprefixLookup>,
        standard: DesignatorStandard,
        plan: &ReprefixPlan,
    ) -> Self {
        let parts = plan.changes.len();
        let headline = format!(
            "Re-issue the prefix of {parts} part(s) across {} sheet(s) under {standard}, then \
             renumber them. It is one undo step; every sheet, open or not, stays unsaved \
             until you save it. PCB references are not updated.",
            plan.sheets_changed
        );
        let change_lines: Vec<String> = plan
            .changes
            .iter()
            .take(NAMED_CHANGES)
            .map(|c| format!("{}:  {} -> {}", c.reference, c.old_prefix, c.new_prefix))
            .collect();
        let mut skipped: std::collections::BTreeMap<SkipReason, usize> = Default::default();
        for part in &plan.skipped {
            *skipped.entry(part.reason).or_default() += 1;
        }
        Self {
            generation,
            project,
            lookup,
            standard,
            shown: plan.targets(),
            headline,
            more: parts - change_lines.len(),
            change_lines,
            skip_lines: skipped
                .into_iter()
                .map(|(reason, n)| format!("{n} part(s) {}: left unchanged", reason.heading()))
                .collect(),
            button_label: format!("Re-prefix {parts} part(s)"),
        }
    }
}
