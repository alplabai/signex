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
use crate::pcb::lock::locked_names;
use crate::pcb::placement::PlacementRefusal;

/// Gloss the tracks `tracks` names, kept `clearance(a, b)` from copper of
/// other nets. A locked track is left as it is and counted as skipped; a
/// selection of locked tracks only is refused.
pub fn gloss_tracks<F: Fn(u32, u32) -> i64>(
    board: &PcbBoard,
    tracks: &BTreeSet<Uuid>,
    clearance: F,
) -> Result<ToolEdit, ToolRefusal> {
    let (locked, free): (Vec<_>, Vec<_>) = board
        .segments
        .iter()
        .filter(|s| tracks.contains(&s.uuid))
        .partition(|s| s.locked);
    if locked.is_empty() && free.is_empty() {
        return Err(ToolRefusal::NothingSelected);
    }
    if free.is_empty() {
        let names = locked_names(board, locked.iter().map(|s| BoardItemId::Segment(s.uuid)));
        return Err(PlacementRefusal::Locked(names).into());
    }
    let free: BTreeSet<Uuid> = free.iter().map(|s| s.uuid).collect();
    let glossed = gloss(board, &free, clearance);
    if glossed.is_empty() {
        return Err(ToolRefusal::NothingToGloss);
    }
    let report = ToolReport {
        removed: glossed.removed.clone(),
        changed: glossed.replaced.iter().map(|s| s.uuid).collect(),
        skipped: locked.len(),
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
