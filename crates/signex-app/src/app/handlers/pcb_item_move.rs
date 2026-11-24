//! Every other selectable board object moved by a drag (#2870): tracks,
//! free arcs and pads, fills, solid regions, pours, keepouts and cutouts,
//! strings, dimensions, drawings and rooms. A press on one carries it, or
//! the selection it is part of; the release moves them as one board undo
//! step (`signex_engine::pcb::item_move`, a room by `MoveRoom` with the
//! parts in it), snapped as a Move snaps. A locked object refuses at the
//! press, and the press then carries nothing.

use std::collections::BTreeSet;
use std::path::Path;

use signex_engine::pcb::item_move::move_items;
use signex_engine::pcb::placement::PlacementRefusal;
use signex_engine::pcb::{BoardCommand, BoardCommandError, BoardItemId};
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::pcb_placement::refuse;
use crate::app::Signex;
use crate::pcb_canvas::{
    CarryKind, DraggedItems, PcbCarry, carried_item_ids, item_carry_delta, item_points,
};

/// The commands that move `items` and `rooms` of `board` by `delta`, one
/// undo step; refused whole when one is locked or a room is not a box.
fn move_commands(
    board: &PcbBoard,
    items: &BTreeSet<Uuid>,
    rooms: &BTreeSet<Uuid>,
    delta: Point,
) -> Result<Vec<BoardCommand>, PlacementRefusal> {
    let ids: BTreeSet<BoardItemId> = carried_item_ids(board, items).into_iter().collect();
    let mut commands = if ids.is_empty() {
        Vec::new()
    } else {
        move_items(board, &ids, delta)?
    };
    for &id in rooms {
        let room = (board.rooms.iter().find(|r| r.uuid == id))
            .ok_or(BoardCommandError::RoomInvalid { field: "room" })?;
        if room.locked {
            return Err(BoardCommandError::RoomLocked(id).into());
        }
        let b =
            (room.outline.bounds()).ok_or(BoardCommandError::RoomInvalid { field: "outline" })?;
        if delta.x != 0.0 || delta.y != 0.0 {
            let to = Point::new(b.x_mm + delta.x, b.y_mm + delta.y);
            commands.push(BoardCommand::MoveRoom { room: id, to });
        }
    }
    Ok(commands)
}

impl Signex {
    /// A press at `from` on a track, arc, area, string, dimension, drawing
    /// or room starts a drag of it. Whether it did.
    pub(super) fn start_item_drag(&mut self, path: &Path, from: Point, reach: f64) -> bool {
        let Some(dragged) = self
            .with_pcb_selection(|state, board| state.grab_items_for_drag(board, from, reach))
            .flatten()
        else {
            return false;
        };
        let DraggedItems { items, rooms } = dragged;
        let refused = (self.active_pcb())
            .and_then(|board| move_commands(board, &items, &rooms, Point::new(0.0, 0.0)).err());
        let window = self.interaction_state.acting_window;
        let mut carry = PcbCarry::new(CarryKind::Drag, path.to_path_buf(), window, BTreeSet::new());
        carry.anchor = Some(from);
        match refused {
            Some(refusal) => refuse("Move", &refusal),
            None => {
                carry.items = items;
                carry.rooms = rooms;
            }
        }
        self.interaction_state.pcb_carry = Some(carry);
        self.refresh_pcb_highlight();
        true
    }

    /// The offset of what `carry` holds for a pointer gone from `from` to
    /// `to`, under the Snap options.
    pub(super) fn item_drag_delta(
        &self,
        carry: &PcbCarry,
        from: Point,
        to: Point,
    ) -> Option<Point> {
        let board = self.active_pcb()?;
        let skip = carry.snap_skip(board);
        let mut points = item_points(board, &carry.items);
        let rooms = board.rooms.iter().filter(|r| carry.rooms.contains(&r.uuid));
        points.extend(rooms.flat_map(|r| r.outline.corners()));
        let view = self.pcb_snap_view();
        Some(item_carry_delta(board, &points, &skip, from, to, view))
    }

    /// Release an item drag at `to`: move what it carries as one step.
    pub(super) fn drop_item_carry(&mut self, carry: &PcbCarry, from: Point, to: Point) {
        let built = self.item_drag_delta(carry, from, to).and_then(|delta| {
            let board = self.active_pcb()?;
            Some(move_commands(board, &carry.items, &carry.rooms, delta))
        });
        self.end_pcb_carry();
        if let Some(built) = built {
            self.run_placement("Move", built);
        }
    }
}
