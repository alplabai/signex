//! The preview list of Positional Re-Annotate (#2533): renamed parts first,
//! then the kept ones with their reason, then (when asked) the parts
//! already in order. Every footprint of the board is in exactly one of the
//! three.

use std::collections::HashSet;

use signex_engine::pcb::designators::{Kept, ReannotatePlan, Rename};
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::BoardReannotateState;

/// One line of the preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewRow<'a> {
    Renamed(&'a Rename),
    Kept(&'a Kept),
    /// Already the designator it would get.
    InOrder(&'a str),
}

/// The designators of the footprints the plan neither renames nor keeps:
/// those already in order, in board order.
pub fn in_order(board: &PcbBoard, plan: &ReannotatePlan) -> Vec<String> {
    let listed: HashSet<Uuid> = plan
        .renames
        .iter()
        .map(|r| r.footprint)
        .chain(plan.kept.iter().map(|k| k.footprint))
        .collect();
    board
        .footprints
        .iter()
        .filter(|fp| !listed.contains(&fp.uuid))
        .map(|fp| fp.reference.clone())
        .collect()
}

/// The preview lines of `state`: renames, then kept parts, then the parts
/// already in order when the toggle shows them.
pub fn rows(state: &BoardReannotateState) -> Vec<PreviewRow<'_>> {
    let renamed = state.plan.renames.iter().map(PreviewRow::Renamed);
    let kept = state.plan.kept.iter().map(PreviewRow::Kept);
    let ordered = state
        .in_order
        .iter()
        .filter(|_| state.show_in_order)
        .map(|d| PreviewRow::InOrder(d.as_str()));
    renamed.chain(kept).chain(ordered).collect()
}

#[cfg(test)]
#[path = "preview_tests.rs"]
mod tests;
