//! Sketch-mode rendering — entity overlay, DOF arrows, snap glyph,
//! constraint icons, filled closed loops, and the live ghost preview
//! for multi-click drawing tools.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point};

use super::super::layers::FpLayer;
use super::super::snap::{self, SnapKind, SnapResult};
use super::super::state::{EditorPad, FootprintEditorState};
use super::FootprintCanvasState;


/// v0.13.2 Phase 6.6 — render constraint glyphs above the sketch
/// entities. Each constraint's centroid (geometric mean of the
/// entities it touches) gets a small Unicode glyph; over-constrained
/// constraints render in red.
pub(super) fn draw_constraint_icons(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &signex_sketch::SketchData,
    state: &FootprintEditorState,
) {
    use signex_sketch::constraint::ConstraintKind;
    use signex_sketch::entity::EntityKind;
    use signex_sketch::id::{ConstraintId, SketchEntityId};

    let over_set: std::collections::HashSet<ConstraintId> = state
        .last_solve
        .as_ref()
        .map(|s| s.over_constraints.iter().copied().collect())
        .unwrap_or_default();

    let point_world_local = |id: SketchEntityId| -> Option<(f64, f64)> {
        if let Some(solve) = state.last_solve.as_ref() {
            if let Some(p) = signex_sketch::solver::state::point_xy(
                id,
                &solve.result.state,
                &solve.result.index,
                sketch,
            ) {
                return Some(p);
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
    };
    let line_endpoints_local = |id: SketchEntityId| -> Option<(SketchEntityId, SketchEntityId)> {
        sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                EntityKind::Line { start, end } => Some((start, end)),
                _ => None,
            })
    };
    fn arc_refs_local(
        sketch: &signex_sketch::SketchData,
        id: signex_sketch::id::SketchEntityId,
    ) -> Option<(
        signex_sketch::id::SketchEntityId,
        signex_sketch::id::SketchEntityId,
        signex_sketch::id::SketchEntityId,
        bool,
    )> {
        sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                signex_sketch::entity::EntityKind::Arc {
                    center,
                    start,
                    end,
                    sweep_ccw,
                } => Some((center, start, end, sweep_ccw)),
                _ => None,
            })
    }
    fn circle_center_local(
        sketch: &signex_sketch::SketchData,
        id: signex_sketch::id::SketchEntityId,
    ) -> Option<signex_sketch::id::SketchEntityId> {
        sketch
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| match e.kind {
                signex_sketch::entity::EntityKind::Circle { center, .. } => Some(center),
                _ => None,
            })
    }

    for c in &sketch.constraints {
        let (glyph, points): (&str, Vec<SketchEntityId>) = match &c.kind {
            ConstraintKind::Coincident { p1, p2 } => ("=", vec![*p1, *p2]),
            ConstraintKind::PointOnLine { point, line } => {
                let mut v = vec![*point];
                if let Some((s, e)) = line_endpoints_local(*line) {
                    v.push(s);
                    v.push(e);
                }
                ("|", v)
            }
            ConstraintKind::Horizontal { line } => {
                let mut v = Vec::new();
                if let Some((s, e)) = line_endpoints_local(*line) {
                    v.push(s);
                    v.push(e);
                }
                ("H", v)
            }
            ConstraintKind::Vertical { line } => {
                let mut v = Vec::new();
                if let Some((s, e)) = line_endpoints_local(*line) {
                    v.push(s);
                    v.push(e);
                }
                ("V", v)
            }
            ConstraintKind::Parallel { l1, l2 } => {
                let mut v = Vec::new();
                if let Some((s, e)) = line_endpoints_local(*l1) {
                    v.extend([s, e]);
                }
                if let Some((s, e)) = line_endpoints_local(*l2) {
                    v.extend([s, e]);
                }
                ("//", v)
            }
            ConstraintKind::Perpendicular { l1, l2 } => {
                let mut v = Vec::new();
                if let Some((s, e)) = line_endpoints_local(*l1) {
                    v.extend([s, e]);
                }
                if let Some((s, e)) = line_endpoints_local(*l2) {
                    v.extend([s, e]);
                }
                ("L", v)
            }
            ConstraintKind::DistancePtPt { p1, p2, .. } => ("D", vec![*p1, *p2]),
            ConstraintKind::DistancePtLine { point, line, .. } => {
                let mut v = vec![*point];
                if let Some((s, e)) = line_endpoints_local(*line) {
                    v.push(s);
                    v.push(e);
                }
                ("d", v)
            }
            ConstraintKind::DistancePtCircle { point, circle, .. } => {
                let mut v = vec![*point];
                if let Some(c) = circle_center_local(sketch, *circle) {
                    v.push(c);
                } else if let Some((c, _, _, _)) = arc_refs_local(sketch, *circle) {
                    v.push(c);
                }
                ("\u{29bf}", v) // ⦿ "DistancePtCircle"
            }
            ConstraintKind::Fixed { point } => ("\u{1F512}", vec![*point]),
            // v0.13.3 — remaining constraint glyphs.
            ConstraintKind::PointOnArc { point, arc } => {
                let mut v = vec![*point];
                if let Some((c, s, e, _)) = arc_refs_local(sketch, *arc) {
                    v.extend([c, s, e]);
                }
                ("\u{2192}", v) // → "PointOnArc"
            }
            ConstraintKind::Angle { l1, l2, .. } => {
                let mut v = Vec::new();
                if let Some((s, e)) = line_endpoints_local(*l1) {
                    v.extend([s, e]);
                }
                if let Some((s, e)) = line_endpoints_local(*l2) {
                    v.extend([s, e]);
                }
                ("A", v)
            }
            ConstraintKind::EqualLength { l1, l2 } => {
                let mut v = Vec::new();
                if let Some((s, e)) = line_endpoints_local(*l1) {
                    v.extend([s, e]);
                }
                if let Some((s, e)) = line_endpoints_local(*l2) {
                    v.extend([s, e]);
                }
                ("=L", v)
            }
            ConstraintKind::EqualRadius { e1, e2 } => {
                let mut v = Vec::new();
                if let Some((c, _, _, _)) = arc_refs_local(sketch, *e1) {
                    v.push(c);
                }
                if let Some((c, _, _, _)) = arc_refs_local(sketch, *e2) {
                    v.push(c);
                }
                if v.is_empty() {
                    if let Some(c) = circle_center_local(sketch, *e1) {
                        v.push(c);
                    }
                    if let Some(c) = circle_center_local(sketch, *e2) {
                        v.push(c);
                    }
                }
                ("=R", v)
            }
            ConstraintKind::TangentLineArc { line, arc } => {
                let mut v = Vec::new();
                if let Some((s, e)) = line_endpoints_local(*line) {
                    v.extend([s, e]);
                }
                if let Some((c, _, _, _)) = arc_refs_local(sketch, *arc) {
                    v.push(c);
                }
                ("T", v)
            }
            ConstraintKind::TangentArcArc { a1, a2, .. } => {
                let mut v = Vec::new();
                if let Some((c, _, _, _)) = arc_refs_local(sketch, *a1) {
                    v.push(c);
                }
                if let Some((c, _, _, _)) = arc_refs_local(sketch, *a2) {
                    v.push(c);
                }
                ("TT", v)
            }
            ConstraintKind::SymmetricAboutLine { p1, p2, line } => {
                let mut v = vec![*p1, *p2];
                if let Some((s, e)) = line_endpoints_local(*line) {
                    v.extend([s, e]);
                }
                ("\u{29C7}", v) // ⧇ "Symmetric"
            }
            ConstraintKind::SymmetricAboutPoint { p1, p2, center } => {
                ("\u{29C7}", vec![*p1, *p2, *center])
            }
            ConstraintKind::Midpoint { point, line } => {
                let mut v = vec![*point];
                if let Some((s, e)) = line_endpoints_local(*line) {
                    v.extend([s, e]);
                }
                ("M", v)
            }
        };
        if glyph.is_empty() || points.is_empty() {
            continue;
        }
        let mut sum_x = 0.0_f64;
        let mut sum_y = 0.0_f64;
        let mut n = 0;
        for id in &points {
            if let Some((x, y)) = point_world_local(*id) {
                sum_x += x;
                sum_y += y;
                n += 1;
            }
        }
        if n == 0 {
            continue;
        }
        let centroid = (sum_x / n as f64, sum_y / n as f64);
        let p = cstate.world_to_screen(centroid);
        // v0.23 — per-row precision in the Conflicts list. When the
        // user hovers a specific row, only that constraint renders
        // at full red; every other glyph (including other
        // over-constraints) dims so the offender stands out alone.
        // When no row is hovered, fall back to the v0.22 set-wide
        // isolation (the whole over-constraint set lights up).
        let hover = state.conflicts_row_hovered;
        let is_over = over_set.contains(&c.id);
        let colour = match (hover, is_over) {
            // Specific row hovered + this is the row → full red.
            (Some(h), _) if h == c.id => Color::from_rgba(0.85, 0.10, 0.10, 1.00),
            // Specific row hovered + this is NOT the row → dimmed.
            (Some(_), _) => Color::from_rgba(0.40, 0.40, 0.40, 0.30),
            // No row hover + over-constrained → red (set-wide focus).
            (None, true) => Color::from_rgba(0.85, 0.10, 0.10, 1.00),
            // v0.27 — non-over-constrained constraint glyph at dark
            // grey so it reads against both the dark Pads-mode
            // canvas and the white Sketch-mode canvas without
            // washing out on either.
            (None, false) => Color::from_rgba(0.30, 0.30, 0.30, 0.95),
        };
        frame.fill_text(canvas::Text {
            content: glyph.to_string(),
            position: Point::new(p.x + 6.0, p.y - 6.0),
            color: colour,
            size: iced::Pixels(11.0),
            ..canvas::Text::default()
        });
    }
}


