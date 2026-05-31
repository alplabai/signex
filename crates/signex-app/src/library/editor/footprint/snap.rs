//! v0.16.1 — Fusion-style cursor snap for the footprint sketch
//! canvas.
//!
//! Priority chain (highest first):
//! 1. **Point snap** — cursor within `SKETCH_SNAP_RADIUS_PX` of an
//!    existing `Point` entity → snap to that Point's position.
//! 2. **Horizontal / Vertical inference** — when a multi-click tool
//!    is mid-gesture (line first endpoint, rect first corner, ...),
//!    if the cursor's angle relative to the anchor falls within
//!    [`AXIS_THRESHOLD_DEG`] of horizontal or vertical, snap to the
//!    exact axis.
//! 3. **Angle snap** — same anchor-relative angle, snap to nearest
//!    [`ANGLE_STEP_DEG`] increment if within
//!    [`ANGLE_THRESHOLD_DEG`].
//! 4. **Grid snap** — fall through: round each axis to the nearest
//!    [`GRID_STEP_MM`] increment.
//!
//! No modifier-key suppression yet; iced's `CursorMoved` event
//! doesn't carry modifier state cleanly in 0.14, so a Shift-to-
//! disable toggle is deferred.

use signex_sketch::SketchData;
use signex_sketch::id::SketchEntityId;

use super::state::{FootprintEditorState, ToolPending};

/// Fallback grid step (mm) used only by the snap unit tests where
/// constructing a full `SnapOptions` is cumbersome. Production code
/// reads the configurable `state.snap_options.grid_step_mm` field
/// (v0.18.9), not this constant.
pub const GRID_STEP_MM: f64 = 1.0;
/// Threshold for horizontal / vertical inference (degrees).
pub const AXIS_THRESHOLD_DEG: f64 = 5.0;
/// Angle-snap increment in degrees (15° → 24 ticks per turn).
pub const ANGLE_STEP_DEG: f64 = 15.0;
/// Threshold for angle-snap engagement (degrees).
pub const ANGLE_THRESHOLD_DEG: f64 = 3.0;
/// Pixel radius for snapping to existing Point entities. Same as the
/// constant the canvas uses for its hit-test, kept here so callers
/// outside `canvas.rs` can pass it without importing the canvas.
pub const POINT_SNAP_RADIUS_PX: f32 = 8.0;

/// Outcome of a cursor snap. The `pos` field is the canvas's working
/// world coordinate after snapping; the `kind` discriminates which
/// snap policy fired so the canvas can render an indicator badge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapResult {
    pub pos: (f64, f64),
    pub kind: SnapKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnapKind {
    /// No snap fired — cursor passed through. (Used when the
    /// canvas falls back to raw input, e.g. inside the Select tool
    /// without a hovered Point.)
    Raw,
    /// Snapped onto an existing `Point` entity.
    Point(SketchEntityId),
    Horizontal,
    Vertical,
    /// Snapped to a non-cardinal angle (radians, 0..2π) relative to
    /// the active anchor.
    Angle(f64),
    /// v0.18.23 — snapped onto an Altium-style guide line. When both
    /// an X-guide and a Y-guide fire on the same cursor pass the
    /// snap pins both axes.
    Guide,
    /// v0.27 — snapped onto the intersection of two sketch Line
    /// entities. Powered by `signex_sketch::geom::segment_segment_intersection`.
    Intersection,
    /// Fell through to grid snap.
    Grid,
}

/// v0.18.23 — world-mm tolerance used by the guide-snap priority.
/// The cursor snaps onto a guide whose axis is within this distance.
/// Picked to feel sticky at typical 1 mm grid steps without competing
/// against point-hit (which is px-radius scaled).
pub const GUIDE_SNAP_TOLERANCE_MM: f64 = 0.5;

impl SnapResult {
    pub fn raw(pos: (f64, f64)) -> Self {
        Self {
            pos,
            kind: SnapKind::Raw,
        }
    }
}

/// Convert a screen-pixel radius to world-mm at the current camera
/// scale. Callers pass the canvas's `scale` (px/mm) and we return
/// the equivalent world-mm radius for distance comparisons.
pub fn px_to_world_mm(px: f32, scale_px_per_mm: f32) -> f64 {
    (px / scale_px_per_mm.max(1e-3)) as f64
}

