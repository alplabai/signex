//! Lock and unlock tracks and vias (#2407). A footprint has its own lock
//! (`SetFootprintLocked`). What a lock refuses: a checked replace of the
//! item ([`super::replace`]) here, and move and delete in
//! [`super::placement`].

use std::collections::HashSet;

use signex_types::pcb::PcbBoard;

use super::command::{Applied, BoardCommand, BoardPatch, BoardPatchKind};
use super::error::BoardCommandError;
use super::items::{self, BoardItemId};

/// Set each named track or via locked or not. Refused whole for an id named
/// twice, gone, or not a track or via. The inverse sets the states it
/// replaced.
pub(super) fn set_locked(
    board: &mut PcbBoard,
    locks: Vec<(BoardItemId, bool)>,
) -> Result<Applied, BoardCommandError> {
    let mut seen = HashSet::with_capacity(locks.len());
    let mut slots = Vec::with_capacity(locks.len());
    for &(id, _) in &locks {
        if !matches!(id, BoardItemId::Segment(_) | BoardItemId::Via(_)) {
            return Err(BoardCommandError::NotLockable(id));
        }
        if !seen.insert(id) {
            return Err(BoardCommandError::DuplicateItem(id));
        }
        slots.push(items::position(board, id).ok_or(BoardCommandError::ItemNotFound(id))?);
    }
    let mut previous = Vec::with_capacity(locks.len());
    for ((id, locked), index) in locks.iter().copied().zip(slots) {
        let slot = match id {
            BoardItemId::Segment(_) => &mut board.segments[index].locked,
            _ => &mut board.vias[index].locked,
        };
        previous.push((id, std::mem::replace(slot, locked)));
    }
    Ok(Applied {
        inverse: BoardCommand::SetItemsLocked { items: previous },
        patch: BoardPatch {
            kind: BoardPatchKind::ItemsLockSet,
            items: locks.into_iter().map(|(id, _)| id).collect(),
        },
    })
}

/// Whether `id` is a locked track or via.
pub fn is_locked(board: &PcbBoard, id: BoardItemId) -> bool {
    match id {
        BoardItemId::Segment(uuid) => board.segments.iter().any(|s| s.uuid == uuid && s.locked),
        BoardItemId::Via(uuid) => board.vias.iter().any(|v| v.uuid == uuid && v.locked),
        _ => false,
    }
}

/// How a lock refusal names each locked track or via among `items`.
pub fn locked_names(board: &PcbBoard, items: impl IntoIterator<Item = BoardItemId>) -> Vec<String> {
    items
        .into_iter()
        .filter(|id| is_locked(board, *id))
        .map(|id| match id {
            BoardItemId::Segment(uuid) => format!("track {uuid}"),
            BoardItemId::Via(uuid) => format!("via {uuid}"),
            other => format!("{other:?}"),
        })
        .collect()
}

#[cfg(test)]
#[path = "lock_tests.rs"]
mod tests;
