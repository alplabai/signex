//! v0.16.1 — Fusion-style cursor snap for the footprint sketch
//! canvas.
//!
//! Priority chain (highest first):
//! 1. **Point snap** — cursor within `SKETCH_SNAP_RADIUS_PX` of an
//!    existing `Point` entity → snap to that Point's position (a Circle's
//!    or Arc's centre reads as a Centre snap while a drawing tool infers).
//!    Then, on a board sketch only, the board under it
//!    ([`super::snap_board`]).
//! 2. **Midpoint / Centre** — a Line's midpoint or a Circle's / Arc's
//!    centre within the same radius, while a drawing tool is armed (mockup row 188).
//! 3. **Guide** (behind the Guides switch), then **Intersection** of two
//!    sketch curves.
//! 4. **Horizontal / Vertical inference** — when a multi-click tool
//!    is mid-gesture (line first endpoint, rect first corner, ...),
//!    if the cursor's angle relative to the anchor falls within
//!    [`AXIS_THRESHOLD_DEG`] of horizontal or vertical, snap to the
//!    exact axis.
//! 5. **Angle snap** — same anchor-relative angle, snap to nearest
//!    [`ANGLE_STEP_DEG`] increment if within
//!    [`ANGLE_THRESHOLD_DEG`].
//! 6. **Axes** — line up with a footprint object in Axis Snap Range
//!    ([`super::snap_objects::axis_snap`]).
//! 7. **Grid snap** — fall through: round each axis to the nearest
//!    [`GRID_STEP_MM`] increment.
//!
//! Between 1 and 2 the footprint's own objects snap
//! ([`super::snap_objects`]).
//!
//! [`snap_cursor_with`] with `infer = false` (Ctrl held while drawing)
//! skips Midpoint, Centre, Intersection, H / V and Angle: point reuse,
//! the board, guides and grid stay. The inference a click lands from its
//! snap is [`click_inference`].

use signex_sketch::SketchData;
use signex_sketch::entity::EntityKind;
use signex_sketch::geom::{Arc2, Circle2, Point2, Segment2};
use signex_sketch::id::SketchEntityId;

use super::state::{EditorMode, FootprintEditorState, ToolPending};

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
    /// Snapped onto a feature of the board under a board sketch (#2518):
    /// places the point only and lands no constraint. See
    /// [`super::snap_board`].
    BoardFeature(super::snap_board::BoardFeatureKind),
    /// The midpoint of the sketch Line with this id (mockup row 188).
    Midpoint(SketchEntityId),
    /// The centre of the sketch Circle or Arc with this id (mockup row 188).
    Centre(SketchEntityId),
    /// A footprint object hotspot or an Axes alignment with one.
    /// See [`super::snap_objects`].
    Object,
}

/// The constraint a drawing-tool click lands from the snap it took
/// (mockup row 188), in the same undo step as the click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SketchInference {
    /// Ctrl was held: no inferred constraint, the Line's auto H / V
    /// included.
    Off,
    /// A Midpoint constraint on this Line.
    Midpoint(SketchEntityId),
    /// A Coincident with the centre of this Circle or Arc.
    Centre(SketchEntityId),
    /// The point lies on both curves: Point on Line, Point on Arc, or zero
    /// distance to a Circle.
    Intersection(SketchEntityId, SketchEntityId),
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
        // #467 — Edge Arc anchors to the most recently placed point:
        // `start` while picking `end`, then `end` while picking the
        // third "point on arc" click.
        ToolPending::EdgeArcStart { start } => start,
        ToolPending::EdgeArcEnd { end, .. } => end,
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
        ToolPending::FilletFirst { .. } | ToolPending::ChamferFirst { .. } => return None,
        // A slot's second centre runs from its first; its width click and
        // a polygon's first vertex run from no anchor.
        ToolPending::SlotFirst { a } => a,
        ToolPending::SlotSecond { .. } | ToolPending::PolygonCentre { .. } => return None,
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

