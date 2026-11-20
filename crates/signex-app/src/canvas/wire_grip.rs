//! Grips on the ends of a selected wire. Dragging one moves just that end
//! through `signex_engine::Command::MoveWireEnd`: the wire gets longer or
//! shorter, the wires attached at that end follow, and the drop is one undo
//! step. The drag rides on the move-drag state (`move_origin` is the grabbed
//! end), and its ghost is the sheet the engine commits.

use super::*;
use signex_types::schematic::{Point as SheetPoint, SchematicSheet, SelectedKind};

/// How near (screen px) a press must land to a grip to take it.
const GRIP_HIT_PX: f32 = 6.0;
/// Half the side of a drawn grip, screen px.
const GRIP_HALF_PX: f32 = 3.5;
/// The grip's outline, screen px.
const GRIP_OUTLINE_PX: f32 = 1.0;

/// The end of a selected wire a press took hold of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireGrip {
    pub wire: uuid::Uuid,
    /// The wire's start, else its end.
    pub start: bool,
}

/// Where the end `grip` holds sits in `sheet`.
pub(in crate::canvas) fn grip_end(sheet: &SchematicSheet, grip: WireGrip) -> Option<SheetPoint> {
    let wire = sheet.wires.iter().find(|w| w.uuid == grip.wire)?;
    Some(if grip.start { wire.start } else { wire.end })
}

impl SchematicCanvas<'_> {
    /// The grip under the screen point `at`, if a selected, unlocked wire has
    /// an end there; the nearest one wins.
    pub(in crate::canvas) fn grip_at(
        &self,
        at: iced::Point,
        bounds: Rectangle,
    ) -> Option<WireGrip> {
        let sheet = self.active_snapshot()?;
        let cam = self.camera();
        let mut best: Option<(f32, WireGrip)> = None;
        for item in &self.selected {
            if item.kind != SelectedKind::Wire {
                continue;
            }
            let Some(wire) = sheet.wires.iter().find(|w| w.uuid == item.uuid) else {
                continue;
            };
            if wire.locked {
                continue;
            }
            for (start, end) in [(true, wire.start), (false, wire.end)] {
                let screen =
                    cam.world_to_screen(iced::Point::new(end.x as f32, end.y as f32), bounds);
                let distance = (screen.x - at.x).hypot(screen.y - at.y);
                if distance <= GRIP_HIT_PX && best.is_none_or(|(d, _)| distance < d) {
                    let grip = WireGrip {
                        wire: wire.uuid,
                        start,
                    };
                    best = Some((distance, grip));
                }
            }
        }
        best.map(|(_, grip)| grip)
    }

    /// The grip drag's delta: the cursor's grid steps from the press, then
    /// the grabbed end pulled onto a pin tip, wire end or dot in range.
    pub(in crate::canvas) fn grip_delta(
        &self,
        state: &CanvasState,
        grip: WireGrip,
    ) -> Option<(f64, f64)> {
        let ((ox, oy), (cx, cy)) = state.move_origin.zip(state.move_current)?;
        let delta = (cx - ox, cy - oy);
        if !self.prefs.snap_hotspots {
            return Some(delta);
        }
        let sheet = self.active_snapshot()?;
        let end = grip_end(sheet, grip)?;
        let grid = if self.prefs.snap_enabled {
            self.prefs.snap_grid_mm
        } else {
            0.0
        };
        let radius = signex_types::schematic::hotspot_capture_radius(grid);
        Some(signex_types::schematic::snap_move_to_hotspot(
            sheet,
            &[end],
            &[grip.wire],
            delta,
            radius,
        ))
    }

    /// Compute the grip drag's ghost for `delta`: the sheet the drop commits.
    pub(in crate::canvas) fn refresh_grip_ghost(
        &self,
        state: &mut CanvasState,
        grip: WireGrip,
        delta: (f64, f64),
    ) {
        let (Some(snap), Some(cache)) = (self.active_snapshot(), self.render_cache.as_ref()) else {
            return;
        };
        let Some(end) = grip_end(snap, grip) else {
            return;
        };
        let keep_wires = self.keeps_wires(state);
        let ghost_delta = self.ghost_delta(delta.0, delta.1);
        state.ghost.refresh_grip(
            snap,
            cache.revision(),
            &self.selected,
            (grip.wire, grip.start),
            end,
            ghost_delta,
            keep_wires,
        );
    }

    /// A small square on each end of every selected, unlocked wire.
    pub(in crate::canvas) fn draw_wire_grips(
        &self,
        frame: &mut canvas::Frame,
        snapshot: &SchematicSheet,
        bounds: Rectangle,
    ) {
        let cam = self.camera();
        let fill = crate::render_config::to_iced(&self.prefs.canvas_colors.selection);
        let outline = crate::render_config::to_iced(&self.prefs.canvas_colors.paper);
        for item in &self.selected {
            if item.kind != SelectedKind::Wire {
                continue;
            }
            let Some(wire) = snapshot.wires.iter().find(|w| w.uuid == item.uuid) else {
                continue;
            };
            if wire.locked {
                continue;
            }
            for end in [wire.start, wire.end] {
                let at = cam.world_to_screen(iced::Point::new(end.x as f32, end.y as f32), bounds);
                let square = canvas::Path::rectangle(
                    iced::Point::new(at.x - GRIP_HALF_PX, at.y - GRIP_HALF_PX),
                    iced::Size::new(2.0 * GRIP_HALF_PX, 2.0 * GRIP_HALF_PX),
                );
                frame.fill(&square, fill);
                frame.stroke(
                    &square,
                    canvas::Stroke::default()
                        .with_color(outline)
                        .with_width(GRIP_OUTLINE_PX),
                );
            }
        }
    }
}

impl CanvasState {
    /// The grip the drag in flight holds; never during a click-click float.
    pub(in crate::canvas) fn held_grip(&self) -> Option<WireGrip> {
        self.wire_grip.filter(|_| self.move_float.is_none())
    }
}
