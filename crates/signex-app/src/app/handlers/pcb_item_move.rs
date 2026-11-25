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
use signex_engine::pcb::placement::{self, PlacementRefusal};
use signex_engine::pcb::{BoardCommand, BoardCommandError, BoardItem, BoardItemId};
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::pcb_placement::refuse;
use crate::app::Signex;
use crate::pcb_canvas::{
    CarryKind, DraggedItems, DraggedSelection, PcbCarry, carried_item_ids, carry_delta,
    item_carry_delta, item_points, via_carry_delta,
};

/// The commands that move `items` and `rooms` of `board` by `delta`, one
/// undo step; refused whole when one is locked or a room is not a box. A
/// room takes its unlocked parts along, except the `carried_parts` the drag
/// moves itself, so each part moves by the offset once.
fn move_commands(
    board: &PcbBoard,
    items: &BTreeSet<Uuid>,
    rooms: &BTreeSet<Uuid>,
    carried_parts: &BTreeSet<Uuid>,
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
        if room.outline.bounds().is_none() {
            return Err(BoardCommandError::RoomInvalid { field: "outline" }.into());
        }
        if delta.x != 0.0 || delta.y != 0.0 {
            // `MoveRoom` with the parts the drag does not move already.
            let footprints = (room.members(&board.footprints))
                .filter(|fp| !fp.locked && !carried_parts.contains(&fp.uuid))
                .map(|fp| {
                    let to = Point::new(fp.position.x + delta.x, fp.position.y + delta.y);
                    (fp.uuid, to)
                })
                .collect();
            commands.push(BoardCommand::PlaceRoom {
                room: id,
                outline: room.outline.translated(delta.x, delta.y),
                footprints,
            });
        }
    }
    Ok(commands)
}

/// Why the whole selection `all` cannot be dragged: a locked or missing
/// footprint, via, object or room.
fn selection_refusal(board: &PcbBoard, all: &DraggedSelection) -> Option<PlacementRefusal> {
    let zero = Point::new(0.0, 0.0);
    let footprints = (!all.footprints.is_empty())
        .then(|| placement::editable_footprints(board, &all.footprints).err())
        .flatten();
    let vias = || {
        (!all.vias.is_empty())
            .then(|| placement::editable_vias(board, &all.vias).err())
            .flatten()
    };
    let rest = || {
        let (items, rooms) = (&all.rest.items, &all.rest.rooms);
        move_commands(board, items, rooms, &all.footprints, zero).err()
    };
    footprints.or_else(vias).or_else(rest)
}

/// The commands that move everything `carry` holds by `delta`: footprints
/// and vias with the track ends on them (none with Ctrl), then the other
/// objects and rooms. A track or arc carried whole moves whole, not again
/// by an end. One undo step.
fn mixed_commands(
    board: &PcbBoard,
    carry: &PcbCarry,
    delta: Point,
) -> Result<Vec<BoardCommand>, PlacementRefusal> {
    let mut commands = Vec::new();
    if !carry.footprints.is_empty() {
        commands.extend(if carry.leaves_tracks {
            placement::move_footprints_only(board, &carry.footprints, delta)?
        } else {
            placement::move_commands(board, &carry.footprints, delta)?
        });
    }
    if !carry.vias.is_empty() {
        commands.extend(if carry.leaves_tracks {
            placement::move_vias_only(board, &carry.vias, delta)?
        } else {
            placement::move_vias(board, &carry.vias, delta)?
        });
    }
    // The track ends the part and via moves shifted each on its own; they
    // are put back as one follow per track (`PcbCarry::following`): both
    // ends of a track from a moved pad to a moved via, none of one carried
    // whole. An arc carried whole is not also followed by an end.
    let whole_arc =
        |item: &BoardItem| matches!(item, BoardItem::Arc(a) if carry.items.contains(&a.uuid));
    for command in &mut commands {
        if let BoardCommand::ReplaceItems { items } = command {
            items.retain(|item| !matches!(item, BoardItem::Segment(_)) && !whole_arc(item));
        }
    }
    commands.retain(|c| !matches!(c, BoardCommand::ReplaceItems { items } if items.is_empty()));
    commands.extend(follow_command(board, carry, delta));
    let (items, rooms) = (&carry.items, &carry.rooms);
    commands.extend(move_commands(
        board,
        items,
        rooms,
        &carry.footprints,
        delta,
    )?);
    Ok(commands)
}

