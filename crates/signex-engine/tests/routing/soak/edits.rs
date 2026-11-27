//! The soak's editing actions (#2482, #2967). Each is built by the function
//! the editor's own tool or drag builds it with, and lands as one undo
//! step: a track moved alone, a track dragged with the tracks joined to its
//! ends stretched after it (`item_move::move_items` and
//! `placement::stretch_commands`, as a drop does), a via placed on a track
//! end (`draw::place_via`, as the Via tool does), a via moved with its
//! tracks (`placement::move_vias`), a footprint moved, a track or via
//! deleted. The time from picking the item to the landed step is recorded.

use std::collections::BTreeSet;
use std::time::Instant;

use signex_engine::pcb::draw::place_via;
use signex_engine::pcb::item_move::move_items;
use signex_engine::pcb::placement::{move_vias, stretch_commands};
use signex_engine::pcb::{BoardCommand, BoardItem, BoardItemId};
use signex_types::object_id::IdSource;
use signex_types::pcb::{Point, Segment};
use uuid::Uuid;

use super::{Action, SOAK_IDS, Soak, snapshot};
use crate::session::random_corner;

/// New ids for the soak's edits, from the same count as its routes'.
struct SoakIds<'a>(&'a mut u128);

impl IdSource for SoakIds<'_> {
    fn next_id(&mut self) -> Uuid {
        *self.0 += 1;
        Uuid::from_u128(SOAK_IDS | *self.0)
    }
}

