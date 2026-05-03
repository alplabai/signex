//! Task 2.7 — SymmetricAboutLine / SymmetricAboutPoint / Midpoint
//! residuals.
//!
//! All three are derived from first-principles 2D vector geometry
//! (Hearn & Baker, basic linear algebra). No third-party
//! constraint-solver source was consulted.

use crate::error::SketchError;
use crate::id::SketchEntityId;
use crate::sketch::SketchData;
use crate::solver::state::{line_endpoints, point_xy, EntityIndex};

/// Residual for `p1` and `p2` being mirror images of each other
/// across the infinite line through the line entity's endpoints
/// `A, B`. Returns two scalars:
///
/// 1. **midpoint-on-line**: signed perpendicular distance from the
///    midpoint `M = (p1 + p2) / 2` to the line. Computed as the 2D
///    cross product `(M − A) × d` divided by `|d|`, where
///    `d = B − A`. Zero iff `M` lies on the line.
/// 2. **perpendicular-to-line**: the dot product
///    `(p2 − p1) · d`. Zero iff the segment `p1p2` is perpendicular
///    to the line direction.
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

    let (x1, y1) = point_xy(p1, state, index, sketch).ok_or(SketchError::EntityNotFound(p1))?;
    let (x2, y2) = point_xy(p2, state, index, sketch).ok_or(SketchError::EntityNotFound(p2))?;
    let (ax, ay) = point_xy(a_id, state, index, sketch).ok_or(SketchError::EntityNotFound(a_id))?;
    let (bx, by) = point_xy(b_id, state, index, sketch).ok_or(SketchError::EntityNotFound(b_id))?;

    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;

    let mx = 0.5 * (x1 + x2);
    let my = 0.5 * (y1 + y2);

    // 2D cross of (M − A) and d. When the line is non-degenerate,
    // dividing by |d| yields the signed perpendicular distance.
    // When |d| is effectively zero the line direction is undefined;
    // fall back to the raw cross so the solver still has a finite
    // (non-nan) gradient to push on.
    let cross = (mx - ax) * dy - (my - ay) * dx;
    let midpoint_on_line = if len_sq > 0.0 {
        cross / len_sq.sqrt()
    } else {
        cross
    };

    // Dot product of (p2 − p1) with d. Zero iff p2 − p1 ⟂ d.
    let perp_to_line = (x2 - x1) * dx + (y2 - y1) * dy;

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
    let (x1, y1) = point_xy(p1, state, index, sketch).ok_or(SketchError::EntityNotFound(p1))?;
    let (x2, y2) = point_xy(p2, state, index, sketch).ok_or(SketchError::EntityNotFound(p2))?;
    let (cx, cy) =
        point_xy(center, state, index, sketch).ok_or(SketchError::EntityNotFound(center))?;

    let mid_x_eq = 0.5 * (x1 + x2) - cx;
    let mid_y_eq = 0.5 * (y1 + y2) - cy;

    Ok(vec![mid_x_eq, mid_y_eq])
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

    let (px, py) =
        point_xy(point, state, index, sketch).ok_or(SketchError::EntityNotFound(point))?;
    let (ax, ay) = point_xy(a_id, state, index, sketch).ok_or(SketchError::EntityNotFound(a_id))?;
    let (bx, by) = point_xy(b_id, state, index, sketch).ok_or(SketchError::EntityNotFound(b_id))?;

    let mid_x_match = px - 0.5 * (ax + bx);
    let mid_y_match = py - 0.5 * (ay + by);

    Ok(vec![mid_x_match, mid_y_match])
}