/// [`snap_cursor_with`] with inference on: the snap of every caller that
/// is not a drawing tool under Ctrl.
pub fn snap_cursor(
    raw: (f64, f64),
    sketch: Option<&SketchData>,
    state: &FootprintEditorState,
    point_hit: Option<SketchEntityId>,
) -> SnapResult {
    snap_cursor_with(raw, sketch, state, point_hit, true)
}

/// Apply the priority chain to `raw` and return the snapped
/// position + the snap kind that fired. `point_hit` is the optional
/// outcome of a prior call to the canvas's `sketch_snap` (we don't
/// re-implement the spatial query here so the canvas's existing
/// px-radius logic stays the source of truth). `infer = false` skips the
/// snaps that land a constraint (see the module doc).
pub fn snap_cursor_with(
    raw: (f64, f64),
    sketch: Option<&SketchData>,
    state: &FootprintEditorState,
    point_hit: Option<SketchEntityId>,
    infer: bool,
) -> SnapResult {
    snap_cursor_among(raw, sketch, state, point_hit, infer, None)
}

/// [`snap_cursor_with`] that also reaches the footprint's own objects and
/// their Axes alignment, the canvas's snap.
pub fn snap_cursor_among(
    raw: (f64, f64),
    sketch: Option<&SketchData>,
    state: &FootprintEditorState,
    point_hit: Option<SketchEntityId>,
    infer: bool,
    objects: Option<super::snap_objects::ObjectTargets<'_>>,
) -> SnapResult {
    // Snapping Off, or the status bar's global Snap toggle
    // (`global_snap_disabled`), turns every priority off.
    // `CurrentLayer` behaves as `AllLayers` until a layer-aware filter.
    use super::state::SnappingMode;
    if state.global_snap_disabled || state.snapping_mode == SnappingMode::Off {
        return SnapResult::raw(raw);
    }
    // v0.17.0 — each priority is gated by `state.snap_options.<flag>`.
    // Disabled priorities pass through to the next; if all four are
    // disabled the cursor returns raw (matches Altium's "no snap"
    // workflow).
    let opts = state.snap_options;
    let drawing = infer && draws_with_inference(state);
    // A board sketch's grid steps from the board origin.
    let grid_from = state.grid_origin_mm().unwrap_or((0.0, 0.0));

    // Priority 1 — Point snap.
    if opts.point_hit
        && let Some(id) = point_hit
        && let Some(pos) = point_pos(id, sketch, state)
    {
        if drawing
            && opts.snap_centres
            && let Some(curve) = sketch.and_then(|s| curve_centred_on(s, id))
        {
            return SnapResult {
                pos,
                kind: SnapKind::Centre(curve),
            };
        }
        return SnapResult {
            pos,
            kind: SnapKind::Point(id),
        };
    }

    // Priority 1.2 — the board under a board sketch (#2518). `None` on
    // a footprint sketch, which has no board.
    if let Some(hit) = super::snap_board::board_feature_snap(raw, state) {
        return hit;
    }

    // Priority 1.25 — the footprint's own objects (Sketch mode: those the
    // sketch does not draw).
    if let Some(hit) = objects.and_then(|o| super::snap_objects::object_snap(raw, state, o)) {
        return hit;
    }

    // Priority 1.3 — a Line's midpoint, a Circle's or Arc's centre.
    if drawing
        && let Some(sketch) = sketch
        && let Some(hit) = midpoint_or_centre(raw, sketch, state)
    {
        return hit;
    }

    // Priority 1.5 — Guide snap, behind the Guides switch (#2726). Snaps
    // the cursor onto enabled vertical / horizontal guides whose axis is
    // within `GUIDE_SNAP_TOLERANCE_MM`. When both fire, snap both axes (a
    // guide-intersection lock).
    if opts.snap_to_guides && !state.guides.is_empty() {
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

    // v0.27 — Intersection snap: the closest crossing of two sketch
    // curves within `snap_distance_mm` ([`nearest_intersection`]).
    if infer
        && opts.snap_intersections
        && let Some(sketch) = sketch
        && let Some((pos, _, _)) = nearest_intersection(raw, sketch, state)
    {
        return SnapResult {
            pos,
            kind: SnapKind::Intersection,
        };
    }

    // Priority 2 + 3 — anchor-relative angle snap (H/V/15°).
    if infer
        && (opts.horizontal_vertical || opts.angle)
        && let Some(anchor) = anchor_for_tool(state, sketch)
    {
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
            // The free axis (H / V) and the ray distance (angle) snap to
            // the grid too, so a snapped point is on the axis and on the
            // grid at once. No-op when grid snap is disabled.
            let grid_snap = |v: f64, from: f64| -> f64 {
                if opts.snaps_to_grid() {
                    on_grid(v, from, opts.grid_step_mm)
                } else {
                    v
                }
            };
            if opts.horizontal_vertical && angle.sin().abs() < axis_thresh {
                // Horizontal: lock Y to the anchor, snap the free X
                // axis to grid. Using the cursor's own X (not
                // anchor + hypotenuse) avoids the perpendicular
                // component leaking into the length.
                let pos = (grid_snap(raw.0, grid_from.0), anchor.1);
                return SnapResult {
                    pos,
                    kind: SnapKind::Horizontal,
                };
            }
            if opts.horizontal_vertical && angle.cos().abs() < axis_thresh {
                // Vertical: lock X to the anchor, snap the free Y.
                let pos = (anchor.0, grid_snap(raw.1, grid_from.1));
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
                    // Hold the snapped azimuth exactly; step the
                    // distance along the ray in grid increments so
                    // the endpoint lands on a grid-multiple radius
                    // (a true diagonal can't sit on both the ray
                    // and an XY grid node, so grid the distance).
                    let snapped_dist = grid_snap(dist, 0.0);
                    let pos = (
                        anchor.0 + snapped_dist * snapped_angle.cos(),
                        anchor.1 + snapped_dist * snapped_angle.sin(),
                    );
                    return SnapResult {
                        pos,
                        kind: SnapKind::Angle(snapped_angle),
                    };
                }
            }
        }
    }

    // Priority 3.5 — Axes: line up with a footprint object in Axis Snap
    // Range; the axis nothing lines up on takes the grid (#2726).
    if let Some(objects) = objects {
        let (x, y) = super::snap_objects::axis_snap(raw, state, objects);
        if x.is_some() || y.is_some() {
            let free = |v: f64, from: f64| {
                if opts.snaps_to_grid() {
                    on_grid(v, from, opts.grid_step_mm)
                } else {
                    v
                }
            };
            return SnapResult {
                pos: (
                    x.unwrap_or_else(|| free(raw.0, grid_from.0)),
                    y.unwrap_or_else(|| free(raw.1, grid_from.1)),
                ),
                kind: SnapKind::Object,
            };
        }
    }

    // Priority 4 — grid snap. When disabled, the raw cursor passes
    // through unchanged (mirrors Altium's "Smart Snap → Off" flow).
    // v0.18.9 — uses `opts.grid_step_mm` (configurable) instead of
    // a hardcoded constant. Defends against zero/negative steps by
    // falling through to raw — a misconfigured step shouldn't crash
    // the canvas or move pads to the origin.
    if opts.snaps_to_grid() {
        let step = opts.grid_step_mm;
        let snapped = (
            on_grid(raw.0, grid_from.0, step),
            on_grid(raw.1, grid_from.1, step),
        );
        SnapResult {
            pos: snapped,
            kind: SnapKind::Grid,
        }
    } else {
        SnapResult::raw(raw)
    }
}

