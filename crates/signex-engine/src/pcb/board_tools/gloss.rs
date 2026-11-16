//! Gloss (#2408): the selected routes with their free corners cut where the
//! straight track stays clear (`pcb_geom::gloss`), as one undo step: the
//! merged tracks are removed and the kept ones replaced in place.

use std::collections::BTreeSet;

use pcb_geom::gloss;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::{ToolEdit, ToolRefusal, ToolReport};
use crate::pcb::command::BoardCommand;
use crate::pcb::items::{BoardItem, BoardItemId};

/// Gloss the tracks `tracks` names, kept `clearance(a, b)` from copper of
/// other nets.
pub fn gloss_tracks<F: Fn(u32, u32) -> i64>(
    board: &PcbBoard,
    tracks: &BTreeSet<Uuid>,
    clearance: F,
) -> Result<ToolEdit, ToolRefusal> {
    if !board.segments.iter().any(|s| tracks.contains(&s.uuid)) {
        return Err(ToolRefusal::NothingSelected);
    }
    let glossed = gloss(board, tracks, clearance);
    if glossed.is_empty() {
        return Err(ToolRefusal::NothingToGloss);
    }
    let report = ToolReport {
        removed: glossed.removed.clone(),
        changed: glossed.replaced.iter().map(|s| s.uuid).collect(),
        ..ToolReport::default()
    };
    let mut commands = vec![BoardCommand::DeleteItems {
        items: glossed
            .removed
            .into_iter()
            .map(BoardItemId::Segment)
            .collect(),
    }];
    if !glossed.replaced.is_empty() {
        commands.push(BoardCommand::ReplaceItems {
            items: glossed
                .replaced
                .into_iter()
                .map(BoardItem::Segment)
                .collect(),
        });
    }
    Ok(ToolEdit { commands, report })
}
