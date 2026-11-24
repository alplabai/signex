//! Wire, bus and bus-entry commits as plain-value methods (#1930). The
//! click handler keeps the drawing state (first click anchors, the next
//! click commits from it); these place what one commit adds.

use signex_types::corner::CornerRule;
use signex_types::schematic::{Bus, BusEntry, Point, Wire};

use super::super::super::*;

/// A wire's width in mm (0.0 = the default) and colour (`None` = the theme's).
type WireStyle = (f64, Option<signex_types::theme::Color>);

/// The style a new wire takes: each override of the wire it starts on wins
/// over `base` (the Tab form's or the default); a field the start wire does
/// not override keeps `base` (docs/DECISIONS.md, 2026-11-21).
fn inherited_style(base: WireStyle, start: Option<WireStyle>) -> WireStyle {
    let Some((width, color)) = start else {
        return base;
    };
    (if width != 0.0 { width } else { base.0 }, color.or(base.1))
}

/// The commands that place `segments` as wires of `style` (width, colour):
/// one per segment.
pub(crate) fn wire_commands(
    segments: &[(Point, Point)],
    (stroke_width, color): (f64, Option<signex_types::theme::Color>),
) -> Vec<signex_engine::Command> {
    segments
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
        .collect()
}

/// [`wire_commands`] for a bus.
pub(crate) fn bus_commands(segments: &[(Point, Point)]) -> Vec<signex_engine::Command> {
    segments
        .iter()
        .map(|seg| signex_engine::Command::PlaceBus {
            bus: Bus {
                uuid: uuid::Uuid::now_v7(),
                start: seg.0,
                end: seg.1,
                ..Bus::default()
            },
        })
        .collect()
}

impl Signex {
    /// Place wire segments from `start` to `end`, both already snapped, bent
    /// as `rule` bends them. The point the last segment ends on, which the
    /// next segment starts from, and whether the document changed.
    pub(crate) fn place_wire_at(
        &mut self,
        start: Point,
        end: Point,
        rule: CornerRule,
    ) -> (Point, bool) {
        let segments = rule.legs(start, end);
        let style = inherited_style(
            self.wire_placement_style(),
            self.interaction_state.wire_start_style,
        );
        let commands = wire_commands(&segments, style);
        let changed = !commands.is_empty() && self.apply_engine_commands(commands, false, false);
        (segments.last().map(|s| s.1).unwrap_or(end), changed)
    }

    /// Place one straight wire from `start` to `end` as typed in the Tab
    /// form (#2532): one undo step. Whether the document changed; a wire
    /// whose ends meet (the same 1 µm point, `signex_net::pt_key`) is not
    /// placed.
    pub(crate) fn place_wire_segment(&mut self, start: Point, end: Point) -> bool {
        if signex_net::pt_key(&start) == signex_net::pt_key(&end) {
            return false;
        }
        // Inside a route the typed wire takes the route's start style.
        let start_style = if self.interaction_state.wire_drawing {
            self.interaction_state.wire_start_style
        } else {
            self.wire_style_at(start)
        };
        let style = inherited_style(self.wire_placement_style(), start_style);
        let commands = wire_commands(&[(start, end)], style);
        self.apply_engine_commands(commands, false, false)
    }

    /// Width and colour of the wire at `at`: one ending there, else the one
    /// whose body it is on, picked as `signex_net::wire_at_point` picks the
    /// wire a label joins. `None` on no wire.
    pub(crate) fn wire_style_at(&self, at: Point) -> Option<WireStyle> {
        let sheet = self.document_state.edit_engine()?.document();
        let id = signex_net::wire_at_point(sheet, &at)?;
        let wire = sheet.wires.iter().find(|w| w.uuid == id)?;
        Some((wire.stroke_width, wire.color))
    }

    /// The width and colour the next wire is placed with: what the Tab form
    /// of the Wire tool holds, else the default width and no colour.
    pub(crate) fn wire_placement_style(&self) -> (f64, Option<signex_types::theme::Color>) {
        match &self.document_state.panel_ctx.pre_placement {
            Some(pp) if pp.kind == crate::panels::PrePlacementKind::Wire => {
                (pp.wire_width.stroke_mm(), pp.wire_color)
            }
            _ => (0.0, None),
        }
    }

    /// [`Self::place_wire_at`] for a bus.
    pub(crate) fn place_bus_at(
        &mut self,
        start: Point,
        end: Point,
        rule: CornerRule,
    ) -> (Point, bool) {
        let segments = rule.legs(start, end);
        let commands = bus_commands(&segments);
        let changed = !commands.is_empty() && self.apply_engine_commands(commands, false, false);
        (segments.last().map(|s| s.1).unwrap_or(end), changed)
    }

    /// Place a bus entry of `size` at `at`. Its bus end snaps onto the
    /// nearest bus within one grid step; off a bus it lands on `at`.
    pub(crate) fn place_bus_entry_at(&mut self, at: Point, size: (f64, f64)) -> bool {
        let position = self
            .document_state
            .edit_engine()
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