/// Look up the active tool's anchor — the previously-placed Point
/// the next click attaches to. Returns `None` for tool states with
/// no anchor (Idle, Select, Place Point single-shot).
pub fn anchor_for_tool(
    state: &FootprintEditorState,
    sketch: Option<&SketchData>,
) -> Option<(f64, f64)> {
    let id = match state.tool_pending {
        ToolPending::Idle => return None,
        ToolPending::LineFirst { first }
        | ToolPending::RectangleFirst { first }
        | ToolPending::RoundedRectangleFirst { first } => first,
        ToolPending::CircleCenter { center } | ToolPending::ArcCenter { center } => center,
        // For ArcStart we anchor to the START point — that's the one
        // the cursor's sweeping around to define `end`.
        ToolPending::ArcStart { start, .. } => start,
        // v0.23 — Polar centre re-pick has no anchor; the click sets
        // the centre directly without a tethered preview.
        ToolPending::RepickPolarCenter { .. } => return None,
        // v0.24 Track C — Tangent Arc anchors to the first endpoint
        // so cursor snap pulls onto the first click position while
        // the user picks the second endpoint.
        ToolPending::TangentArcFirst { first } => first,
        // v0.27 — Fillet first-click stashes a Line, not a Point.
        // Lines aren't valid snap anchors here, so suppress the
        // tethered preview by returning None.
        ToolPending::FilletFirst { .. } => return None,
    };
    point_pos(id, sketch, state)
}

/// World-mm position of a sketch `Point`. Prefers the solver's last
/// solved coordinates, falls back to authored coords. Returns `None`
/// if the entity isn't a `Point` or the sketch is missing.
pub fn point_pos(
    id: SketchEntityId,
    sketch: Option<&SketchData>,
    state: &FootprintEditorState,
) -> Option<(f64, f64)> {
    let sketch = sketch?;
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
            signex_sketch::entity::EntityKind::Point { x, y } => Some((x, y)),
            _ => None,
        })
}

