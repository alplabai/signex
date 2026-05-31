//! Task 2.7 — SymmetricAboutLine / SymmetricAboutPoint / Midpoint
//! residuals.
//!
//! All three are derived from first-principles 2D vector geometry
//! and composed from primitives in [`crate::solver::math`].

use crate::error::SketchError;
use crate::id::SketchEntityId;
use crate::sketch::SketchData;
use crate::solver::math::{add, cross, dot, norm_sq_2, scale, sub};
use crate::solver::state::{EntityIndex, line_endpoints, point_xy};

/// Residual for `p1` and `p2` being mirror images of each other
/// across the infinite line through the line entity's endpoints
/// `A, B`. Returns two scalars:
///
/// 1. **midpoint-on-line**: signed perpendicular distance from the
///    midpoint `M = (p1 + p2) / 2` to the line. Computed as the 2D
///    cross product `cross(M − A, d) / |d|`, where `d = B − A`.
///    Zero iff `M` lies on the line.
/// 2. **perpendicular-to-line**: the dot product `dot(p2 − p1, d)`.
///    Zero iff the segment `p1p2` is perpendicular to the line
///    direction.
///
/// Together these two scalars vanish iff `p1` and `p2` are
/// reflections of each other across the line.
///
/// Order: `[midpoint_on_line, perp_to_line]`.
pub fn symmetric_about_line(
    p1: SketchEntityId,
    p2: SketchEntityId,
    line: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    let (a_id, b_id) = line_endpoints(line, sketch).ok_or(SketchError::EntityNotFound(line))?;

    let p1v = point_xy(p1, state, index, sketch).ok_or(SketchError::EntityNotFound(p1))?;
    let p2v = point_xy(p2, state, index, sketch).ok_or(SketchError::EntityNotFound(p2))?;
    let a = point_xy(a_id, state, index, sketch).ok_or(SketchError::EntityNotFound(a_id))?;
    let b = point_xy(b_id, state, index, sketch).ok_or(SketchError::EntityNotFound(b_id))?;

    let d = sub(b, a);
    let m = scale(0.5, add(p1v, p2v));

    // 2D cross of (M − A) and d. When the line is non-degenerate,
    // dividing by |d| yields the signed perpendicular distance.
    // When |d| is effectively zero the line direction is undefined;
    // fall back to the raw cross so the solver still has a finite
    // (non-NaN) gradient to push on.
    let raw_cross = cross(sub(m, a), d);
    let len_sq = norm_sq_2(d);
    let midpoint_on_line = if len_sq > 0.0 {
        raw_cross / len_sq.sqrt()
    } else {
        raw_cross
    };

    // Dot product of (p2 − p1) with d. Zero iff p2 − p1 ⟂ d.
    let perp_to_line = dot(sub(p2v, p1v), d);

    Ok(vec![midpoint_on_line, perp_to_line])
}

/// Residual for `p1` and `p2` being mirror images about the centre
/// point `C` (i.e. `C` is exactly the midpoint of `p1, p2`).
///
/// Returns two scalars:
/// 1. `(p1.x + p2.x) / 2 − C.x`
/// 2. `(p1.y + p2.y) / 2 − C.y`
///
/// Order: `[mid_x_eq, mid_y_eq]`.
pub fn symmetric_about_point(
    p1: SketchEntityId,
    p2: SketchEntityId,
    center: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    let p1v = point_xy(p1, state, index, sketch).ok_or(SketchError::EntityNotFound(p1))?;
    let p2v = point_xy(p2, state, index, sketch).ok_or(SketchError::EntityNotFound(p2))?;
    let c = point_xy(center, state, index, sketch).ok_or(SketchError::EntityNotFound(center))?;

    let mid = scale(0.5, add(p1v, p2v));
    let r = sub(mid, c);
    Ok(vec![r.0, r.1])
}

/// Residual for point `P` being the midpoint of the line entity's
/// endpoints `A, B`.
///
/// Returns two scalars:
/// 1. `P.x − (A.x + B.x) / 2`
/// 2. `P.y − (A.y + B.y) / 2`
///
/// Order: `[mid_x_match, mid_y_match]`.
pub fn midpoint(
    point: SketchEntityId,
    line: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    let (a_id, b_id) = line_endpoints(line, sketch).ok_or(SketchError::EntityNotFound(line))?;

    let p = point_xy(point, state, index, sketch).ok_or(SketchError::EntityNotFound(point))?;
    let a = point_xy(a_id, state, index, sketch).ok_or(SketchError::EntityNotFound(a_id))?;
    let b = point_xy(b_id, state, index, sketch).ok_or(SketchError::EntityNotFound(b_id))?;

    let mid = scale(0.5, add(a, b));
    let r = sub(p, mid);
    Ok(vec![r.0, r.1])
}
