//! The driving-profile highlight of an Edit in Sketch from a pad (mockup row 192):
//! the pad's entity and its profile in the selection colour, with a soft
//! halo so it reads over the selection stroke itself.

use iced::Color;
use iced::widget::canvas::{self, Path, Stroke};
use signex_sketch::entity::EntityKind;
use signex_sketch::id::SketchEntityId;

use crate::library::editor::footprint::canvas::FootprintCanvasState;
use crate::library::editor::footprint::state::FootprintEditorState;
use crate::library::editor::footprint::state::pad_link::driving_profile;

use super::SELECTION;

const HALO: Color = Color {
    a: 0.30,
    ..SELECTION
};
const STROKE_PX: f32 = 2.5;
const HALO_PX: f32 = 7.0;
const POINT_RING_PX: f32 = 6.0;

pub(in crate::library::editor::footprint::canvas::draw) fn draw_pad_link_profile(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &signex_sketch::SketchData,
    state: &FootprintEditorState,
) {
    let Some(focus) = state.live_pad_link() else {
        return;
    };
    let corners = state
        .pads
        .get(focus.pad_idx)
        .and_then(|pad| pad.corner_entity_ids);
    let pos = |id: SketchEntityId| -> Option<(f64, f64)> {
        if let Some(solve) = state.last_solve.as_ref()
            && let Some(p) = signex_sketch::solver::state::point_xy(
                id,
                &solve.result.state,
                &solve.result.index,
                sketch,
            )
        {
            return Some(p);
        }
        sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                EntityKind::Point { x, y } => Some((x, y)),
                _ => None,
            })
    };
    let paint = |frame: &mut canvas::Frame, path: &Path| {
        frame.stroke(path, Stroke::default().with_width(HALO_PX).with_color(HALO));
        frame.stroke(
            path,
            Stroke::default()
                .with_width(STROKE_PX)
                .with_color(SELECTION),
        );
    };
    for id in driving_profile(sketch, focus.entity, corners) {
        let Some(entity) = sketch.entities.iter().find(|e| e.id == id) else {
            continue;
        };
        match entity.kind {
            EntityKind::Point { .. } => {
                if let Some(p) = pos(id) {
                    paint(
                        frame,
                        &Path::circle(cstate.world_to_screen(p), POINT_RING_PX),
                    );
                }
            }
            EntityKind::Line { start, end } => {
                if let (Some(a), Some(b)) = (pos(start), pos(end)) {
                    paint(
                        frame,
                        &Path::line(cstate.world_to_screen(a), cstate.world_to_screen(b)),
                    );
                }
            }
            EntityKind::Circle { center, radius } => {
                if let Some(c) = pos(center) {
                    paint(
                        frame,
                        &Path::circle(cstate.world_to_screen(c), radius as f32 * cstate.scale),
                    );
                }
            }
            EntityKind::Arc {
                center,
                start,
                end,
                sweep_ccw,
            } => {
                if let (Some(c), Some(s), Some(e)) = (pos(center), pos(start), pos(end)) {
                    paint(frame, &arc_path(cstate, c, s, e, sweep_ccw));
                }
            }
        }
    }
}

/// An arc from `s` to `e` about `c` as a 16-segment polyline, the way the
/// entity overlay draws arcs.
fn arc_path(
    cstate: &FootprintCanvasState,
    c: (f64, f64),
    s: (f64, f64),
    e: (f64, f64),
    sweep_ccw: bool,
) -> Path {
    use std::f64::consts::TAU;
    let r = ((s.0 - c.0).powi(2) + (s.1 - c.1).powi(2)).sqrt();
    let a0 = (s.1 - c.1).atan2(s.0 - c.0);
    let mut delta = (e.1 - c.1).atan2(e.0 - c.0) - a0;
    if sweep_ccw {
        delta = delta.rem_euclid(TAU);
    } else {
        delta = -(-delta).rem_euclid(TAU);
    }
    const SEGMENTS: u32 = 16;
    Path::new(|builder| {
        builder.move_to(cstate.world_to_screen(s));
        for i in 1..=SEGMENTS {
            let a = a0 + delta * f64::from(i) / f64::from(SEGMENTS);
            builder.line_to(cstate.world_to_screen((c.0 + r * a.cos(), c.1 + r * a.sin())));
        }
    })
}