/// Render the sketch entities (Phase 6.2). Points draw as small
/// filled circles, Lines stroke between their endpoints (dashed if
/// `construction == true`), Circles stroke the radius circle, Arcs
/// stroke a polyline approximation between start/end. DOF colour
/// drives the tint.
pub(super) fn draw_sketch_overlay(
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
                            let q0 =
                                Point::new(p0.x + dx * (t / len), p0.y + dy * (t / len));
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
                let handle =
                    Path::circle(Point::new(centre.x + r_screen, centre.y), 4.0);
                frame.fill(
                    &handle,
                    Color::from_rgba(0.20, 0.65, 0.95, 1.00),
                );
                frame.stroke(
                    &handle,
                    Stroke::default()
                        .with_width(1.0)
                        .with_color(if unsolved {
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


/// v0.22 Phase E2 — DOF direction-arrow overlay for under-constrained
/// Points. For every Point with `DofColor::Under`, draws a 10-px-long
/// 1-px-wide cyan arrow pointing in the direction of least constraint
/// sensitivity — i.e. the direction in which moving the Point
/// increases the constraint residual the least. Visually answers the
/// "if I drag this blue Point, which way will it go freely?"
/// question Fusion users expect.
///
/// Math: for a Point with Jacobian columns `c_x`, `c_y` (each column
/// is the partial derivative of every residual w.r.t. that state
/// var), the direction of greatest constraint sensitivity is the
/// eigenvector of
///   `M = [[||c_x||², c_x·c_y], [c_x·c_y, ||c_y||²]]`
/// associated with the LARGEST eigenvalue. The free-DoF direction is
/// the perpendicular (smallest-eigenvalue eigenvector).
///
/// Closed-form for a 2×2 symmetric matrix:
/// - λ_min = (a+d)/2 − √(((a-d)/2)² + b²)
/// - eigenvector for λ_min:
///     - if |b| > ε: (b, λ_min − a), normalized
///     - else (already diagonal): pick whichever column is smaller
/// - if all of a, b, d ≈ 0 (Point isn't touched by any constraint):
///   default to (1, 0) so the arrow still gives visual feedback.
///
/// Hides itself entirely when `state.last_solve` is `None` or the
/// jacobian is empty.
pub(super) fn draw_dof_direction_arrows(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &signex_sketch::SketchData,
    state: &FootprintEditorState,
) {
    use signex_sketch::entity::EntityKind;
    use signex_sketch::solver::dof::DofColor;

    let solve = match state.last_solve.as_ref() {
        Some(s) => s,
        None => return,
    };
    if solve.jacobian.is_empty() {
        // No constraints yet — would draw an arrow on every Point.
        // Silent skip; the user reads "no constraints" from the DOF
        // counter in the inspector.
        return;
    }

    const ARROW_LEN_PX: f32 = 10.0;
    const HEAD_LEN_PX: f32 = 3.0;
    const HEAD_SPREAD_RAD: f64 = 0.5; // ~28°
    // v0.27 — DOF arrow shifted to a darker cyan so it reads
    // against the white sketch canvas without competing with the
    // blue under-constrained DOF dot underneath.
    let cyan = Color::from_rgba(0.05, 0.55, 0.80, 1.00);
    let stroke = Stroke::default().with_width(1.0).with_color(cyan);

    let m_rows = solve.jacobian.len();

    for entity in &sketch.entities {
        let pt_id = match entity.kind {
            EntityKind::Point { .. } => entity.id,
            _ => continue,
        };
        if !matches!(solve.colours.get(&pt_id), Some(DofColor::Under)) {
            continue;
        }
        let (xi, yi) = match solve.result.index.points.get(&pt_id) {
            Some(t) => *t,
            None => continue, // Fixed Point — has no state column.
        };
        // Compute a, d, b from columns xi, yi.
        let mut a = 0.0_f64;
        let mut d = 0.0_f64;
        let mut b = 0.0_f64;
        for r in 0..m_rows {
            let row = &solve.jacobian[r];
            if xi >= row.len() || yi >= row.len() {
                continue;
            }
            let cx = row[xi];
            let cy = row[yi];
            a += cx * cx;
            d += cy * cy;
            b += cx * cy;
        }
        let (mut dirx, mut diry) = if a.abs() < 1e-12 && d.abs() < 1e-12 && b.abs() < 1e-12
        {
            (1.0, 0.0)
        } else {
            let half = (a + d) * 0.5;
            let radicand = ((a - d) * 0.5).powi(2) + b * b;
            let lam_min = half - radicand.sqrt();
            if b.abs() > 1e-12 {
                (b, lam_min - a)
            } else if a <= d {
                (1.0, 0.0)
            } else {
                (0.0, 1.0)
            }
        };
        let mag = (dirx * dirx + diry * diry).sqrt();
        if mag < 1e-12 {
            dirx = 1.0;
            diry = 0.0;
        } else {
            dirx /= mag;
            diry /= mag;
        }

        // Resolve world position via the solved state (preferring) or
        // the authored entity coords.
        let world = if let Some(p) = signex_sketch::solver::state::point_xy(
            pt_id,
            &solve.result.state,
            &solve.result.index,
            sketch,
        ) {
            p
        } else {
            match entity.kind {
                EntityKind::Point { x, y } => (x, y),
                _ => continue,
            }
        };
        let p_screen = cstate.world_to_screen(world);

        // Screen-space arrow. Y is flipped on screen so we negate
        // diry to match the world convention (positive y is up in
        // world but down in screen).
        let dx_s = dirx as f32 * ARROW_LEN_PX;
        let dy_s = -(diry as f32) * ARROW_LEN_PX;
        let tip = Point::new(p_screen.x + dx_s, p_screen.y + dy_s);
        let shaft = Path::line(p_screen, tip);
        frame.stroke(&shaft, stroke);

        // Arrow head: two short strokes at ±HEAD_SPREAD_RAD from the
        // shaft direction.
        let dir_angle = (dy_s as f64).atan2(dx_s as f64);
        for sign in [-1.0_f64, 1.0_f64] {
            let a = dir_angle + std::f64::consts::PI - sign * HEAD_SPREAD_RAD;
            let head_end = Point::new(
                tip.x + (a.cos() as f32) * HEAD_LEN_PX,
                tip.y + (a.sin() as f32) * HEAD_LEN_PX,
            );
            frame.stroke(&Path::line(tip, head_end), stroke);
        }
    }
}


/// v0.22 Phase A6 — Inferred-constraint snap glyph at the cursor.
/// Rendered AFTER the entity overlay so the badge sits on top of the
/// underlying geometry. Drives off `cstate.last_snap` which the
/// cursor-moved handler refreshes via `snap::snap_cursor`. Visible
/// only while a placement tool is active — Select doesn't draw a
/// hint because no entity is about to land. Glyphs:
/// - `●` (filled circle in cyan) — `SnapKind::Point` — auto-Coincident
///   target; clicking lands a new Point coincident with this one.
/// - `─` (horizontal cyan bar) — `SnapKind::Horizontal` — auto-H
///   constraint will land on the new Line.
/// - `│` (vertical cyan bar) — `SnapKind::Vertical` — auto-V
///   constraint will land on the new Line.
/// - `◇` (cyan diamond) — `SnapKind::Angle` — angle-snapped to the
///   nearest 15° increment.
/// - Guide / Grid / Raw — silent (Guide already paints its line;
///   Grid + Raw aren't actionable hints).
pub(super) fn draw_sketch_snap_glyph(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    state: &FootprintEditorState,
) {
    use super::super::state::SketchTool;

    if matches!(state.active_tool, SketchTool::Select) {
        return;
    }
    let snap = match cstate.last_snap {
        Some(s) => s,
        None => return,
    };
    let p = cstate.world_to_screen(snap.pos);
    // v0.27 — slightly darkened cyan so the snap badge reads
    // against both the dark Pads-mode canvas and the white
    // Sketch-mode canvas without competing.
    let c = Color::from_rgba(0.10, 0.60, 0.90, 1.00);
    let fill = Color { a: 0.30, ..c };
    let stroke = Stroke::default().with_width(1.5).with_color(c);

    match snap.kind {
        SnapKind::Point(_) => {
            let path = Path::circle(Point::new(p.x, p.y), 7.0);
            frame.fill(&path, fill);
            frame.stroke(&path, stroke);
        }
        SnapKind::Horizontal => {
            frame.stroke(
                &Path::line(
                    Point::new(p.x - 10.0, p.y),
                    Point::new(p.x + 10.0, p.y),
                ),
                stroke,
            );
        }
        SnapKind::Vertical => {
            frame.stroke(
                &Path::line(
                    Point::new(p.x, p.y - 10.0),
                    Point::new(p.x, p.y + 10.0),
                ),
                stroke,
            );
        }
        SnapKind::Angle(_) => {
            let r = 6.0;
            frame.stroke(
                &Path::line(Point::new(p.x, p.y - r), Point::new(p.x + r, p.y)),
                stroke,
            );
            frame.stroke(
                &Path::line(Point::new(p.x + r, p.y), Point::new(p.x, p.y + r)),
                stroke,
            );
            frame.stroke(
                &Path::line(Point::new(p.x, p.y + r), Point::new(p.x - r, p.y)),
                stroke,
            );
            frame.stroke(
                &Path::line(Point::new(p.x - r, p.y), Point::new(p.x, p.y - r)),
                stroke,
            );
        }
        SnapKind::Intersection => {
            // v0.27 — small "+" badge marks an intersection snap.
            let r = 6.0;
            frame.stroke(
                &Path::line(Point::new(p.x - r, p.y), Point::new(p.x + r, p.y)),
                stroke,
            );
            frame.stroke(
                &Path::line(Point::new(p.x, p.y - r), Point::new(p.x, p.y + r)),
                stroke,
            );
            let path = Path::circle(Point::new(p.x, p.y), 2.5);
            frame.fill(&path, c);
        }
        SnapKind::Guide | SnapKind::Grid | SnapKind::Raw => {}
    }
}


/// v0.16.1 — Walk the sketch's line graph, find simple closed
/// cycles, and render each as a filled polygon. Skips cycles where
/// every Line is `construction = true` (those are pad-corner
/// outlines or user-authored guides — already rendered as dashed
/// strokes elsewhere; double-filling would obscure the rendered
/// pad). Arc-bounded loops are deferred to v0.16.2.
///
/// v0.16.2 — Looks up the role attr on every entity in the loop.
/// The first hit picks the fill colour from the matching layer in
/// [`super::layers::FpLayer`]. Loops with no role assignment fall
/// back to neutral grey.
/// v0.27 — closed-loop record exposed to the click handler so a
/// single click on the polygon fill can select every entity in the
/// loop. Mirrors what `draw_filled_closed_loops` walks internally.
pub(super) struct ClosedLoop {
    pub lines: Vec<signex_sketch::id::SketchEntityId>,
    pub points: Vec<signex_sketch::id::SketchEntityId>,
    /// Vertex array shaped as `[[x, y]; n]` for direct hand-off to
    /// `super::geometry::point_in_polygon`.
    pub polygon: Vec<[f64; 2]>,
}

/// v0.27 — find every closed loop in the sketch. Same adjacency
/// walk as the fill renderer; centralised so the click handler can
/// reuse it. Skips loops where every line is bake-skipped (purely
/// construction loops); those are visible only as dashed strokes
/// and selecting them via fill would surprise the user.
pub(super) fn find_closed_loops(
    sketch: &signex_sketch::SketchData,
    state: &FootprintEditorState,
) -> Vec<ClosedLoop> {
    use signex_sketch::entity::EntityKind;
    use signex_sketch::id::SketchEntityId;
    use std::collections::{HashMap, HashSet};

    fn pos(
        id: SketchEntityId,
        sketch: &signex_sketch::SketchData,
        state: &FootprintEditorState,
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

    // v0.27 — adjacency now includes Arc edges as well as Lines so
    // the rounded-rectangle (line + arc + line + arc + ...) walks
    // as a single closed loop. The fill renderer was missing
    // rounded shapes for the same reason; extending the walker
    // here keeps both in sync.
    let mut adj: HashMap<SketchEntityId, Vec<(SketchEntityId, SketchEntityId, bool)>> =
        HashMap::new();
    for e in &sketch.entities {
        match e.kind {
            EntityKind::Line { start, end } => {
                adj.entry(start).or_default().push((end, e.id, e.bake_skipped()));
                adj.entry(end).or_default().push((start, e.id, e.bake_skipped()));
            }
            EntityKind::Arc { start, end, .. } => {
                adj.entry(start).or_default().push((end, e.id, e.bake_skipped()));
                adj.entry(end).or_default().push((start, e.id, e.bake_skipped()));
            }
            _ => {}
        }
    }
    let mut visited: HashSet<SketchEntityId> = HashSet::new();
    let mut out: Vec<ClosedLoop> = Vec::new();
    for seed in &sketch.entities {
        let (s_a, s_b) = match seed.kind {
            EntityKind::Line { start, end } => (start, end),
            EntityKind::Arc { start, end, .. } => (start, end),
            _ => continue,
        };
        if visited.contains(&seed.id) {
            continue;
        }
        let mut points = vec![s_a];
        let mut lines = vec![seed.id];
        let mut all_skipped = seed.bake_skipped();
        let mut current = s_b;
        let mut prev_line = seed.id;
        let mut closed = false;
        for _ in 0..256 {
            if current == s_a {
                closed = true;
                break;
            }
            let neigh = match adj.get(&current) {
                Some(n) if n.len() == 2 => n,
                _ => break,
            };
            match neigh.iter().find(|(_, lid, _)| *lid != prev_line) {
                Some((other, lid, skipped)) => {
                    points.push(current);
                    lines.push(*lid);
                    all_skipped &= *skipped;
                    prev_line = *lid;
                    current = *other;
                }
                None => break,
            }
        }
        if !closed || points.len() < 3 || all_skipped {
            visited.insert(seed.id);
            continue;
        }
        for lid in &lines {
            visited.insert(*lid);
        }
        let polygon: Vec<[f64; 2]> = points
            .iter()
            .filter_map(|id| pos(*id, sketch, state).map(|(x, y)| [x, y]))
            .collect();
        if polygon.len() < 3 {
            continue;
        }
        out.push(ClosedLoop {
            lines,
            points,
            polygon,
        });
    }
    out
}

pub(super) fn draw_filled_closed_loops(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &signex_sketch::SketchData,
    state: &FootprintEditorState,
) {
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_types::layer::SignexLayer;
    use std::collections::{HashMap, HashSet};

    // v0.16.2 — pick a fill colour for a loop by inspecting each
    // entity's role attr. Returns `None` when no entity in the loop
    // carries a role; the caller falls back to neutral grey.
    fn role_color(entity: &Entity) -> Option<FpLayer> {
        if entity.pad.is_some() {
            return Some(FpLayer::FCu);
        }
        if let Some(s) = entity.silk.as_ref() {
            return Some(if matches!(s.layer, SignexLayer::TopSilk) {
                FpLayer::FSilks
            } else {
                FpLayer::BSilks
            });
        }
        if entity.courtyard.is_some() {
            return Some(FpLayer::EdgeCuts);
        }
        if let Some(m) = entity.mask_opening.as_ref() {
            return Some(if matches!(m.layer, SignexLayer::TopSolderMask) {
                FpLayer::FFab
            } else {
                FpLayer::BFab
            });
        }
        if let Some(m) = entity.mask_exclude.as_ref() {
            return Some(if matches!(m.layer, SignexLayer::TopSolderMask) {
                FpLayer::FFab
            } else {
                FpLayer::BFab
            });
        }
        if let Some(p) = entity.paste_aperture.as_ref() {
            return Some(if matches!(p.layer, SignexLayer::TopPaste) {
                FpLayer::FFab
            } else {
                FpLayer::BFab
            });
        }
        if let Some(p) = entity.pour.as_ref() {
            return Some(if matches!(p.layer, SignexLayer::TopCopper) {
                FpLayer::FCu
            } else {
                FpLayer::BCu
            });
        }
        if entity.keepout.is_some() {
            return Some(FpLayer::EdgeCuts);
        }
        if entity.board_cutout.is_some() {
            return Some(FpLayer::EdgeCuts);
        }
        None
    }

    fn point_pos(
        id: SketchEntityId,
        sketch: &signex_sketch::SketchData,
        state: &FootprintEditorState,
    ) -> Option<(f64, f64)> {
        if let Some(solve) = state.last_solve.as_ref() {
            if let Some(p) = signex_sketch::solver::state::point_xy(
                id,
                &solve.result.state,
                &solve.result.index,
                sketch,
            ) {
                return Some(p);
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

    // Build adjacency: Point ID -> Vec<(other_endpoint, edge_id, construction)>.
    // v0.27 — Arcs are now full participants alongside Lines so the
    // rounded-rectangle (line + arc + line + arc + ...) reads as
    // one closed loop and the fill paints across the whole shape.
    let mut adj: HashMap<SketchEntityId, Vec<(SketchEntityId, SketchEntityId, bool)>> =
        HashMap::new();
    for e in &sketch.entities {
        match e.kind {
            EntityKind::Line { start, end } => {
                adj.entry(start)
                    .or_default()
                    // v0.22 Phase A5 — Treat centerline lines the same
                    // as construction for closed-loop fill detection: a
                    // loop made entirely of skipped lines must not paint
                    // a profile fill (Altium / Fusion convention).
                    .push((end, e.id, e.bake_skipped()));
                adj.entry(end)
                    .or_default()
                    .push((start, e.id, e.bake_skipped()));
            }
            EntityKind::Arc { start, end, .. } => {
                adj.entry(start)
                    .or_default()
                    .push((end, e.id, e.bake_skipped()));
                adj.entry(end)
                    .or_default()
                    .push((start, e.id, e.bake_skipped()));
            }
            _ => {}
        }
    }

    let mut visited_lines: HashSet<SketchEntityId> = HashSet::new();
    for seed in &sketch.entities {
        let (seed_start, seed_end) = match seed.kind {
            EntityKind::Line { start, end } => (start, end),
            EntityKind::Arc { start, end, .. } => (start, end),
            _ => continue,
        };
        if visited_lines.contains(&seed.id) {
            continue;
        }
        // Walk: start at seed_start, follow seed → seed_end → next →
        // ... until we return to seed_start or fail.
        let mut points: Vec<SketchEntityId> = vec![seed_start];
        let mut lines: Vec<SketchEntityId> = vec![seed.id];
        let mut all_construction = seed.bake_skipped();
        let mut current = seed_end;
        let mut prev_line = seed.id;
        let mut closed = false;
        for _ in 0..256 {
            if current == seed_start {
                closed = true;
                break;
            }
            let neighbors = match adj.get(&current) {
                Some(n) if n.len() == 2 => n,
                _ => break,
            };
            let next = neighbors.iter().find(|(_, lid, _)| *lid != prev_line);
            match next {
                Some((other, lid, construction)) => {
                    points.push(current);
                    lines.push(*lid);
                    all_construction &= *construction;
                    prev_line = *lid;
                    current = *other;
                }
                None => break,
            }
        }
        if !closed || points.len() < 3 || all_construction {
            // Mark seed line visited so we don't re-walk it; but
            // don't fill.
            visited_lines.insert(seed.id);
            continue;
        }
        for lid in &lines {
            visited_lines.insert(*lid);
        }
        // Resolve to world positions, drop loops with missing data.
        let positions: Vec<(f64, f64)> = points
            .iter()
            .filter_map(|id| point_pos(*id, sketch, state))
            .collect();
        if positions.len() < 3 {
            continue;
        }
        // v0.16.2 — find the first role attr in the loop's lines or
        // points; use its layer colour for the fill. Falls back to
        // neutral grey when nothing in the loop carries a role.
        let loop_role: Option<FpLayer> = lines
            .iter()
            .chain(points.iter())
            .filter_map(|id| sketch.entities.iter().find(|e| e.id == *id))
            .find_map(role_color);
        // v0.27 — Fusion-style two-tone fill. The fill is the
        // primary visual cue of "this is a closed shape" — Fusion
        // shows pale-blue when idle and saturated-blue when the
        // whole loop is selected. We mirror that here: detect a
        // loop-wide selection by checking whether any line / point
        // ID is in the user's selection set, then pick the
        // saturated-blue plate; otherwise stay pale.
        //
        // Role-tagged loops (Pad / Silk / Courtyard / etc.)
        // override the pale fill with the layer colour at higher
        // alpha so the user can still distinguish role assignments
        // at a glance. Selection still wins to make the active
        // shape pop.
        let mut selected_set: std::collections::HashSet<SketchEntityId> =
            std::collections::HashSet::new();
        if let Some(id) = state.selected_sketch {
            selected_set.insert(id);
        }
        if let Some(id) = state.selected_sketch_secondary {
            selected_set.insert(id);
        }
        for id in &state.selected_sketch_extra {
            selected_set.insert(*id);
        }
        let loop_selected = !selected_set.is_empty()
            && (lines.iter().any(|l| selected_set.contains(l))
                || points.iter().any(|p| selected_set.contains(p)));
        let fill = if loop_selected {
            // Fusion saturated-blue selected fill.
            Color::from_rgba(0.30, 0.55, 0.92, 0.45)
        } else {
            match loop_role {
                Some(layer) => {
                    let c = layer.color();
                    Color {
                        r: c.r,
                        g: c.g,
                        b: c.b,
                        a: 0.20,
                    }
                }
                None => {
                    // Fusion idle pale-blue.
                    Color::from_rgba(0.55, 0.75, 0.98, 0.18)
                }
            }
        };
        let path = Path::new(|builder| {
            let p0 = cstate.world_to_screen(positions[0]);
            builder.move_to(p0);
            for pos in positions.iter().skip(1) {
                let p = cstate.world_to_screen(*pos);
                builder.line_to(p);
            }
            builder.close();
        });
        frame.fill(&path, fill);
    }
}


/// v0.14.2 — live ghost preview for the multi-click sketch drawing
/// tools. Reads `state.tool_pending` + `state.cursor_mm` and draws a
/// dashed semi-transparent overlay showing where the next click would
/// land:
///
/// - **Line tool, after click 1** → ghost line from first endpoint
///   to cursor.
/// - **Circle tool, after click 1** → ghost circle centred on click 1
///   with radius = distance(centre, cursor).
/// - **Arc tool, after click 1** → ghost line from centre to cursor
///   (cursor will become the start endpoint).
/// - **Arc tool, after click 2** → ghost arc from start through the
///   cursor angle, around the centre.
/// v0.27 — Fusion-style dimension pill chrome. Centred at
/// `centre` (screen coords), draws a soft grey rounded-look
/// rectangle behind a centred white label. Used by the live
/// dimension overlays during sketch tool placement so the user
/// sees the running length / width / height / angle as they
/// move the cursor.
fn draw_dim_pill(frame: &mut canvas::Frame, centre: Point, label: &str) {
    // Label text first so we can size the background plate from
    // its rough advance width. Iosevka 11px averages ≈ 6 px per
    // character; pad ±5 px on each side and 3 px top/bottom.
    let glyph_w = 6.5_f32;
    let pad_x = 5.0_f32;
    let pad_y = 2.0_f32;
    let body_w = glyph_w * (label.chars().count() as f32) + pad_x * 2.0;
    let body_h = 14.0_f32 + pad_y * 2.0;
    let plate_origin = Point::new(centre.x - body_w / 2.0, centre.y - body_h / 2.0);
    frame.fill_rectangle(
        plate_origin,
        iced::Size::new(body_w, body_h),
        Color::from_rgba(0.20, 0.22, 0.26, 0.92),
    );
    frame.stroke(
        &Path::rectangle(plate_origin, iced::Size::new(body_w, body_h)),
        Stroke::default()
            .with_width(0.8)
            .with_color(Color::from_rgba(0.10, 0.12, 0.15, 1.0)),
    );
    frame.fill_text(canvas::Text {
        content: label.to_string(),
        position: centre,
        color: Color::from_rgba(0.95, 0.95, 0.97, 1.0),
        size: iced::Pixels(11.0),
        align_x: iced::alignment::Horizontal::Center.into(),
        align_y: iced::alignment::Vertical::Center,
        ..canvas::Text::default()
    });
}

pub(super) fn draw_sketch_tool_preview(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &signex_sketch::SketchData,
    state: &FootprintEditorState,
) {
    use crate::library::editor::footprint::state::ToolPending;
    use signex_sketch::entity::EntityKind;
    use signex_sketch::id::SketchEntityId;

    let cursor = match state.cursor_mm {
        Some(c) => c,
        None => return,
    };

    let resolve_point = |id: SketchEntityId| -> Option<(f64, f64)> {
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
    };

    // Ghost colour — accent at low alpha so it reads as preview, not
    // committed geometry. Dashed stroke for the same reason.
    let ghost = Color::from_rgba(0.40, 0.70, 1.00, 0.85);
    let stroke = Stroke::default().with_width(1.5).with_color(ghost);

    let dashed = |frame: &mut canvas::Frame, p0: Point, p1: Point| {
        let dx = p1.x - p0.x;
        let dy = p1.y - p0.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len <= 0.5 {
            return;
        }
        let dash_len = 8.0_f32;
        let n = ((len / dash_len).ceil() as i32).max(2);
        for i in (0..n).step_by(2) {
            let t0 = i as f32 / n as f32;
            let t1 = ((i + 1) as f32 / n as f32).min(1.0);
            let q0 = Point::new(p0.x + dx * t0, p0.y + dy * t0);
            let q1 = Point::new(p0.x + dx * t1, p0.y + dy * t1);
            frame.stroke(&Path::line(q0, q1), stroke);
        }
    };

    let cursor_screen = cstate.world_to_screen(cursor);

    // v0.27 — suppress the bright cyan cursor pip; the dark-blue
    // snap reticle (drawn later in canvas/mod.rs) already marks the
    // snap target. The pip + reticle stacking read as "too bright"
    // on the white sketch canvas.

    match state.tool_pending {
        ToolPending::Idle => {}
        ToolPending::LineFirst { first } => {
            let Some(first_world) = resolve_point(first) else {
                return;
            };
            let p0 = cstate.world_to_screen(first_world);
            dashed(frame, p0, cursor_screen);
            // v0.27 — Fusion-style live dimension pill. Length =
            // distance(first, cursor) in mm; angle = direction in
            // degrees. Length pill sits next to the segment
            // midpoint; angle pill sits offset from the cursor.
            let length_mm =
                ((cursor.0 - first_world.0).powi(2) + (cursor.1 - first_world.1).powi(2)).sqrt();
            let angle_deg =
                ((cursor.1 - first_world.1).atan2(cursor.0 - first_world.0)).to_degrees();
            let mid = Point::new((p0.x + cursor_screen.x) / 2.0, (p0.y + cursor_screen.y) / 2.0);
            // Perpendicular offset for the length pill so it sits
            // beside the segment, not on top of it.
            let dx = cursor_screen.x - p0.x;
            let dy = cursor_screen.y - p0.y;
            let len_screen = (dx * dx + dy * dy).sqrt().max(1.0);
            let nx = -dy / len_screen;
            let ny = dx / len_screen;
            let length_label =
                Point::new(mid.x + nx * 18.0, mid.y + ny * 18.0);
            draw_dim_pill(frame, length_label, &format!("{:.3} mm", length_mm));
            // Angle pill — offset down-right from the cursor.
            let angle_label = Point::new(cursor_screen.x + 22.0, cursor_screen.y + 22.0);
            draw_dim_pill(frame, angle_label, &format!("{:.1} deg", angle_deg));
        }
        ToolPending::RectangleFirst { first } => {
            // v0.15 — preview the axis-aligned rectangle from the
            // first corner to the cursor.
            let Some(first_world) = resolve_point(first) else {
                return;
            };
            let p0 = cstate.world_to_screen(first_world);
            let p2 = cursor_screen;
            let p1 = Point::new(p2.x, p0.y);
            let p3 = Point::new(p0.x, p2.y);
            dashed(frame, p0, p1);
            dashed(frame, p1, p2);
            dashed(frame, p2, p3);
            dashed(frame, p3, p0);
            // v0.27 — Fusion-style width + height dimension pills.
            // Width along the top edge; height along the left edge.
            let width_mm = (cursor.0 - first_world.0).abs();
            let height_mm = (cursor.1 - first_world.1).abs();
            let top_mid = Point::new((p0.x + p1.x) / 2.0, p0.y - 18.0);
            let left_mid = Point::new(p0.x - 18.0, (p0.y + p3.y) / 2.0);
            draw_dim_pill(frame, top_mid, &format!("{:.3} mm", width_mm));
            draw_dim_pill(frame, left_mid, &format!("{:.3} mm", height_mm));
        }
        ToolPending::RoundedRectangleFirst { first } => {
            // v0.16 — preview the rounded rectangle. Compute the bbox
            // from the first corner + cursor, derive a clamped
            // corner radius from the dimension input (default 0.5
            // mm), and stroke 4 dashed line segments + 4 dashed
            // 90° arcs.
            let Some(first_world) = resolve_point(first) else {
                return;
            };
            let x0 = first_world.0.min(cursor.0);
            let y0 = first_world.1.min(cursor.1);
            let x1 = first_world.0.max(cursor.0);
            let y1 = first_world.1.max(cursor.1);
            let half_w = (x1 - x0) / 2.0;
            let half_h = (y1 - y0) / 2.0;
            let r_input = state
                .dimension_input
                .trim()
                .parse::<f64>()
                .ok()
                .unwrap_or(0.5);
            let r_max = half_w.min(half_h).max(0.05);
            let r = r_input.clamp(0.05, r_max);
            // Line endpoints in world coords.
            let tl_right = (x0 + r, y0);
            let tr_left = (x1 - r, y0);
            let tr_top = (x1, y0 + r);
            let br_top = (x1, y1 - r);
            let br_right = (x1 - r, y1);
            let bl_left = (x0 + r, y1);
            let bl_bot = (x0, y1 - r);
            let tl_bot = (x0, y0 + r);
            // Lines.
            for (a, b) in [
                (tl_right, tr_left),
                (tr_top, br_top),
                (br_right, bl_left),
                (bl_bot, tl_bot),
            ] {
                dashed(frame, cstate.world_to_screen(a), cstate.world_to_screen(b));
            }
            // Arc centres.
            let centres = [
                ((x1 - r, y0 + r), tr_left, tr_top),
                ((x1 - r, y1 - r), br_top, br_right),
                ((x0 + r, y1 - r), bl_left, bl_bot),
                ((x0 + r, y0 + r), tl_bot, tl_right),
            ];
            for (c_world, s_world, e_world) in centres {
                let a0 = (s_world.1 - c_world.1).atan2(s_world.0 - c_world.0);
                let a1 = (e_world.1 - c_world.1).atan2(e_world.0 - c_world.0);
                let mut delta = a1 - a0;
                while delta < 0.0 {
                    delta += std::f64::consts::TAU;
                }
                let segs = 12;
                let mut prev = cstate.world_to_screen(s_world);
                for i in 1..=segs {
                    if i % 2 == 0 {
                        let t = (i as f64) / (segs as f64);
                        let a = a0 + delta * t;
                        let p = (c_world.0 + r * a.cos(), c_world.1 + r * a.sin());
                        let q = cstate.world_to_screen(p);
                        frame.stroke(&Path::line(prev, q), stroke);
                        prev = q;
                    } else {
                        let t = (i as f64) / (segs as f64);
                        let a = a0 + delta * t;
                        let p = (c_world.0 + r * a.cos(), c_world.1 + r * a.sin());
                        prev = cstate.world_to_screen(p);
                    }
                }
            }
            // v0.27 — width / height / radius dim pills.
            let p0_screen = cstate.world_to_screen(first_world);
            let cur_screen = cstate.world_to_screen(cursor);
            let width_mm = (cursor.0 - first_world.0).abs();
            let height_mm = (cursor.1 - first_world.1).abs();
            let top_mid = Point::new((p0_screen.x + cur_screen.x) / 2.0, p0_screen.y - 18.0);
            let left_mid = Point::new(p0_screen.x - 18.0, (p0_screen.y + cur_screen.y) / 2.0);
            draw_dim_pill(frame, top_mid, &format!("{:.3} mm", width_mm));
            draw_dim_pill(frame, left_mid, &format!("{:.3} mm", height_mm));
            // Radius pill near the cursor so the user sees the
            // clamped corner radius (driven by `dimension_input`).
            let r_label = Point::new(cur_screen.x + 24.0, cur_screen.y + 24.0);
            draw_dim_pill(frame, r_label, &format!("r {:.3} mm", r));
        }
        ToolPending::CircleCenter { center } => {
            let Some(c_world) = resolve_point(center) else {
                return;
            };
            let c_screen = cstate.world_to_screen(c_world);
            let r_world = ((cursor.0 - c_world.0).powi(2) + (cursor.1 - c_world.1).powi(2)).sqrt();
            let r_screen = (r_world as f32) * cstate.scale;
            // Approximate dashed circle with 32-segment polyline.
            let segments = 32;
            for i in (0..segments).step_by(2) {
                let t0 = i as f32 / segments as f32;
                let t1 = (i + 1) as f32 / segments as f32;
                let a0 = t0 * std::f32::consts::TAU;
                let a1 = t1 * std::f32::consts::TAU;
                let q0 = Point::new(
                    c_screen.x + r_screen * a0.cos(),
                    c_screen.y + r_screen * a0.sin(),
                );
                let q1 = Point::new(
                    c_screen.x + r_screen * a1.cos(),
                    c_screen.y + r_screen * a1.sin(),
                );
                frame.stroke(&Path::line(q0, q1), stroke);
            }
            // Radial guide from centre to cursor.
            dashed(frame, c_screen, cursor_screen);
            // v0.27 — radius pill near the cursor.
            let mid = Point::new(
                (c_screen.x + cursor_screen.x) / 2.0,
                (c_screen.y + cursor_screen.y) / 2.0,
            );
            // Perpendicular offset so the pill sits beside the
            // radial guide, not on top of it.
            let dx_s = cursor_screen.x - c_screen.x;
            let dy_s = cursor_screen.y - c_screen.y;
            let len_s = (dx_s * dx_s + dy_s * dy_s).sqrt().max(1.0);
            let nx = -dy_s / len_s;
            let ny = dx_s / len_s;
            let label = Point::new(mid.x + nx * 18.0, mid.y + ny * 18.0);
            draw_dim_pill(frame, label, &format!("r {:.3} mm", r_world));
        }
        ToolPending::ArcCenter { center } => {
            // Centre placed; cursor will become the start point. Show
            // a dashed radial line from centre to cursor.
            let Some(c_world) = resolve_point(center) else {
                return;
            };
            let c_screen = cstate.world_to_screen(c_world);
            dashed(frame, c_screen, cursor_screen);
            // v0.27 — radius pill on the radial midpoint.
            let r_world =
                ((cursor.0 - c_world.0).powi(2) + (cursor.1 - c_world.1).powi(2)).sqrt();
            let mid = Point::new(
                (c_screen.x + cursor_screen.x) / 2.0,
                (c_screen.y + cursor_screen.y) / 2.0,
            );
            let dx_s = cursor_screen.x - c_screen.x;
            let dy_s = cursor_screen.y - c_screen.y;
            let len_s = (dx_s * dx_s + dy_s * dy_s).sqrt().max(1.0);
            let nx = -dy_s / len_s;
            let ny = dx_s / len_s;
            let label = Point::new(mid.x + nx * 18.0, mid.y + ny * 18.0);
            draw_dim_pill(frame, label, &format!("r {:.3} mm", r_world));
        }
        ToolPending::ArcStart { center, start } => {
            // Centre + start placed; cursor will become the end. Draw
            // a dashed CCW arc from start to cursor angle.
            let Some(c_world) = resolve_point(center) else {
                return;
            };
            let Some(s_world) = resolve_point(start) else {
                return;
            };
            let c_screen = cstate.world_to_screen(c_world);
            let r_world =
                ((s_world.0 - c_world.0).powi(2) + (s_world.1 - c_world.1).powi(2)).sqrt();
            let r_screen = (r_world as f32) * cstate.scale;
            let start_angle = (s_world.1 - c_world.1).atan2(s_world.0 - c_world.0) as f32;
            let end_angle = (cursor.1 - c_world.1).atan2(cursor.0 - c_world.0) as f32;
            // CCW sweep — wrap end above start by 2π if needed.
            let mut delta = end_angle - start_angle;
            while delta < 0.0 {
                delta += std::f32::consts::TAU;
            }
            let segments = 32;
            for i in (0..segments).step_by(2) {
                let t0 = i as f32 / segments as f32;
                let t1 = (i + 1) as f32 / segments as f32;
                let a0 = start_angle + delta * t0;
                let a1 = start_angle + delta * t1;
                let q0 = Point::new(
                    c_screen.x + r_screen * a0.cos(),
                    c_screen.y + r_screen * a0.sin(),
                );
                let q1 = Point::new(
                    c_screen.x + r_screen * a1.cos(),
                    c_screen.y + r_screen * a1.sin(),
                );
                frame.stroke(&Path::line(q0, q1), stroke);
            }
            // Radial guides for both endpoints + cursor.
            let s_screen = cstate.world_to_screen(s_world);
            dashed(frame, c_screen, s_screen);
            dashed(frame, c_screen, cursor_screen);
            // v0.27 — sweep angle pill (deg). Radius is fixed by
            // the start endpoint so we surface the sweep, which is
            // what changes as the cursor moves.
            let sweep_deg = (delta.to_degrees().rem_euclid(360.0)) as f64;
            let label = Point::new(cursor_screen.x + 24.0, cursor_screen.y + 24.0);
            draw_dim_pill(frame, label, &format!("{:.1} deg", sweep_deg));
        }
        // v0.23 — Polar centre re-pick has no preview shape; the
        // cursor PIP at the top of this match is the only visual cue.
        ToolPending::RepickPolarCenter { .. } => {}
        // v0.24 Track C — Tangent Arc, first endpoint placed.
        // Mirror the dispatcher's geometry: locate a Line ending at
        // `first`, compute the tangent-circle centre on the line's
        // perpendicular through `first` that passes through the
        // cursor, and stroke a dashed ghost arc from `first` to the
        // cursor along that circle.
        //
        // Without an incident line, fall back to a dashed straight
        // segment (matches the LineFirst preview) so the user still
        // gets visual feedback while the dispatcher will publish a
        // placeholder warning on commit.
        ToolPending::TangentArcFirst { first } => {
            let Some(first_world) = resolve_point(first) else {
                return;
            };
            // Find a Line ending at `first` (most recent first, same
            // priority the dispatcher uses).
            let incident_line: Option<(f64, f64)> =
                sketch.entities.iter().rev().find_map(|e| match e.kind {
                    EntityKind::Line { start, end } if end == first => resolve_point(start),
                    EntityKind::Line { start, end } if start == first => resolve_point(end),
                    _ => None,
                });
            let p0 = cstate.world_to_screen(first_world);
            match incident_line {
                Some(line_other) => {
                    // Line direction (line_other -> first).
                    let lx = first_world.0 - line_other.0;
                    let ly = first_world.1 - line_other.1;
                    let llen_sq = lx * lx + ly * ly;
                    if llen_sq <= 1e-12 {
                        dashed(frame, p0, cursor_screen);
                        return;
                    }
                    let llen = llen_sq.sqrt();
                    // Perpendicular to the line at `first`.
                    let nx = -ly / llen;
                    let ny = lx / llen;
                    // Solve for the centre (see dispatcher comment).
                    let dx = first_world.0 - cursor.0;
                    let dy = first_world.1 - cursor.1;
                    let denom = 2.0 * (dx * nx + dy * ny);
                    let chord_sq = dx * dx + dy * dy;
                    if denom.abs() <= 1e-9 || chord_sq <= 1e-9 {
                        // Cursor on the tangent line — preview the
                        // straight segment until the cursor pulls off
                        // axis.
                        dashed(frame, p0, cursor_screen);
                        return;
                    }
                    let t = -chord_sq / denom;
                    let cx = first_world.0 + t * nx;
                    let cy = first_world.1 + t * ny;
                    // Radius (use the start-side distance — both
                    // sides should match within solver tolerance).
                    let rx = first_world.0 - cx;
                    let ry = first_world.1 - cy;
                    let r_world = (rx * rx + ry * ry).sqrt();
                    if r_world <= 1e-9 {
                        dashed(frame, p0, cursor_screen);
                        return;
                    }
                    // Sweep direction — match the dispatcher's logic.
                    let ex = cursor.0 - first_world.0;
                    let ey = cursor.1 - first_world.1;
                    let sweep_ccw = lx * ey - ly * ex >= 0.0;
                    // Stroke the dashed arc.
                    let c_screen =
                        cstate.world_to_screen((cx, cy));
                    let r_screen = (r_world as f32) * cstate.scale;
                    let start_angle =
                        (first_world.1 - cy).atan2(first_world.0 - cx) as f32;
                    let end_angle = (cursor.1 - cy).atan2(cursor.0 - cx) as f32;
                    let mut delta = end_angle - start_angle;
                    if sweep_ccw {
                        while delta < 0.0 {
                            delta += std::f32::consts::TAU;
                        }
                    } else {
                        while delta > 0.0 {
                            delta -= std::f32::consts::TAU;
                        }
                    }
                    let segments = 32;
                    for i in (0..segments).step_by(2) {
                        let t0 = i as f32 / segments as f32;
                        let t1 = (i + 1) as f32 / segments as f32;
                        let a0 = start_angle + delta * t0;
                        let a1 = start_angle + delta * t1;
                        let q0 = Point::new(
                            c_screen.x + r_screen * a0.cos(),
                            c_screen.y + r_screen * a0.sin(),
                        );
                        let q1 = Point::new(
                            c_screen.x + r_screen * a1.cos(),
                            c_screen.y + r_screen * a1.sin(),
                        );
                        frame.stroke(&Path::line(q0, q1), stroke);
                    }
                }
                None => {
                    // No incident line — show a dashed chord so the
                    // user gets a visual cue. Dispatcher publishes
                    // the "no incident line" warning on commit.
                    dashed(frame, p0, cursor_screen);
                }
            }
        }
        // v0.27 — Fillet, first Line picked. The tool is waiting for
        // the user's second-line click; no shape preview to draw,
        // the cursor reticle is the sole visual cue.
        ToolPending::FilletFirst { .. } => {}
    }

    // v0.24 Track D — modeless live numeric placement-input overlay.
    // Renders the user-typed buffer at the cursor whenever
    // `placement_input` is `Some`, regardless of which `ToolPending`
    // is current (the kind picker decided which tool's gesture mints
    // it; the dispatcher tolerates unrelated tool changes by clearing
    // on commit + on Esc). Position: 4 px right and 8 px below the
    // cursor, with a translucent rounded background so the buffer
    // reads against any canvas content.
    if let Some(input) = state.placement_input.as_ref() {
        let label = input.kind.label();
        let body = if input.buffer.is_empty() {
            format!("{label}: _")
        } else {
            format!("{label}: {}", input.buffer)
        };
        let origin = Point::new(cursor_screen.x + 4.0, cursor_screen.y + 8.0);
        // Approximate a one-line text bbox so the background plate
        // sits behind the glyphs. Iosevka 11px averages ~6 px per
        // character at the canvas's default rendering; a 4 px pad
        // around the label keeps the chrome readable.
        let glyph_w = 6.5_f32;
        let pad_x = 5.0_f32;
        let pad_y = 3.0_f32;
        let body_w = glyph_w * (body.chars().count() as f32) + pad_x * 2.0;
        let body_h = 16.0_f32 + pad_y * 2.0;
        let plate_origin = Point::new(origin.x - pad_x, origin.y - pad_y);
        // Background plate.
        frame.fill_rectangle(
            plate_origin,
            iced::Size::new(body_w, body_h),
            Color::from_rgba(0.05, 0.07, 0.10, 0.85),
        );
        // Accent border.
        frame.stroke(
            &Path::rectangle(plate_origin, iced::Size::new(body_w, body_h)),
            Stroke::default()
                .with_width(1.0)
                .with_color(Color::from_rgba(0.40, 0.70, 1.00, 0.95)),
        );
        // Buffer text.
        frame.fill_text(canvas::Text {
            content: body,
            position: origin,
            color: Color::from_rgba(0.95, 0.95, 0.97, 1.00),
            size: iced::Pixels(13.0),
            ..canvas::Text::default()
        });
    }
}

