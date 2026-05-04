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

use signex_sketch::id::SketchEntityId;
use signex_sketch::SketchData;

use super::state::{FootprintEditorState, ToolPending};

/// Default grid step for free-canvas snap (mm).
///
/// v0.18.7.2 — bumped from 0.1mm to 1.0mm so snap-to-grid is
/// visibly effective in Pads mode. With 0.1mm the snap fired but
/// rounded to a position the user couldn't tell apart from the raw
/// click. 1.0mm matches the typical Altium PCB Library grid step
/// (50mil ≈ 1.27mm — close enough for default; the v0.18.10 Snap
/// Distance numeric input will let the user dial it in).
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
    /// Fell through to grid snap.
    Grid,
}

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
                    let angle_diff = ((angle - snapped_angle).abs()).min(
                        (std::f64::consts::TAU - (angle - snapped_angle).abs()).abs(),
                    );
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
    if opts.grid {
        let snapped = (
            (raw.0 / GRID_STEP_MM).round() * GRID_STEP_MM,
            (raw.1 / GRID_STEP_MM).round() * GRID_STEP_MM,
        );
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
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_sketch::SketchData;

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
}
