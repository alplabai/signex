//! The driving-profile highlight of the pad link (mockup row 192): after an
//! Edit in Sketch, or with a pad's entity selected, the entity, its profile
//! and the outline of the pad it drives in the selection colour, with a
//! soft halo so it reads over the selection stroke itself; the pattern
//! instance the focus came from is outlined dashed.

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
/// Dash of the outlined pattern instance, in px.
const INSTANCE_DASH: [f32; 2] = [5.0, 3.0];

pub(in crate::library::editor::footprint::canvas::draw) fn draw_pad_link_profile(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &signex_sketch::SketchData,
    state: &FootprintEditorState,
) {
    let Some((entity, outlines)) = state.pad_link_highlight() else {
        return;
    };
    // The profile is the source's, also when the focus is one of its instances.
    let corners = state
        .pad_driven_by(entity)
        .and_then(|idx| state.pads.get(idx))
        .and_then(|pad| pad.corner_entity_ids);
    for outline in &outlines {
        let path = outline_path(cstate, outline.corners);
        if outline.is_instance {
            stroke_instance(frame, &path);
        } else {
            paint(frame, &path);
        }
    }
    let pos = |id: SketchEntityId| point_xy(state, sketch, id);
    for id in driving_profile(sketch, entity, corners) {
        let path = sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| entity_path(cstate, &e.kind, id, pos));
        if let Some(path) = path {
            paint(frame, &path);
        }
    }
}

/// The halo and selection stroke of a highlighted path.
fn paint(frame: &mut canvas::Frame, path: &Path) {
    frame.stroke(path, Stroke::default().with_width(HALO_PX).with_color(HALO));
    frame.stroke(
        path,
        Stroke::default()
            .with_width(STROKE_PX)
            .with_color(SELECTION),
    );
}

/// Where point `id` lies: from the last solve, else its stored place.
fn point_xy(
    state: &FootprintEditorState,
    sketch: &signex_sketch::SketchData,
    id: SketchEntityId,
) -> Option<(f64, f64)> {
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
}

/// The screen path of profile entity `id` of `kind`; `None` while one of
/// its points has no place.
fn entity_path(
    cstate: &FootprintCanvasState,
    kind: &EntityKind,
    id: SketchEntityId,
    pos: impl Fn(SketchEntityId) -> Option<(f64, f64)>,
) -> Option<Path> {
    let screen = |id| pos(id).map(|p| cstate.world_to_screen(p));
    match *kind {
        EntityKind::Point { .. } => Some(Path::circle(screen(id)?, POINT_RING_PX)),
        EntityKind::Line { start, end } => Some(Path::line(screen(start)?, screen(end)?)),
        EntityKind::Circle { center, radius } => {
            Some(Path::circle(screen(center)?, radius as f32 * cstate.scale))
        }
        EntityKind::Arc {
            center,
            start,
            end,
            sweep_ccw,
        } => Some(arc_path(
            cstate,
            pos(center)?,
            pos(start)?,
            pos(end)?,
            sweep_ccw,
        )),
    }
}

/// A pad outline through its four corners, in screen space.
fn outline_path(cstate: &FootprintCanvasState, corners: [(f64, f64); 4]) -> Path {
    Path::new(|builder| {
        builder.move_to(cstate.world_to_screen(corners[0]));
        for corner in &corners[1..] {
            builder.line_to(cstate.world_to_screen(*corner));
        }
        builder.close();
    })
}

/// The pattern instance the focus came from, outlined dashed (mockup row 192).
fn stroke_instance(frame: &mut canvas::Frame, outline: &Path) {
    frame.stroke(
        outline,
        Stroke {
            line_dash: canvas::LineDash {
                segments: &INSTANCE_DASH,
                offset: 0,
            },
            ..Stroke::default()
                .with_width(STROKE_PX)
                .with_color(SELECTION)
        },
    );
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