impl Soak<'_> {
    /// Build `action`'s commands on the board and land them as one step;
    /// nothing when there is nothing to act on or the tool refuses.
    pub(super) fn edit(&mut self, action: Action) -> Result<(), String> {
        let before = snapshot(self.engine.board());
        let started = Instant::now();
        let commands = match action {
            Action::MoveTrack => self.move_track(),
            Action::DragTrack => self.drag_track(),
            Action::PlaceVia => self.place_via(),
            Action::MoveVia => self.move_via(),
            Action::MoveFootprint => self.move_footprint(),
            Action::Delete => self.delete(),
            Action::Route | Action::Shove | Action::Undo | Action::Redo => {
                unreachable!("{action:?} is not an edit")
            }
        };
        if commands.is_empty() {
            return Ok(());
        }
        self.last = format!("{commands:?}");
        self.engine
            .execute_batch(commands.clone())
            .map_err(|e| format!("the engine refused {commands:?}: {e}"))?;
        self.latencies.push((action, started.elapsed()));
        self.tally.edits += 1;
        match action {
            Action::DragTrack => self.tally.drags += 1,
            Action::PlaceVia => self.tally.vias_placed += 1,
            Action::MoveVia => self.tally.vias_moved += 1,
            _ => {}
        }
        self.stepped(before);
        Ok(())
    }

    fn unlocked_tracks(&self) -> Vec<Segment> {
        (self.engine.board().segments.iter())
            .filter(|s| !s.locked)
            .cloned()
            .collect()
    }

    /// A step of up to `reach` mm each way, on a 0.05 mm grid.
    fn delta(&mut self, reach: f64) -> Point {
        Point::new(
            self.rng.offset_mm(reach, 0.05),
            self.rng.offset_mm(reach, 0.05),
        )
    }

    /// A track moved alone: its ends leave what they were on.
    fn move_track(&mut self) -> Vec<BoardCommand> {
        let tracks = self.unlocked_tracks();
        let Some(track) = self.rng.pick(&tracks).cloned() else {
            return Vec::new();
        };
        let (dx, dy) = (self.rng.offset_mm(1.0, 0.05), self.rng.offset_mm(1.0, 0.05));
        let shift = |p: Point| Point::new(p.x + dx, p.y + dy);
        self.touch(track.net);
        let moved = Segment {
            start: shift(track.start),
            end: shift(track.end),
            ..track
        };
        vec![BoardCommand::ReplaceItems {
            items: vec![BoardItem::Segment(moved)],
        }]
    }

    /// A track dragged as the editor drops a dragged track (#2513): moved by
    /// the offset, the tracks joined to its ends stretched after it and
    /// bent at a random corner rule.
    fn drag_track(&mut self) -> Vec<BoardCommand> {
        let tracks = self.unlocked_tracks();
        let Some(track) = self.rng.pick(&tracks).cloned() else {
            return Vec::new();
        };
        let delta = self.delta(1.0);
        let corner = pcb_geom::stretch_rule(random_corner(&mut self.rng));
        self.touch(track.net);
        let board = self.engine.board();
        let ids = BTreeSet::from([BoardItemId::Segment(track.uuid)]);
        let Ok(mut commands) = move_items(board, &ids, delta) else {
            return Vec::new();
        };
        if commands.is_empty() {
            return commands;
        }
        let carried = BTreeSet::from([track.uuid]);
        let following = pcb_geom::tracks_on_tracks(board, &carried, &carried);
        let mut new_ids = SoakIds(&mut self.next_id);
        commands.extend(stretch_commands(
            board,
            &following,
            delta,
            corner,
            &mut new_ids,
        ));
        commands
    }

    /// A via placed by the Via tool on a random track end, on the net of the
    /// copper there.
    fn place_via(&mut self) -> Vec<BoardCommand> {
        let ends: Vec<Point> = (self.engine.board().segments.iter())
            .flat_map(|s| [s.start, s.end])
            .collect();
        let Some(&at) = self.rng.pick(&ends) else {
            return Vec::new();
        };
        self.next_id += 1;
        let id = Uuid::from_u128(SOAK_IDS | self.next_id);
        let command = place_via(self.engine.board(), id, at);
        if let BoardCommand::AddItems { items } = &command {
            let nets: Vec<u32> = (items.iter())
                .filter_map(|item| match item {
                    BoardItem::Via(via) => Some(via.net),
                    _ => None,
                })
                .collect();
            nets.into_iter().for_each(|net| self.touch(net));
        }
        vec![command]
    }

    /// A via dragged with the tracks on it, bent at a random corner rule; a
    /// move the tool refuses (an arc that would collapse) is not made.
    fn move_via(&mut self) -> Vec<BoardCommand> {
        let vias: Vec<(Uuid, u32)> = (self.engine.board().vias.iter())
            .filter(|v| !v.locked)
            .map(|v| (v.uuid, v.net))
            .collect();
        let Some(&(via, net)) = self.rng.pick(&vias) else {
            return Vec::new();
        };
        let delta = self.delta(1.0);
        let corner = pcb_geom::stretch_rule(random_corner(&mut self.rng));
        self.touch(net);
        let board = self.engine.board();
        let mut new_ids = SoakIds(&mut self.next_id);
        match move_vias(board, &BTreeSet::from([via]), delta, corner, &mut new_ids) {
            Ok(commands) => commands,
            Err(refused) => {
                self.last = format!("MoveVia {via} refused: {refused}");
                Vec::new()
            }
        }
    }

    /// A footprint moved alone; its pads may change any net's links.
    fn move_footprint(&mut self) -> Vec<BoardCommand> {
        let footprints: Vec<(Uuid, Point)> = (self.engine.board().footprints.iter())
            .filter(|f| !f.locked)
            .map(|f| (f.uuid, f.position))
            .collect();
        let Some(&(footprint, at)) = self.rng.pick(&footprints) else {
            return Vec::new();
        };
        let by = self.delta(2.0);
        self.touched = None;
        let to = Point::new(at.x + by.x, at.y + by.y);
        vec![BoardCommand::MoveFootprint { footprint, to }]
    }

    fn delete(&mut self) -> Vec<BoardCommand> {
        let board = self.engine.board();
        let items: Vec<(BoardItemId, u32)> = (board.segments.iter())
            .filter(|s| !s.locked)
            .map(|s| (BoardItemId::Segment(s.uuid), s.net))
            .chain(
                (board.vias.iter())
                    .filter(|v| !v.locked)
                    .map(|v| (BoardItemId::Via(v.uuid), v.net)),
            )
            .collect();
        let Some(&(item, net)) = self.rng.pick(&items) else {
            return Vec::new();
        };
        self.touch(net);
        vec![BoardCommand::DeleteItems { items: vec![item] }]
    }
}
