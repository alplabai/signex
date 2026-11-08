//! Wire, bus and bus-entry commits as plain-value methods (#1930). The
//! click handler keeps the drawing state (first click anchors, each next
//! click commits from the last anchor); these place what one commit adds.

use signex_types::schematic::{Bus, BusEntry, Point, Wire};

use super::super::super::helpers::constrain_segments;
use super::super::super::*;

impl Signex {
    /// Place wire segments from `start` to `end`, both already snapped, bent
    /// as `mode` routes them. The point the last segment ends on, which the
    /// next segment starts from.
    pub(crate) fn place_wire_at(&mut self, start: Point, end: Point, mode: DrawMode) -> Point {
        let segments = constrain_segments(start, end, mode);
        let (stroke_width, color) = self.wire_placement_style();
        let commands: Vec<_> = segments
            .iter()
            .map(|seg| signex_engine::Command::PlaceWireSegment {
                wire: Wire {
                    unknown_columns: Default::default(),
                    uuid: uuid::Uuid::now_v7(),
                    start: seg.0,
                    end: seg.1,
                    stroke_width,
                    color,
                    locked: false,
                },
            })
            .collect();
        if !commands.is_empty() {
            self.apply_engine_commands(commands, false, false);
        }
        segments.last().map(|s| s.1).unwrap_or(end)
    }

    /// The width and colour the next wire is placed with: what the Tab form
    /// of the Wire tool holds, else the default width and no colour.
    fn wire_placement_style(&self) -> (f64, Option<signex_types::theme::Color>) {
        match &self.document_state.panel_ctx.pre_placement {
            Some(pp) if pp.kind == crate::panels::PrePlacementKind::Wire => {
                (pp.wire_width.stroke_mm(), pp.wire_color)
            }
            _ => (0.0, None),
        }
    }

    /// [`Self::place_wire_at`] for a bus.
    pub(crate) fn place_bus_at(&mut self, start: Point, end: Point, mode: DrawMode) -> Point {
        let segments = constrain_segments(start, end, mode);
        let commands: Vec<_> = segments
            .iter()
            .map(|seg| signex_engine::Command::PlaceBus {
                bus: Bus {
                    uuid: uuid::Uuid::now_v7(),
                    start: seg.0,
                    end: seg.1,
                },
            })
            .collect();
        if !commands.is_empty() {
            self.apply_engine_commands(commands, false, false);
        }
        segments.last().map(|s| s.1).unwrap_or(end)
    }

    /// Place a bus entry of `size` at `at`. Its bus end snaps onto the
    /// nearest bus within one grid step; off a bus it lands on `at`.
    pub(crate) fn place_bus_entry_at(&mut self, at: Point, size: (f64, f64)) -> bool {
        let position = self
            .document_state
            .active_engine()
            .and_then(|engine| {
                signex_types::schematic::snap_to_bus(
                    &engine.document().buses,
                    at,
                    crate::canvas::grid::pitch_mm_exact(self.ui_state.grid_size_mm),
                )
            })
            .unwrap_or(at);
        let bus_entry = BusEntry {
            uuid: uuid::Uuid::now_v7(),
            position,
            size,
        };
        self.apply_engine_command(
            signex_engine::Command::PlaceBusEntry { bus_entry },
            false,
            false,
        )
    }
}
