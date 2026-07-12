//! Constraint-glyph overlay — each constraint's Unicode glyph rendered
//! at the centroid of the entities it touches, tinted red when the
//! constraint is over-constrained.

use iced::widget::canvas;
use iced::{Color, Point, Radians, Vector};

use crate::library::editor::footprint::canvas::FootprintCanvasState;
use crate::library::editor::footprint::state::FootprintEditorState;

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
        // v0.27 — `primary_line` carries the two endpoint IDs of the
        // line whose direction the glyph should track. Set for every
        // line-anchored constraint so the H / V / // / ⊥ / = glyph
        // rotates with the line during a drag instead of staying
        // upright while the geometry rotates underneath it.
        let (glyph, points, primary_line): (
            &str,
            Vec<SketchEntityId>,
            Option<(SketchEntityId, SketchEntityId)>,
        ) = match &c.kind {
            ConstraintKind::Coincident { p1, p2 } => ("=", vec![*p1, *p2], Some((*p1, *p2))),
            ConstraintKind::PointOnLine { point, line } => {
                let mut v = vec![*point];
                let lp = line_endpoints_local(*line);
                if let Some((s, e)) = lp {
                    v.push(s);
                    v.push(e);
                }
                ("|", v, lp)
            }
            ConstraintKind::Horizontal { line } => {
                let lp = line_endpoints_local(*line);
                let mut v = Vec::new();
                if let Some((s, e)) = lp {
                    v.push(s);
                    v.push(e);
                }
                ("H", v, lp)
            }
            ConstraintKind::Vertical { line } => {
                let lp = line_endpoints_local(*line);
                let mut v = Vec::new();
                if let Some((s, e)) = lp {
                    v.push(s);
                    v.push(e);
                }
                ("V", v, lp)
            }
            ConstraintKind::Parallel { l1, l2 } => {
                let lp = line_endpoints_local(*l1);
                let mut v = Vec::new();
                if let Some((s, e)) = lp {
                    v.extend([s, e]);
                }
                if let Some((s, e)) = line_endpoints_local(*l2) {
                    v.extend([s, e]);
                }
                ("//", v, lp)
            }
            ConstraintKind::Perpendicular { l1, l2 } => {
                let lp = line_endpoints_local(*l1);
                let mut v = Vec::new();
                if let Some((s, e)) = lp {
                    v.extend([s, e]);
                }
                if let Some((s, e)) = line_endpoints_local(*l2) {
                    v.extend([s, e]);
                }
                ("L", v, lp)
            }
            ConstraintKind::DistancePtPt { p1, p2, .. } => ("D", vec![*p1, *p2], None),
            ConstraintKind::DistancePtLine { point, line, .. } => {
                let mut v = vec![*point];
                if let Some((s, e)) = line_endpoints_local(*line) {
                    v.push(s);
                    v.push(e);
                }
                ("d", v, None)
            }
            ConstraintKind::DistancePtCircle { point, circle, .. } => {
                let mut v = vec![*point];
                if let Some(c) = circle_center_local(sketch, *circle) {
                    v.push(c);
                } else if let Some((c, _, _, _)) = arc_refs_local(sketch, *circle) {
                    v.push(c);
                }
                ("\u{29bf}", v, None) // ⦿ "DistancePtCircle"
            }
            ConstraintKind::Fixed { point } => ("\u{1F512}", vec![*point], None),
            // v0.13.3 — remaining constraint glyphs.
            ConstraintKind::PointOnArc { point, arc } => {
                let mut v = vec![*point];
                if let Some((c, s, e, _)) = arc_refs_local(sketch, *arc) {
                    v.extend([c, s, e]);
                }
                ("\u{2192}", v, None) // → "PointOnArc"
            }
            ConstraintKind::Angle { l1, l2, .. } => {
                let mut v = Vec::new();
                if let Some((s, e)) = line_endpoints_local(*l1) {
                    v.extend([s, e]);
                }
                if let Some((s, e)) = line_endpoints_local(*l2) {
                    v.extend([s, e]);
                }
                ("A", v, None)
            }
            ConstraintKind::EqualLength { l1, l2 } => {
                let lp = line_endpoints_local(*l1);
                let mut v = Vec::new();
                if let Some((s, e)) = lp {
                    v.extend([s, e]);
                }
                if let Some((s, e)) = line_endpoints_local(*l2) {
                    v.extend([s, e]);
                }
                ("=L", v, lp)
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
                ("=R", v, None)
            }
            ConstraintKind::TangentLineArc { line, arc } => {
                let lp = line_endpoints_local(*line);
                let mut v = Vec::new();
                if let Some((s, e)) = lp {
                    v.extend([s, e]);
                }
                if let Some((c, _, _, _)) = arc_refs_local(sketch, *arc) {
                    v.push(c);
                }
                ("T", v, lp)
            }
            ConstraintKind::TangentArcArc { a1, a2, .. } => {
                let mut v = Vec::new();
                if let Some((c, _, _, _)) = arc_refs_local(sketch, *a1) {
                    v.push(c);
                }
                if let Some((c, _, _, _)) = arc_refs_local(sketch, *a2) {
                    v.push(c);
                }
                ("TT", v, None)
            }
            ConstraintKind::SymmetricAboutLine { p1, p2, line } => {
                let lp = line_endpoints_local(*line);
                let mut v = vec![*p1, *p2];
                if let Some((s, e)) = lp {
                    v.extend([s, e]);
                }
                ("\u{29C7}", v, lp) // ⧇ "Symmetric"
            }
            ConstraintKind::SymmetricAboutPoint { p1, p2, center } => {
                ("\u{29C7}", vec![*p1, *p2, *center], Some((*p1, *p2)))
            }
            ConstraintKind::Midpoint { point, line } => {
                let lp = line_endpoints_local(*line);
                let mut v = vec![*point];
                if let Some((s, e)) = lp {
                    v.extend([s, e]);
                }
                ("M", v, lp)
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
        // v0.27 — rotate the glyph along its primary line so an H,
        // V, //, ⊥, =, =L etc. follows the line's current direction
        // during a drag. Without this the glyph reads upright while
        // the geometry rotates underneath, which looks decoupled.
        // For non-line-anchored constraints (Fixed, distance values,
        // tangent-arc-arc, etc.) `primary_line` is None and the
        // glyph stays upright.
        let rotation = primary_line.and_then(|(a_id, b_id)| {
            let a = point_world_local(a_id)?;
            let b = point_world_local(b_id)?;
            let a_screen = cstate.world_to_screen(a);
            let b_screen = cstate.world_to_screen(b);
            let dx = b_screen.x - a_screen.x;
            let dy = b_screen.y - a_screen.y;
            if dx.hypot(dy) <= 1.0 {
                return None;
            }
            // v0.27 — track the raw line angle. Earlier revisions
            // clamped to [-π/2, π/2] to keep the glyph upright, but
            // that decoupled the glyph orientation from a freely
            // rotating line and read as "limited". Now the glyph
            // follows the line through a full 360°.
            Some(dy.atan2(dx))
        });
        let text = canvas::Text {
            content: glyph.to_string(),
            // Glyph centred on its draw position when rotated; when
            // upright we keep the legacy +6 / -6 nudge so the
            // existing constraint cluster doesn't jump.
            position: if rotation.is_some() {
                Point::new(0.0, -10.0)
            } else {
                Point::new(p.x + 6.0, p.y - 6.0)
            },
            color: colour,
            size: iced::Pixels(11.0),
            ..canvas::Text::default()
        };
        if let Some(angle) = rotation {
            frame.with_save(|inner| {
                inner.translate(Vector::new(p.x, p.y));
                inner.rotate(Radians(angle));
                inner.fill_text(text);
            });
        } else {
            frame.fill_text(text);
        }
    }
}