/// The point of the grid through `from`, of pitch `step`, nearest `v`.
fn on_grid(v: f64, from: f64, step: f64) -> f64 {
    if from == 0.0 {
        (v / step).round() * step
    } else {
        from + ((v - from) / step).round() * step
    }
}

/// Whether the snap may land a constraint: a sketch drawing tool is armed.
fn draws_with_inference(state: &FootprintEditorState) -> bool {
    state.mode == EditorMode::Sketch && state.active_tool.is_drawing()
}

/// The Circle or Arc whose centre is the Point `id`, if any.
fn curve_centred_on(sketch: &SketchData, id: SketchEntityId) -> Option<SketchEntityId> {
    sketch.entities.iter().find_map(|e| match e.kind {
        EntityKind::Circle { center, .. } | EntityKind::Arc { center, .. } if center == id => {
            Some(e.id)
        }
        _ => None,
    })
}

/// The nearest Line midpoint or Circle / Arc centre within the Point
/// snap's reach of `raw`, each gated by its Snap ▾ switch.
fn midpoint_or_centre(
    raw: (f64, f64),
    sketch: &SketchData,
    state: &FootprintEditorState,
) -> Option<SnapResult> {
    let opts = state.snap_options;
    let reach = super::snap_board::reach_mm(state.view_px_per_mm);
    let at = |id| point_pos(id, Some(sketch), state);
    let mut best: Option<(f64, SnapResult)> = None;
    for e in &sketch.entities {
        let target = match e.kind {
            EntityKind::Line { start, end } if opts.snap_midpoints => {
                let (Some(s), Some(f)) = (at(start), at(end)) else {
                    continue;
                };
                (
                    ((s.0 + f.0) / 2.0, (s.1 + f.1) / 2.0),
                    SnapKind::Midpoint(e.id),
                )
            }
            EntityKind::Circle { center, .. } | EntityKind::Arc { center, .. }
                if opts.snap_centres =>
            {
                let Some(c) = at(center) else {
                    continue;
                };
                (c, SnapKind::Centre(e.id))
            }
            _ => continue,
        };
        let d = (target.0.0 - raw.0).hypot(target.0.1 - raw.1);
        if d <= reach && best.is_none_or(|(b, _)| d < b) {
            best = Some((
                d,
                SnapResult {
                    pos: target.0,
                    kind: target.1,
                },
            ));
        }
    }
    best.map(|(_, hit)| hit)
}

