//! The Re-prefix to standard confirm (#1315, #77): what the user is shown
//! before anything is rewritten. Built from a plan: counts, the change rows
//! and the skip counts; the view words them through the catalog.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use signex_i18n::{MessageId, ids};
use signex_types::designator::DesignatorStandard;
use uuid::Uuid;

use super::reprefix_plan::{ReprefixPlan, SkipReason};
use crate::library::reprefix_lookup::ReprefixLookup;

/// Changes named in the confirm; the last row counts the rest.
const NAMED_CHANGES: usize = 60;

/// One change row: the part and its old and new prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReprefixChangeRow {
    /// The part's designator now (`D5`, `U?`).
    pub reference: String,
    pub old_prefix: String,
    pub new_prefix: String,
    /// Matched through its library symbol, not a stored row (#1417).
    pub by_symbol: bool,
}

/// How many parts one skip reason leaves as they are, and the catalog
/// message that counts them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReprefixSkipCount {
    pub label: MessageId,
    pub count: usize,
}

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
    /// Up to [`NAMED_CHANGES`] change rows, in plan order.
    pub changes: Vec<ReprefixChangeRow>,
    /// Changes beyond the named ones.
    pub more: usize,
    /// Distinct sheets holding a unit of a changed part.
    pub sheets: usize,
    /// Parts matched through their library symbol.
    pub by_symbol: usize,
    /// Parts left as they are, one count per reason.
    pub skips: Vec<ReprefixSkipCount>,
    /// How many parts the Confirm button re-prefixes.
    pub parts: usize,
}

/// The catalog message that counts the parts `reason` leaves unchanged.
fn skip_label(reason: SkipReason) -> MessageId {
    match reason {
        SkipReason::ReadOnlySheet => ids::DIALOG_REPREFIX_SKIP_READ_ONLY,
        SkipReason::Locked => ids::DIALOG_REPREFIX_SKIP_LOCKED,
        SkipReason::NoRowLink => ids::DIALOG_REPREFIX_SKIP_NO_LINK,
        SkipReason::LibraryNotMounted => ids::DIALOG_REPREFIX_SKIP_NOT_MOUNTED,
        SkipReason::RowMissing => ids::DIALOG_REPREFIX_SKIP_ROW_MISSING,
        SkipReason::RowAmbiguous => ids::DIALOG_REPREFIX_SKIP_AMBIGUOUS,
        SkipReason::RetargetedByInstance => ids::DIALOG_REPREFIX_SKIP_PER_PLACEMENT,
        SkipReason::MixedVersions => ids::DIALOG_REPREFIX_SKIP_MIXED_VERSIONS,
        SkipReason::UserEdited => ids::DIALOG_REPREFIX_SKIP_HAND_EDITED,
    }
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
        let changes: Vec<ReprefixChangeRow> = (plan.changes.iter())
            .take(NAMED_CHANGES)
            .map(|c| ReprefixChangeRow {
                reference: c.reference.clone(),
                old_prefix: c.old_prefix.clone(),
                new_prefix: c.new_prefix.clone(),
                by_symbol: c.by_symbol,
            })
            .collect();
        let mut skipped: BTreeMap<SkipReason, usize> = BTreeMap::new();
        for part in &plan.skipped {
            *skipped.entry(part.reason).or_default() += 1;
        }
        Self {
            generation,
            project,
            lookup,
            standard,
            shown: plan.targets(),
            more: parts - changes.len(),
            changes,
            sheets: plan.sheets_changed,
            by_symbol: plan.changes.iter().filter(|c| c.by_symbol).count(),
            skips: skipped
                .into_iter()
                .map(|(reason, count)| ReprefixSkipCount {
                    label: skip_label(reason),
                    count,
                })
                .collect(),
            parts,
        }
    }
}
