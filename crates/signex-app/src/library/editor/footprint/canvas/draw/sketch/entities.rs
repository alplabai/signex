//! Sketch-entity overlay — points, lines, circles, and arcs drawn in
//! the DOF / selection palette, layered above the constraint glyphs and
//! the filled closed loops.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point};

use crate::library::editor::footprint::canvas::FootprintCanvasState;
use crate::library::editor::footprint::state::{EditorPad, FootprintEditorState};

use super::constraints::draw_constraint_icons;
use super::fills::draw_filled_closed_loops;

/// Render the sketch entities (Phase 6.2). Points draw as small
/// filled circles, Lines stroke between their endpoints (dashed if
/// `construction == true`), Circles stroke the radius circle, Arcs
/// stroke a polyline approximation between start/end. DOF colour
/// drives the tint.
pub(in crate::library::editor::footprint::canvas::draw) fn draw_sketch_overlay(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &signex_sketch::SketchData,
    state: &FootprintEditorState,
) {
    use signex_sketch::entity::EntityKind;
    use signex_sketch::id::SketchEntityId;

    fn point_world(
        id: SketchEntityId,
        sketch: &signex_sketch::SketchData,
        state: &FootprintEditorState,
    ) -> Option<(f64, f64)> {
        // Prefer the solved state if available; fall back to the
        // entity's authored coords.
        if let Some(solve) = state.last_solve.as_ref() {
            if let Some((x, y)) = signex_sketch::solver::state::point_xy(
                id,
                &solve.result.state,
                &solve.result.index,
                sketch,
            ) {
                return Some((x, y));
            }
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

    // v0.27 — Fusion-style DOF palette tuned for the white sketch
    // canvas. Under-constrained → blue, fully-constrained → black,
    // over-constrained → red, no-solve → dark grey. The pre-v0.27
    // light-grey "no solve" colour was effectively invisible
    // against white; dark grey reads at the same weight as the
    // black fully-constrained state without competing with it.
    let dof_colour = |id: SketchEntityId| -> Color {
        use signex_sketch::solver::dof::DofColor;
        if let Some(solve) = state.last_solve.as_ref() {
            match solve.colours.get(&id) {
                Some(DofColor::Under) => Color::from_rgba(0.10, 0.30, 0.85, 1.00),
                Some(DofColor::Over) => Color::from_rgba(0.85, 0.10, 0.10, 1.00),
                Some(DofColor::Full) => Color::from_rgba(0.0, 0.0, 0.0, 1.00),
                None => Color::from_rgba(0.40, 0.40, 0.40, 1.00),
            }
        } else {
            Color::from_rgba(0.40, 0.40, 0.40, 1.00)
        }
    };

    // v0.27 — Fusion-style selection highlight. Entities in the
    // primary / secondary / extras selection sets render with a
    // saturated orange instead of the DOF palette, so the user
    // can see which entities the rubber-band picked. Mirrors the
    // colour Altium / Fusion use for selected sketch geometry.
    let selection_colour = Color::from_rgba(1.00, 0.45, 0.05, 1.00);
    let is_selected = |id: SketchEntityId| -> bool {
        state.selected_sketch == Some(id)
            || state.selected_sketch_secondary == Some(id)
            || state.selected_sketch_extra.contains(&id)
    };

    // v0.13.2 Phase 6.6 — Constraint icon overlay. Render BEFORE
    // entities so glyphs sit underneath the geometry layer and don't
    // hide pad-edge clicks. Tinted red for over-constrained
    // constraints; muted otherwise.
    draw_constraint_icons(frame, cstate, sketch, state);

    // v0.16.1 — Filled rendering for closed loops. Walks the line
    // graph, finds simple cycles whose Lines are NOT all
    // construction-flagged, and fills the polygon with a faint
    // role-tinted fill. Pad-corner outlines (whose Lines are all
    // construction-flagged) are skipped so they don't double-fill
    // over the rendered pads. Role-attr-driven layer tinting comes
    // with the role-assignment UI; for now everything assigns to a
    // neutral grey at low opacity.
    draw_filled_closed_loops(frame, cstate, sketch, state);

    for entity in &sketch.entities {
        match entity.kind {
            EntityKind::Point { .. } => {
                let world = match point_world(entity.id, sketch, state) {
                    Some(w) => w,
                    None => continue,
                };
                let p = cstate.world_to_screen(world);
                // v0.23 — Bumped Point handle sizes so corner/edge
                // grab targets read from a normal viewing distance.
                // Construction (bake-skipped) Points stay smaller
                // than authored Points so they read as secondary
                // chrome, but both are now grab-friendly.
                let mut r = if entity.bake_skipped() { 4.0 } else { 5.5 };
                if is_selected(entity.id) {
                    r += 1.5;
                }
                let path = Path::circle(Point::new(p.x, p.y), r);
                let col = if is_selected(entity.id) {
                    selection_colour
                } else {
                    dof_colour(entity.id)
                };
                frame.fill(&path, col);
                frame.stroke(
                    &path,
                    Stroke::default().with_width(1.5).with_color(Color {
                        a: 1.0,
                        r: col.r * 0.6,
                        g: col.g * 0.6,
                        b: col.b * 0.6,
                    }),
                );
            }
            EntityKind::Line { start, end } => {
                let s = match point_world(start, sketch, state) {
                    Some(w) => w,
                    None => continue,
                };
                let e = match point_world(end, sketch, state) {
                    Some(w) => w,
                    None => continue,
                };
                let p0 = cstate.world_to_screen(s);
                let p1 = cstate.world_to_screen(e);
                // v0.22 Phase A5 — Centerline lines render in Altium /
                // Fusion gold (#c9a04b) regardless of DOF colour, so
                // axis / mirror lines stay visually distinct from
                // construction scaffolding.
                let col = if is_selected(entity.id) {
                    selection_colour
                } else if entity.centerline {
                    Color::from_rgba(0.79, 0.63, 0.30, 1.00)
                } else {
                    dof_colour(start)
                };
                let line_width = if is_selected(entity.id) { 2.5 } else { 1.5 };
                let stroke = Stroke::default().with_width(line_width).with_color(col);
                if entity.construction {
                    // Dashed line via short segments.
                    let dx = p1.x - p0.x;
                    let dy = p1.y - p0.y;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len > 0.0 {
                        let dash_len = 6.0_f32;
                        let n = (len / dash_len).floor() as i32;
                        for i in (0..n).step_by(2) {
                            let t0 = i as f32 / n as f32;
                            let t1 = ((i + 1) as f32 / n as f32).min(1.0);
                            let q0 = Point::new(p0.x + dx * t0, p0.y + dy * t0);
                            let q1 = Point::new(p0.x + dx * t1, p0.y + dy * t1);
                            frame.stroke(&Path::line(q0, q1), stroke);
                        }
                    }
                } else if entity.centerline {
                    // v0.22 Phase A5 — long-dash + dot pattern.
                    // Walk the line in screen-space cycles of
                    // [long-dash 12 px][gap 4][dot 1.5][gap 4]; ~21 px
                    // per cycle. Matches Altium's centerline glyph.
                    let dx = p1.x - p0.x;
                    let dy = p1.y - p0.y;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len > 0.5 {
                        let cycle = 21.5_f32;
                        let mut t = 0.0_f32;
                        while t < len {
                            let long_end = (t + 12.0).min(len);
                            let q0 = Point::new(p0.x + dx * (t / len), p0.y + dy * (t / len));
                            let q1 = Point::new(
                                p0.x + dx * (long_end / len),
                                p0.y + dy * (long_end / len),
                            );
                            frame.stroke(&Path::line(q0, q1), stroke);
                            let dot_start = t + 16.0;
                            let dot_end = (dot_start + 1.5).min(len);
                            if dot_start < len {
                                let q2 = Point::new(
                                    p0.x + dx * (dot_start / len),
                                    p0.y + dy * (dot_start / len),
                                );
                                let q3 = Point::new(
                                    p0.x + dx * (dot_end / len),
                                    p0.y + dy * (dot_end / len),
                                );
                                frame.stroke(&Path::line(q2, q3), stroke);
                            }
                            t += cycle;
                        }
                    }
                } else {
                    frame.stroke(&Path::line(p0, p1), stroke);
                }
            }
            EntityKind::Circle { center, radius } => {
                let c = match point_world(center, sketch, state) {
                    Some(w) => w,
                    None => continue,
                };
                let centre = cstate.world_to_screen(c);
                let r_screen = (radius as f32) * cstate.scale;
                let path = Path::circle(Point::new(centre.x, centre.y), r_screen);
                // v0.27 — Circle entity stroke. Pre-solve uses a
                // darker cyan that reads against white sketch
                // canvas; post-solve drops to the DOF palette so
                // the constraint state shows through.
                let dof = dof_colour(entity.id);
                let unsolved = state.last_solve.is_none();
                let selected = is_selected(entity.id);
                let col = if selected {
                    selection_colour
                } else if unsolved {
                    Color::from_rgba(0.10, 0.55, 0.85, 1.00)
                } else {
                    dof
                };
                let width = if selected {
                    2.5
                } else if unsolved {
                    2.0
                } else {
                    1.5
                };
                frame.stroke(&path, Stroke::default().with_width(width).with_color(col));
                // v0.27 — diameter handle Point on the east edge.
                // Filled cyan disc with a darker outline; the
                // outline picks up the DOF palette so a fully-
                // constrained Circle's handle is rimmed in black.
                let handle = Path::circle(Point::new(centre.x + r_screen, centre.y), 4.0);
                frame.fill(&handle, Color::from_rgba(0.20, 0.65, 0.95, 1.00));
                frame.stroke(
                    &handle,
                    Stroke::default().with_width(1.0).with_color(if unsolved {
                        Color::from_rgba(0.05, 0.30, 0.55, 1.0)
                    } else {
                        dof
                    }),
                );
            }
            EntityKind::Arc {
                center,
                start,
                end,
                sweep_ccw,
            } => {
                // Approximate the arc by a 16-segment polyline between
                // start and end on the circle through `center`. Sweep
                // direction respects the entity's `sweep_ccw` flag —
                // CCW arcs walk positive delta, CW arcs walk negative
                // delta.
                let c = match point_world(center, sketch, state) {
                    Some(w) => w,
                    None => continue,
                };
                let s = match point_world(start, sketch, state) {
                    Some(w) => w,
                    None => continue,
                };
                let e = match point_world(end, sketch, state) {
                    Some(w) => w,
                    None => continue,
                };
                let r = ((s.0 - c.0).powi(2) + (s.1 - c.1).powi(2)).sqrt();
                let a0 = (s.1 - c.1).atan2(s.0 - c.0);
                let a1 = (e.1 - c.1).atan2(e.0 - c.0);
                let mut delta = a1 - a0;
                let tau = std::f64::consts::TAU;
                if sweep_ccw {
                    while delta < 0.0 {
                        delta += tau;
                    }
                } else {
                    // Clockwise sweep — delta should be ≤ 0; wrap into
                    // (−2π, 0].
                    while delta > 0.0 {
                        delta -= tau;
                    }
                }
                let segs = 16;
                let mut prev = cstate.world_to_screen(s);
                let selected = is_selected(entity.id);
                let col = if selected {
                    selection_colour
                } else {
                    dof_colour(entity.id)
                };
                let arc_width = if selected { 2.5 } else { 1.5 };
                for i in 1..=segs {
                    let t = (i as f64) / (segs as f64);
                    let a = a0 + delta * t;
                    let p = (c.0 + r * a.cos(), c.1 + r * a.sin());
                    let q = cstate.world_to_screen(p);
                    frame.stroke(
                        &Path::line(prev, q),
                        Stroke::default().with_width(arc_width).with_color(col),
                    );
                    prev = q;
                }
            }
        }
    }
}