/// The closest crossing of two sketch curves within `snap_distance_mm` of
/// `raw`, with the two curves' ids. Line × Line, Line × Arc, Line ×
/// Circle, then Circle × Circle, Arc × Arc and Arc × Circle; a later
/// crossing wins only when strictly closer.
pub fn nearest_intersection(
    raw: (f64, f64),
    sketch: &SketchData,
    state: &FootprintEditorState,
) -> Option<((f64, f64), SketchEntityId, SketchEntityId)> {
    use signex_sketch::geom::{
        SegmentIntersection, arc_arc_intersections, arc_circle_intersections,
        circle_circle_intersections, segment_arc_intersections, segment_circle_intersections,
        segment_segment_intersection,
    };
    let resolve = |id: SketchEntityId| point_pos(id, Some(sketch), state);
    let mut lines: Vec<(SketchEntityId, Segment2)> = Vec::new();
    let mut circles: Vec<(SketchEntityId, Circle2)> = Vec::new();
    let mut arcs: Vec<(SketchEntityId, Arc2)> = Vec::new();
    for e in &sketch.entities {
        match e.kind {
            EntityKind::Line { start, end } => {
                if let (Some(s), Some(f)) = (resolve(start), resolve(end)) {
                    let seg = Segment2::new(Point2::new(s.0, s.1), Point2::new(f.0, f.1));
                    lines.push((e.id, seg));
                }
            }
            EntityKind::Circle { center, radius } => {
                if let Some(c) = resolve(center) {
                    circles.push((e.id, Circle2::new(Point2::new(c.0, c.1), radius)));
                }
            }
            EntityKind::Arc {
                center,
                start,
                end,
                sweep_ccw,
            } => {
                if let (Some(c), Some(s), Some(f)) = (resolve(center), resolve(start), resolve(end))
                {
                    let radius = ((s.0 - c.0).powi(2) + (s.1 - c.1).powi(2)).sqrt();
                    let start_rad = (s.1 - c.1).atan2(s.0 - c.0);
                    let end_rad = (f.1 - c.1).atan2(f.0 - c.0);
                    let arc =
                        Arc2::new(Point2::new(c.0, c.1), radius, start_rad, end_rad, sweep_ccw);
                    arcs.push((e.id, arc));
                }
            }
            EntityKind::Point { .. } => {}
        }
    }

    let tol = state.snap_options.snap_distance_mm.max(1e-6);
    let mut best: Option<(f64, (f64, f64), SketchEntityId, SketchEntityId)> = None;
    let mut consider = |pt: Point2, a: SketchEntityId, b: SketchEntityId| {
        let d_sq = (pt.x - raw.0).powi(2) + (pt.y - raw.1).powi(2);
        if d_sq <= tol * tol && best.is_none_or(|(b_sq, ..)| d_sq < b_sq) {
            best = Some((d_sq, (pt.x, pt.y), a, b));
        }
    };
    for (i, &(a, seg_a)) in lines.iter().enumerate() {
        for &(b, seg_b) in &lines[i + 1..] {
            if let SegmentIntersection::Point { pt, .. } =
                segment_segment_intersection(seg_a, seg_b)
            {
                consider(pt, a, b);
            }
        }
    }
    for &(a, seg) in &lines {
        for &(b, arc) in &arcs {
            for (pt, _) in segment_arc_intersections(seg, arc) {
                consider(pt, a, b);
            }
        }
    }
    for &(a, seg) in &lines {
        for &(b, circle) in &circles {
            for (pt, _) in segment_circle_intersections(seg, circle) {
                consider(pt, a, b);
            }
        }
    }
    for (i, &(a, ca)) in circles.iter().enumerate() {
        for &(b, cb) in &circles[i + 1..] {
            for pt in circle_circle_intersections(ca, cb) {
                consider(pt, a, b);
            }
        }
    }
    for (i, &(a, arc_a)) in arcs.iter().enumerate() {
        for &(b, arc_b) in &arcs[i + 1..] {
            for pt in arc_arc_intersections(arc_a, arc_b) {
                consider(pt, a, b);
            }
        }
    }
    for &(a, arc) in &arcs {
        for &(b, circle) in &circles {
            for pt in arc_circle_intersections(arc, circle) {
                consider(pt, a, b);
            }
        }
    }
    best.map(|(_, pos, a, b)| (pos, a, b))
}