/// Apply the priority chain to `raw` and return the snapped position
/// + the snap kind that fired. `point_hit` is the optional outcome of
/// a prior call to the canvas's `sketch_snap` (we don't re-implement
/// the spatial query here so the canvas's existing px-radius logic
/// stays the source of truth).
pub fn snap_cursor(
    raw: (f64, f64),
    sketch: Option<&SketchData>,
    state: &FootprintEditorState,
    point_hit: Option<SketchEntityId>,
) -> SnapResult {
    // v0.18.14.3 — Altium "Snapping" 3-state short-circuits all
    // priorities when set to `Off`. `CurrentLayer` is a placeholder
    // for the v0.18.15 layer-aware filter; today it behaves like
    // `AllLayers`.
    //
    // v0.18.25.1 — global status-bar Snap toggle (`ui_state.snap_enabled`)
    // mirrors into `state.global_snap_disabled`. Treat the same as
    // `SnappingMode::Off` so guides / grid / point-hit / angle all
    // stop firing together when the user disables snap globally.
    use super::state::SnappingMode;
    if state.global_snap_disabled || state.snapping_mode == SnappingMode::Off {
        return SnapResult::raw(raw);
    }
    // v0.17.0 — each priority is gated by `state.snap_options.<flag>`.
    // Disabled priorities pass through to the next; if all four are
    // disabled the cursor returns raw (matches Altium's "no snap"
    // workflow).
    let opts = state.snap_options;

    // Priority 1 — Point snap.
    if opts.point_hit {
        if let Some(id) = point_hit {
            if let Some(pos) = point_pos(id, sketch, state) {
                return SnapResult {
                    pos,
                    kind: SnapKind::Point(id),
                };
            }
        }
    }

    // Priority 1.5 — Guide snap. Snaps the cursor onto enabled
    // vertical / horizontal guides whose axis is within
    // `GUIDE_SNAP_TOLERANCE_MM`. When both fire, snap both axes (a
    // guide-intersection lock).
    if !state.guides.is_empty() {
        use super::state::GuideAxis;
        let mut snapped_x: Option<f64> = None;
        let mut snapped_y: Option<f64> = None;
        for g in state.guides.iter().filter(|g| g.enabled) {
            match g.axis {
                GuideAxis::Vertical => {
                    if (raw.0 - g.position_mm).abs() <= GUIDE_SNAP_TOLERANCE_MM
                        && snapped_x
                            .map(|sx| (raw.0 - g.position_mm).abs() < (raw.0 - sx).abs())
                            .unwrap_or(true)
                    {
                        snapped_x = Some(g.position_mm);
                    }
                }
                GuideAxis::Horizontal => {
                    if (raw.1 - g.position_mm).abs() <= GUIDE_SNAP_TOLERANCE_MM
                        && snapped_y
                            .map(|sy| (raw.1 - g.position_mm).abs() < (raw.1 - sy).abs())
                            .unwrap_or(true)
                    {
                        snapped_y = Some(g.position_mm);
                    }
                }
            }
        }
        if snapped_x.is_some() || snapped_y.is_some() {
            return SnapResult {
                pos: (snapped_x.unwrap_or(raw.0), snapped_y.unwrap_or(raw.1)),
                kind: SnapKind::Guide,
            };
        }
    }

    // v0.27 — Intersection snap. Walks Line×Line, Line×Arc, and
    // Line×Circle pairs through the geom module helpers, snaps to
    // the closest hit within `snap_distance_mm`. Arc×Arc and
    // Arc×Circle pairs are queued — they need a curve×curve
    // intersection helper that doesn't exist yet.
    if opts.snap_intersections {
        if let Some(sketch) = sketch {
            use signex_sketch::entity::EntityKind;
            use signex_sketch::geom::{
                segment_arc_intersections, segment_circle_intersections,
                segment_segment_intersection, Arc2, Circle2, Point2, Segment2,
                SegmentIntersection,
            };

            let resolve = |id: SketchEntityId| -> Option<(f64, f64)> {
                point_pos(id, Some(sketch), state)
            };

            // Lines as (start, end) world-mm pairs.
            let lines: Vec<(Point2, Point2)> = sketch
                .entities
                .iter()
                .filter_map(|e| {
                    if let EntityKind::Line { start, end } = e.kind {
                        let s = resolve(start)?;
                        let f = resolve(end)?;
                        Some((Point2::new(s.0, s.1), Point2::new(f.0, f.1)))
                    } else {
                        None
                    }
                })
                .collect();

            // Circles as (centre, radius).
            let circles: Vec<(Point2, f64)> = sketch
                .entities
                .iter()
                .filter_map(|e| {
                    if let EntityKind::Circle { center, radius } = e.kind {
                        let c = resolve(center)?;
                        Some((Point2::new(c.0, c.1), radius))
                    } else {
                        None
                    }
                })
                .collect();

            // Arcs as (centre, radius, start_rad, end_rad, sweep_ccw).
            let arcs: Vec<Arc2> = sketch
                .entities
                .iter()
                .filter_map(|e| {
                    if let EntityKind::Arc {
                        center,
                        start,
                        end,
                        sweep_ccw,
                    } = e.kind
                    {
                        let c = resolve(center)?;
                        let s = resolve(start)?;
                        let f = resolve(end)?;
                        let radius = ((s.0 - c.0).powi(2) + (s.1 - c.1).powi(2)).sqrt();
                        let start_rad = (s.1 - c.1).atan2(s.0 - c.0);
                        let end_rad = (f.1 - c.1).atan2(f.0 - c.0);
                        Some(Arc2::new(
                            Point2::new(c.0, c.1),
                            radius,
                            start_rad,
                            end_rad,
                            sweep_ccw,
                        ))
                    } else {
                        None
                    }
                })
                .collect();

            let tol = state.snap_options.snap_distance_mm.max(1e-6);
            let tol_sq = tol * tol;
            let mut best: Option<(f64, (f64, f64))> = None;
            let mut consider = |pt: Point2, best: &mut Option<(f64, (f64, f64))>| {
                let dx = pt.x - raw.0;
                let dy = pt.y - raw.1;
                let d_sq = dx * dx + dy * dy;
                if d_sq <= tol_sq {
                    match best {
                        Some((b_sq, _)) if *b_sq <= d_sq => {}
                        _ => *best = Some((d_sq, (pt.x, pt.y))),
                    }
                }
            };

            // Line × Line.
            for i in 0..lines.len() {
                for j in (i + 1)..lines.len() {
                    let s_a = Segment2::new(lines[i].0, lines[i].1);
                    let s_b = Segment2::new(lines[j].0, lines[j].1);
                    if let SegmentIntersection::Point { pt, .. } =
                        segment_segment_intersection(s_a, s_b)
                    {
                        consider(pt, &mut best);
                    }
                }
            }

            // Line × Arc.
            for line in &lines {
                let seg = Segment2::new(line.0, line.1);
                for arc in &arcs {
                    for (pt, _) in segment_arc_intersections(seg, *arc) {
                        consider(pt, &mut best);
                    }
                }
            }

            // Line × Circle.
            for line in &lines {
                let seg = Segment2::new(line.0, line.1);
                for c in &circles {
                    let circle = Circle2::new(c.0, c.1);
                    for (pt, _) in segment_circle_intersections(seg, circle) {
                        consider(pt, &mut best);
                    }
                }
            }

            // v0.27 — Curve × curve. Circle × Circle, Arc × Arc,
            // Arc × Circle. Round out the snap so any pair of
            // sketch curves yields a snap target at their
            // crossing.
            use signex_sketch::geom::{
                arc_arc_intersections, arc_circle_intersections,
                circle_circle_intersections,
            };
            for i in 0..circles.len() {
                for j in (i + 1)..circles.len() {
                    let a = Circle2::new(circles[i].0, circles[i].1);
                    let b = Circle2::new(circles[j].0, circles[j].1);
                    for pt in circle_circle_intersections(a, b) {
                        consider(pt, &mut best);
                    }
                }
            }
            for i in 0..arcs.len() {
                for j in (i + 1)..arcs.len() {
                    for pt in arc_arc_intersections(arcs[i], arcs[j]) {
                        consider(pt, &mut best);
                    }
                }
            }
            for arc in &arcs {
                for c in &circles {
                    let circle = Circle2::new(c.0, c.1);
                    for pt in arc_circle_intersections(*arc, circle) {
                        consider(pt, &mut best);
                    }
                }
            }

            if let Some((_, pos)) = best {
                return SnapResult {
                    pos,
                    kind: SnapKind::Intersection,
                };
            }
        }
    }

    // Priority 2 + 3 — anchor-relative angle snap (H/V/15°).
    if (opts.horizontal_vertical || opts.angle) {
        if let Some(anchor) = anchor_for_tool(state, sketch) {
            let dx = raw.0 - anchor.0;
            let dy = raw.1 - anchor.1;
            let dist = (dx * dx + dy * dy).sqrt();
            // Skip degenerate (cursor on the anchor) — fall through to
            // grid so we don't divide by zero on the angle math.
            if dist > 1e-6 {
                let angle = dy.atan2(dx);
                // Horizontal: angle near 0 or ±π → |sin(angle)| small.
                // Vertical:   angle near ±π/2  → |cos(angle)| small.
                let axis_thresh = AXIS_THRESHOLD_DEG.to_radians().sin();
                if opts.horizontal_vertical && angle.sin().abs() < axis_thresh {
                    let dir = if angle.cos() >= 0.0 { 1.0 } else { -1.0 };
                    let pos = (anchor.0 + dir * dist, anchor.1);
                    return SnapResult {
                        pos,
                        kind: SnapKind::Horizontal,
                    };
                }
                if opts.horizontal_vertical && angle.cos().abs() < axis_thresh {
                    let dir = if angle.sin() >= 0.0 { 1.0 } else { -1.0 };
                    let pos = (anchor.0, anchor.1 + dir * dist);
                    return SnapResult {
                        pos,
                        kind: SnapKind::Vertical,
                    };
                }

                if opts.angle {
                    // Angle snap to ANGLE_STEP_DEG increments.
                    let step_rad = ANGLE_STEP_DEG.to_radians();
                    let snapped_angle = (angle / step_rad).round() * step_rad;
                    let angle_diff = ((angle - snapped_angle).abs())
                        .min((std::f64::consts::TAU - (angle - snapped_angle).abs()).abs());
                    if angle_diff < ANGLE_THRESHOLD_DEG.to_radians() {
                        let pos = (
                            anchor.0 + dist * snapped_angle.cos(),
                            anchor.1 + dist * snapped_angle.sin(),
                        );
                        return SnapResult {
                            pos,
                            kind: SnapKind::Angle(snapped_angle),
                        };
                    }
                }
            }
        }
    }

    // Priority 4 — grid snap. When disabled, the raw cursor passes
    // through unchanged (mirrors Altium's "Smart Snap → Off" flow).
    // v0.18.9 — uses `opts.grid_step_mm` (configurable) instead of
    // a hardcoded constant. Defends against zero/negative steps by
    // falling through to raw — a misconfigured step shouldn't crash
    // the canvas or move pads to the origin.
    if opts.grid && opts.grid_step_mm > 1e-9 {
        let step = opts.grid_step_mm;
        let snapped = ((raw.0 / step).round() * step, (raw.1 / step).round() * step);
        SnapResult {
            pos: snapped,
            kind: SnapKind::Grid,
        }
    } else {
        SnapResult::raw(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_sketch::SketchData;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

    fn empty_state() -> FootprintEditorState {
        FootprintEditorState::empty()
    }

    fn sketch_with_anchor() -> (SketchData, SketchEntityId) {
        let mut sketch = SketchData::default();
        let plane = Plane {
            id: PlaneId::new(),
            kind: PlaneKind::BoardTop,
        };
        sketch.planes.push(plane.clone());
        let anchor_id = SketchEntityId::new();
        sketch.entities.push(Entity::new(
            anchor_id,
            plane.id,
            EntityKind::Point { x: 0.0, y: 0.0 },
        ));
        (sketch, anchor_id)
    }

    #[test]
    fn no_anchor_grid_snaps() {
        let state = empty_state();
        // v0.18.7.2 — GRID_STEP_MM is 1.0mm; raw cursor at
        // (1.234, -2.567) rounds to (1.0, -3.0).
        let r = snap_cursor((1.234, -2.567), None, &state, None);
        assert!((r.pos.0 - 1.0).abs() < 1e-9);
        assert!((r.pos.1 - (-3.0)).abs() < 1e-9);
        assert!(matches!(r.kind, SnapKind::Grid));
    }

    #[test]
    fn horizontal_within_threshold() {
        let (sketch, anchor_id) = sketch_with_anchor();
        let mut state = empty_state();
        state.tool_pending = ToolPending::LineFirst { first: anchor_id };
        // Almost on the +X axis (1° off horizontal).
        let raw = (5.0, 5.0 * 1.0_f64.to_radians().tan());
        let r = snap_cursor(raw, Some(&sketch), &state, None);
        assert_eq!(r.kind, SnapKind::Horizontal);
        assert!((r.pos.1).abs() < 1e-9);
    }

    #[test]
    fn vertical_within_threshold() {
        let (sketch, anchor_id) = sketch_with_anchor();
        let mut state = empty_state();
        state.tool_pending = ToolPending::LineFirst { first: anchor_id };
        // Almost on the +Y axis.
        let raw = (5.0 * 1.0_f64.to_radians().tan(), 5.0);
        let r = snap_cursor(raw, Some(&sketch), &state, None);
        assert_eq!(r.kind, SnapKind::Vertical);
        assert!((r.pos.0).abs() < 1e-9);
    }

    #[test]
    fn forty_five_degree_angle_snaps() {
        let (sketch, anchor_id) = sketch_with_anchor();
        let mut state = empty_state();
        state.tool_pending = ToolPending::LineFirst { first: anchor_id };
        // 44° from horizontal — within threshold of 45° increment.
        let angle = 44.0_f64.to_radians();
        let raw = (10.0 * angle.cos(), 10.0 * angle.sin());
        let r = snap_cursor(raw, Some(&sketch), &state, None);
        match r.kind {
            SnapKind::Angle(a) => {
                assert!((a - 45.0_f64.to_radians()).abs() < 1e-6);
            }
            other => panic!("expected Angle, got {other:?}"),
        }
    }

    #[test]
    fn point_hit_takes_priority() {
        let (sketch, anchor_id) = sketch_with_anchor();
        let state = empty_state();
        let r = snap_cursor((100.0, 100.0), Some(&sketch), &state, Some(anchor_id));
        assert_eq!(r.pos, (0.0, 0.0));
        assert!(matches!(r.kind, SnapKind::Point(_)));
    }

    #[test]
    fn far_off_axis_falls_through_to_grid() {
        let (sketch, anchor_id) = sketch_with_anchor();
        let mut state = empty_state();
        state.tool_pending = ToolPending::LineFirst { first: anchor_id };
        // 22° — outside H/V (5°) and angle-snap (3° around 15°).
        let angle = 22.0_f64.to_radians();
        let raw = (10.0 * angle.cos(), 10.0 * angle.sin());
        let r = snap_cursor(raw, Some(&sketch), &state, None);
        assert!(matches!(r.kind, SnapKind::Grid));
    }

    #[test]
    fn vertical_guide_snaps_x_axis() {
        let mut state = empty_state();
        state.guides.push(super::super::state::Guide {
            axis: super::super::state::GuideAxis::Vertical,
            position_mm: 5.0,
            enabled: true,
        });
        // Cursor 0.2 mm from the guide on X — within 0.5 mm tolerance.
        let r = snap_cursor((4.8, 1.234), None, &state, None);
        assert_eq!(r.kind, SnapKind::Guide);
        assert!((r.pos.0 - 5.0).abs() < 1e-9);
        assert!((r.pos.1 - 1.234).abs() < 1e-9);
    }

    #[test]
    fn horizontal_guide_snaps_y_axis() {
        let mut state = empty_state();
        state.guides.push(super::super::state::Guide {
            axis: super::super::state::GuideAxis::Horizontal,
            position_mm: -3.0,
            enabled: true,
        });
        let r = snap_cursor((7.7, -2.7), None, &state, None);
        assert_eq!(r.kind, SnapKind::Guide);
        assert!((r.pos.0 - 7.7).abs() < 1e-9);
        assert!((r.pos.1 - (-3.0)).abs() < 1e-9);
    }

    #[test]
    fn guide_intersection_pins_both_axes() {
        let mut state = empty_state();
        state.guides.push(super::super::state::Guide {
            axis: super::super::state::GuideAxis::Vertical,
            position_mm: 2.0,
            enabled: true,
        });
        state.guides.push(super::super::state::Guide {
            axis: super::super::state::GuideAxis::Horizontal,
            position_mm: 4.0,
            enabled: true,
        });
        let r = snap_cursor((1.9, 4.2), None, &state, None);
        assert_eq!(r.kind, SnapKind::Guide);
        assert!((r.pos.0 - 2.0).abs() < 1e-9);
        assert!((r.pos.1 - 4.0).abs() < 1e-9);
    }

    #[test]
    fn disabled_guide_does_not_snap() {
        let mut state = empty_state();
        state.guides.push(super::super::state::Guide {
            axis: super::super::state::GuideAxis::Vertical,
            position_mm: 5.0,
            enabled: false,
        });
        let r = snap_cursor((4.8, 1.234), None, &state, None);
        assert!(matches!(r.kind, SnapKind::Grid));
    }

    #[test]
    fn global_snap_disabled_short_circuits() {
        // v0.18.25.1 — global status-bar Snap toggle short-circuits
        // every priority including grid + guides + point-hit. The
        // raw cursor passes through unchanged.
        let (sketch, anchor_id) = sketch_with_anchor();
        let mut state = empty_state();
        state.global_snap_disabled = true;
        state.guides.push(super::super::state::Guide {
            axis: super::super::state::GuideAxis::Vertical,
            position_mm: 5.0,
            enabled: true,
        });
        // Cursor near a guide AND a point-hit AND on grid; with
        // global-snap-off, none should fire.
        let r = snap_cursor((4.8, 0.0), Some(&sketch), &state, Some(anchor_id));
        assert!(matches!(r.kind, SnapKind::Raw));
        assert_eq!(r.pos, (4.8, 0.0));
    }
}
