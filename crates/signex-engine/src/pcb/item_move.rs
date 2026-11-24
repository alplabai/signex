//! Board items other than footprints and vias moved by an offset, as a drag
//! moves them (#2870): tracks, free arcs and pads, drawn areas (fills,
//! solid regions, pours, keepouts, cutouts), board strings, dimensions and
//! drawings. Pure: one `ReplaceItems`, so a move is one undo step, refused
//! whole when any item is locked. Footprints and vias keep their own
//! builders in `placement`, which also carry the tracks on them.

use std::collections::BTreeSet;

use signex_types::pcb::{PcbBoard, Point};

use super::command::BoardCommand;
use super::error::BoardCommandError;
use super::items::{BoardItem, BoardItemId};
use super::placement::PlacementRefusal;

fn shift(p: Point, by: Point) -> Point {
    Point::new(p.x + by.x, p.y + by.y)
}

/// The item `id` of `board` moved by `by`; `None` for an id the board does
/// not hold, or a footprint or via (moved by `placement`).
pub fn moved_item(board: &PcbBoard, id: BoardItemId, by: Point) -> Option<BoardItem> {
    let s = |p: Point| shift(p, by);
    Some(match id {
        BoardItemId::Segment(uuid) => {
            let mut t = board.segments.iter().find(|t| t.uuid == uuid)?.clone();
            (t.start, t.end) = (s(t.start), s(t.end));
            BoardItem::Segment(t)
        }
        BoardItemId::Zone(uuid) => {
            let mut z = board.zones.iter().find(|z| z.uuid == uuid)?.clone();
            z.outline.iter_mut().for_each(|p| *p = s(*p));
            z.holes.iter_mut().flatten().for_each(|p| *p = s(*p));
            BoardItem::Zone(z)
        }
        BoardItemId::Text(uuid) => {
            let mut t = board.texts.iter().find(|t| t.uuid == uuid)?.clone();
            t.position = s(t.position);
            BoardItem::Text(t)
        }
        BoardItemId::Arc(uuid) => {
            let mut a = board.free.arcs.iter().find(|a| a.uuid == uuid)?.clone();
            (a.start, a.mid, a.end) = (s(a.start), s(a.mid), s(a.end));
            BoardItem::Arc(a)
        }
        BoardItemId::FreePad(uuid) => {
            let mut p = board.free.pads.iter().find(|p| p.uuid == uuid)?.clone();
            p.position = s(p.position);
            BoardItem::FreePad(p)
        }
        BoardItemId::Dimension(uuid) => {
            let mut d = board
                .free
                .dimensions
                .iter()
                .find(|d| d.uuid == uuid)?
                .clone();
            (d.start, d.end) = (s(d.start), s(d.end));
            BoardItem::Dimension(d)
        }
        BoardItemId::Graphic(uuid) => {
            let mut g = board.graphics.iter().find(|g| g.uuid == uuid)?.clone();
            g.start = g.start.map(s);
            g.end = g.end.map(s);
            g.center = g.center.map(s);
            g.points.iter_mut().for_each(|p| *p = s(*p));
            BoardItem::Graphic(g)
        }
        BoardItemId::Footprint(_) | BoardItemId::Via(_) => return None,
    })
}

/// Move every item in `ids` by `delta` as one undo step. Refused whole when
/// one is locked or not on the board; nothing for a zero offset.
pub fn move_items(
    board: &PcbBoard,
    ids: &BTreeSet<BoardItemId>,
    delta: Point,
) -> Result<Vec<BoardCommand>, PlacementRefusal> {
    if ids.is_empty() {
        return Err(PlacementRefusal::NothingSelected);
    }
    let locked = super::lock::locked_names(board, ids.iter().copied());
    if !locked.is_empty() {
        return Err(PlacementRefusal::Locked(locked));
    }
    let items = ids
        .iter()
        .map(|&id| moved_item(board, id, delta).ok_or(BoardCommandError::ItemNotFound(id)))
        .collect::<Result<Vec<_>, _>>()?;
    if delta.x == 0.0 && delta.y == 0.0 {
        return Ok(Vec::new());
    }
    Ok(vec![BoardCommand::ReplaceItems { items }])
}

#[cfg(test)]
#[path = "item_move_tests.rs"]
mod tests;