/// The inference a sketch click at `click` lands, from the snap `snap`
/// the press took there: `Off` while Ctrl is held with a drawing tool
/// armed, a Midpoint / Centre / Intersection when that snap fired at the
/// click, else `None` (a plain click: no inference beyond a Line's auto
/// H / V).
pub fn click_inference(
    snap: Option<SnapResult>,
    click: (f64, f64),
    sketch: Option<&SketchData>,
    state: &FootprintEditorState,
    ctrl: bool,
) -> Option<SketchInference> {
    if !state.active_tool.is_drawing() {
        return None;
    }
    if ctrl {
        return Some(SketchInference::Off);
    }
    let snap =
        snap.filter(|s| (s.pos.0 - click.0).abs() < 1e-9 && (s.pos.1 - click.1).abs() < 1e-9)?;
    match snap.kind {
        SnapKind::Midpoint(line) => Some(SketchInference::Midpoint(line)),
        SnapKind::Centre(curve) => Some(SketchInference::Centre(curve)),
        SnapKind::Intersection => nearest_intersection(click, sketch?, state)
            .map(|(_, a, b)| SketchInference::Intersection(a, b)),
        _ => None,
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
    fn axis_snap_also_grid_snaps_free_axis() {
        // v0.14-footprint — a near-horizontal cursor whose X sits off
        // the 1 mm grid must snap to BOTH the anchor's Y (axis lock)
        // AND the nearest grid X. Regression for "snapped to an angle
        // but not to grid".
        let (sketch, anchor_id) = sketch_with_anchor();
        let mut state = empty_state();
        state.tool_pending = ToolPending::LineFirst { first: anchor_id };
        // ~1° off horizontal; cursor X = 5.4 (off the 1 mm grid).
        let raw = (5.4, 5.4 * 1.0_f64.to_radians().tan());
        let r = snap_cursor(raw, Some(&sketch), &state, None);
        assert_eq!(r.kind, SnapKind::Horizontal);
        assert!(
            (r.pos.0 - 5.0).abs() < 1e-9,
            "free X axis must grid-snap to 5.0; got {}",
            r.pos.0
        );
        assert!(
            r.pos.1.abs() < 1e-9,
            "locked Y stays on the anchor; got {}",
            r.pos.1
        );
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