/// The track ends that follow what `carry` holds (`PcbCarry::following`),
/// moved by `delta` as the drag preview draws them; `None` with none or a
/// zero offset.
fn follow_command(board: &PcbBoard, carry: &PcbCarry, delta: Point) -> Option<BoardCommand> {
    let followed: Vec<BoardItem> = (carry.following(board).iter())
        .map(|ends| BoardItem::Segment(ends.follow(&board.segments[ends.index], delta)))
        .collect();
    let moves = delta.x != 0.0 || delta.y != 0.0;
    (!followed.is_empty() && moves).then_some(BoardCommand::ReplaceItems { items: followed })
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
        let refused = (self.active_pcb()).and_then(|board| {
            let zero = Point::new(0.0, 0.0);
            move_commands(board, &items, &rooms, &BTreeSet::new(), zero).err()
        });
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

    /// A drag just started on a member of the selection carries all of it
    /// (#2870): its footprints, vias, other objects and rooms, whichever
    /// member was pressed. A locked member refuses the whole drag at the
    /// press, which then carries nothing. A footprint text drag, a refused
    /// press and a press that replaced the selection are left as they are.
    pub(super) fn widen_drag_to_selection(&mut self) {
        let Some(carry) = self
            .live_carry(CarryKind::Drag)
            .filter(|c| c.text.is_none() && (!c.carried().is_empty() || !c.rooms.is_empty()))
        else {
            return;
        };
        let Some(all) = self.with_pcb_selection(|state, _| state.selection_for_drag()) else {
            return;
        };
        let inside = carry.footprints.is_subset(&all.footprints)
            && carry.vias.is_subset(&all.vias)
            && carry.items.is_subset(&all.rest.items)
            && carry.rooms.is_subset(&all.rest.rooms);
        let Some(board) = self.active_pcb().filter(|_| inside) else {
            return;
        };
        let refused = selection_refusal(board, &all);
        let via_tracks = !pcb_geom::tracks_on_vias(board, &all.vias).is_empty()
            || !pcb_geom::arcs_on_vias(board, &all.vias).is_empty();
        let Some(carry) = self.interaction_state.pcb_carry.as_mut() else {
            return;
        };
        match refused {
            Some(refusal) => {
                refuse("Move", &refusal);
                carry.footprints.clear();
                carry.vias.clear();
                carry.items.clear();
                carry.rooms.clear();
                carry.via_tracks = false;
            }
            None => {
                carry.footprints = all.footprints;
                carry.vias = all.vias;
                carry.items = all.rest.items;
                carry.rooms = all.rest.rooms;
                carry.via_tracks = via_tracks;
            }
        }
        self.refresh_pcb_highlight();
    }

    /// The offset of a mixed carry for a pointer gone from `from` to `to`:
    /// snapped by its footprints when it holds any, else by its vias, else
    /// as an item drag.
    pub(super) fn mixed_drag_delta(
        &self,
        carry: &PcbCarry,
        from: Point,
        to: Point,
    ) -> Option<Point> {
        let board = self.active_pcb()?;
        let skip = carry.snap_skip(board);
        let view = self.pcb_snap_view();
        if !carry.footprints.is_empty() {
            return Some(carry_delta(board, &carry.footprints, &skip, from, to, view));
        }
        if !carry.vias.is_empty() {
            return Some(via_carry_delta(board, &carry.vias, &skip, from, to, view));
        }
        self.item_drag_delta(carry, from, to)
    }

    /// Release a mixed drag at `to`: everything it carries moves by one
    /// offset as one undo step.
    pub(super) fn drop_mixed_carry(&mut self, carry: &PcbCarry, from: Point, to: Point) {
        let built = self.mixed_drag_delta(carry, from, to).and_then(|delta| {
            let board = self.active_pcb()?;
            Some(mixed_commands(board, carry, delta))
        });
        self.end_pcb_carry();
        if let Some(built) = built {
            self.run_placement("Move", built);
        }
    }

    /// Release an item drag at `to`: move what it carries, and stretch the
    /// tracks joined to a carried track's ends (#2513), as one step.
    pub(super) fn drop_item_carry(&mut self, carry: &PcbCarry, from: Point, to: Point) {
        let built = self.item_drag_delta(carry, from, to).and_then(|delta| {
            let board = self.active_pcb()?;
            let (items, rooms) = (&carry.items, &carry.rooms);
            let moved = move_commands(board, items, rooms, &carry.footprints, delta);
            Some(moved.map(|mut commands| {
                commands.extend(follow_command(board, carry, delta));
                commands
            }))
        });
        self.end_pcb_carry();
        if let Some(built) = built {
            self.run_placement("Move", built);
        }
    }
}
