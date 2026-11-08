//! Delete board items and put them back where they were.

use std::collections::HashSet;

use signex_types::pcb::{BoardText, Footprint, PcbBoard, Segment, Via, Zone};
use uuid::Uuid;

use super::command::{Applied, BoardCommand, BoardPatch, BoardPatchKind};
use super::error::BoardCommandError;

/// A board item that carries a UUID. Board graphics carry none yet, so they
/// cannot be named here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BoardItemId {
    Footprint(Uuid),
    Segment(Uuid),
    Via(Uuid),
    Zone(Uuid),
    Text(Uuid),
}

/// A removed item, owned so it can be put back.
#[derive(Debug, Clone, PartialEq)]
pub enum BoardItem {
    Footprint(Footprint),
    Segment(Segment),
    Via(Via),
    Zone(Zone),
    Text(BoardText),
}

impl BoardItem {
    pub fn id(&self) -> BoardItemId {
        match self {
            Self::Footprint(fp) => BoardItemId::Footprint(fp.uuid),
            Self::Segment(s) => BoardItemId::Segment(s.uuid),
            Self::Via(v) => BoardItemId::Via(v.uuid),
            Self::Zone(z) => BoardItemId::Zone(z.uuid),
            Self::Text(t) => BoardItemId::Text(t.uuid),
        }
    }
}

/// An item and the index it held in its board list before removal.
#[derive(Debug, Clone, PartialEq)]
pub struct RemovedItem {
    pub index: usize,
    pub item: BoardItem,
}

/// Which board list an item lives in. Ordered so sorting by
/// `(ListKind, index)` groups each list's items together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ListKind {
    Footprints,
    Segments,
    Vias,
    Zones,
    Texts,
}

impl BoardItemId {
    fn split(self) -> (ListKind, Uuid) {
        match self {
            Self::Footprint(u) => (ListKind::Footprints, u),
            Self::Segment(u) => (ListKind::Segments, u),
            Self::Via(u) => (ListKind::Vias, u),
            Self::Zone(u) => (ListKind::Zones, u),
            Self::Text(u) => (ListKind::Texts, u),
        }
    }
}

fn position(board: &PcbBoard, id: BoardItemId) -> Option<usize> {
    let (list, uuid) = id.split();
    match list {
        ListKind::Footprints => board.footprints.iter().position(|x| x.uuid == uuid),
        ListKind::Segments => board.segments.iter().position(|x| x.uuid == uuid),
        ListKind::Vias => board.vias.iter().position(|x| x.uuid == uuid),
        ListKind::Zones => board.zones.iter().position(|x| x.uuid == uuid),
        ListKind::Texts => board.texts.iter().position(|x| x.uuid == uuid),
    }
}

fn list_len(board: &PcbBoard, list: ListKind) -> usize {
    match list {
        ListKind::Footprints => board.footprints.len(),
        ListKind::Segments => board.segments.len(),
        ListKind::Vias => board.vias.len(),
        ListKind::Zones => board.zones.len(),
        ListKind::Texts => board.texts.len(),
    }
}

fn take(board: &mut PcbBoard, list: ListKind, index: usize) -> BoardItem {
    match list {
        ListKind::Footprints => BoardItem::Footprint(board.footprints.remove(index)),
        ListKind::Segments => BoardItem::Segment(board.segments.remove(index)),
        ListKind::Vias => BoardItem::Via(board.vias.remove(index)),
        ListKind::Zones => BoardItem::Zone(board.zones.remove(index)),
        ListKind::Texts => BoardItem::Text(board.texts.remove(index)),
    }
}

fn put(board: &mut PcbBoard, index: usize, item: BoardItem) {
    match item {
        BoardItem::Footprint(x) => board.footprints.insert(index, x),
        BoardItem::Segment(x) => board.segments.insert(index, x),
        BoardItem::Via(x) => board.vias.insert(index, x),
        BoardItem::Zone(x) => board.zones.insert(index, x),
        BoardItem::Text(x) => board.texts.insert(index, x),
    }
}

pub(super) fn delete(
    board: &mut PcbBoard,
    ids: &[BoardItemId],
) -> Result<Applied, BoardCommandError> {
    let mut seen = HashSet::with_capacity(ids.len());
    let mut slots = Vec::with_capacity(ids.len());
    for &id in ids {
        if !seen.insert(id) {
            return Err(BoardCommandError::DuplicateItem(id));
        }
        let index = position(board, id).ok_or(BoardCommandError::ItemNotFound(id))?;
        slots.push((id.split().0, index));
    }
    // Remove back to front so the indices still to remove stay valid.
    slots.sort_unstable();
    let mut removed: Vec<RemovedItem> = slots
        .into_iter()
        .rev()
        .map(|(list, index)| RemovedItem {
            index,
            item: take(board, list, index),
        })
        .collect();
    removed.reverse();
    Ok(Applied {
        inverse: BoardCommand::RestoreItems { items: removed },
        patch: BoardPatch {
            kind: BoardPatchKind::ItemsDeleted,
            items: ids.to_vec(),
        },
    })
}

pub(super) fn restore(
    board: &mut PcbBoard,
    mut removed: Vec<RemovedItem>,
) -> Result<Applied, BoardCommandError> {
    let ids: Vec<BoardItemId> = removed.iter().map(|r| r.item.id()).collect();
    removed.sort_by_key(|r| (r.item.id().split().0, r.index));
    check_restorable(board, &removed, &ids)?;
    // Front to back: each item lands at its index once the earlier ones of
    // its list are back, which is the list it was removed from.
    for r in removed {
        put(board, r.index, r.item);
    }
    Ok(Applied {
        inverse: BoardCommand::DeleteItems { items: ids.clone() },
        patch: BoardPatch {
            kind: BoardPatchKind::ItemsRestored,
            items: ids,
        },
    })
}

/// `sorted` is ordered by `(list, index)`.
fn check_restorable(
    board: &PcbBoard,
    sorted: &[RemovedItem],
    ids: &[BoardItemId],
) -> Result<(), BoardCommandError> {
    let mut seen = HashSet::with_capacity(ids.len());
    for &id in ids {
        if !seen.insert(id) {
            return Err(BoardCommandError::DuplicateItem(id));
        }
        if position(board, id).is_some() {
            return Err(BoardCommandError::ItemAlreadyPresent(id));
        }
    }
    let mut previous: Option<(ListKind, usize)> = None;
    let mut inserted = 0;
    for r in sorted {
        let id = r.item.id();
        let list = id.split().0;
        match previous {
            Some((l, i)) if l == list && i == r.index => {
                return Err(BoardCommandError::DuplicateRestoreIndex { index: r.index });
            }
            Some((l, _)) if l == list => inserted += 1,
            _ => inserted = 0,
        }
        let len = list_len(board, list) + inserted;
        if r.index > len {
            return Err(BoardCommandError::RestoreIndexOutOfRange {
                id,
                index: r.index,
                len,
            });
        }
        previous = Some((list, r.index));
    }
    Ok(())
}
