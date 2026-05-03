//! Task 2.5 — PointOnLine / PointOnArc / DistancePtLine residuals.
//!
//! Each helper returns a single scalar residual (`Vec<f64>` of length 1)
//! that the Levenberg–Marquardt driver in Phase 3 will drive to zero.
//!
//! Geometry references:
//!  - Hearn & Baker, *Computer Graphics*, ch. 5 (2D vector geometry,
//!    point-to-line distance via the 2D cross product).
//!  - *Numerical Recipes*, §10.6 (numerically stable scalar forms).
//!
//! All formulas were derived from first-principles 2D vector algebra:
//!  - The signed perpendicular distance from `P = (x0,y0)` to the
//!    infinite line through `A = (x1,y1)` and `B = (x2,y2)` is
//!    `((P−A) × (B−A)) / |B−A|`. In scalar form that expands to
//!    `((y2−y1)·x0 − (x2−x1)·y0 + x2·y1 − y2·x1) / hypot(dx, dy)`,
//!    where `(dx, dy) = (B − A)`.
//!  - For an arc, the underlying circle has radius `|start − center|`,
//!    so `point-on-circle` reduces to `|P − center| − |start − center|`.
//!
//! Sign convention: the perpendicular-distance residual is signed (the
//! 2D cross product gives the "side" of the line as well as the
//! magnitude). The solver needs the sign so it can drive `P` from
//! either side of the line; bake/UI layers take the absolute value
//! when they need an unsigned distance.

use crate::error::SketchError;
use crate::id::SketchEntityId;
use crate::sketch::SketchData;
use crate::solver::state::{arc_refs, line_endpoints, point_xy, EntityIndex};

/// Tolerance below which a line's direction vector is treated as
/// degenerate (zero-length). Anything smaller is a malformed line for
/// which the perpendicular-distance formula has no defined direction.
const DEGENERATE_LEN_EPS: f64 = 1e-12;

/// Signed perpendicular distance from point `(x0, y0)` to the infinite
/// line through `A = (x1, y1)` and `B = (x2, y2)`.
///
/// Implementation uses the 2D cross product
/// `(P − A) × (B − A) = (x0 − x1)·(y2 − y1) − (y0 − y1)·(x2 − x1)`
/// divided by the segment length `hypot(dx, dy)`.
///
/// Returns `None` if `|B − A| < DEGENERATE_LEN_EPS`; the caller maps
/// that to `SketchError::EntityNotFound` for the malformed line.
fn signed_perp_distance(
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
) -> Option<f64> {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = dx.hypot(dy);
    if len < DEGENERATE_LEN_EPS {
        return None;
    }
    let cross = (x0 - x1) * dy - (y0 - y1) * dx;
    Some(cross / len)
}

/// PointOnLine: signed perpendicular distance from `point` to the
/// infinite line through `line`'s endpoints. Zero when `point` sits on
/// the line.
///
/// A degenerate line (`|B − A| < ε`) is treated as a malformed entity
/// and reported via `SketchError::EntityNotFound(line)` — the
/// perpendicular-distance formula has no defined direction in that
/// case, so the residual would otherwise be `NaN`.
pub fn point_on_line(
    point: SketchEntityId,
    line: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    let (s, e) = line_endpoints(line, sketch).ok_or(SketchError::EntityNotFound(line))?;
    let (x0, y0) = point_xy(point, state, index, sketch)
        .ok_or(SketchError::EntityNotFound(point))?;
    let (x1, y1) = point_xy(s, state, index, sketch).ok_or(SketchError::EntityNotFound(s))?;
    let (x2, y2) = point_xy(e, state, index, sketch).ok_or(SketchError::EntityNotFound(e))?;

    let d = signed_perp_distance(x0, y0, x1, y1, x2, y2)
        .ok_or(SketchError::EntityNotFound(line))?;
    Ok(vec![d])
}

/// PointOnArc: distance from `point` to the arc's centre equals the
/// arc's underlying radius. The radius is implied by the arc's start
/// Point — `radius = |start − center|`.
///
/// This residual constrains `point` to lie on the FULL circle through
/// the arc's start; the start/end-sweep envelope is not enforced here
/// (the bake layer handles the sweep envelope when rasterising).
pub fn point_on_arc(
    point: SketchEntityId,
    arc: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    let (center, start, _end, _ccw) =
        arc_refs(arc, sketch).ok_or(SketchError::EntityNotFound(arc))?;
    let (cx, cy) = point_xy(center, state, index, sketch)
        .ok_or(SketchError::EntityNotFound(center))?;
    let (sx, sy) = point_xy(start, state, index, sketch)
        .ok_or(SketchError::EntityNotFound(start))?;
    let (px, py) = point_xy(point, state, index, sketch)
        .ok_or(SketchError::EntityNotFound(point))?;

    let radius = (sx - cx).hypot(sy - cy);
    let dist = (px - cx).hypot(py - cy);
    Ok(vec![dist - radius])
}

/// DistancePtLine: signed perpendicular distance from `point` to the
/// infinite line, minus `target_mm`. Zero when `point` is exactly
/// `target_mm` away on the line's left-hand side (with the cross-
/// product sign convention; right-hand side requires negative target).
///
/// At `target_mm = 0` this reduces exactly to [`point_on_line`].
pub fn distance_pt_line(
    point: SketchEntityId,
    line: SketchEntityId,
    target_mm: f64,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    let (s, e) = line_endpoints(line, sketch).ok_or(SketchError::EntityNotFound(line))?;
    let (x0, y0) = point_xy(point, state, index, sketch)
        .ok_or(SketchError::EntityNotFound(point))?;
    let (x1, y1) = point_xy(s, state, index, sketch).ok_or(SketchError::EntityNotFound(s))?;
    let (x2, y2) = point_xy(e, state, index, sketch).ok_or(SketchError::EntityNotFound(e))?;

    let d = signed_perp_distance(x0, y0, x1, y1, x2, y2)
        .ok_or(SketchError::EntityNotFound(line))?;
    Ok(vec![d - target_mm])
}
