//! Task 2.5 — PointOnLine / PointOnArc / DistancePtLine residuals.
//!
//! Each helper returns a single scalar residual (`Vec<f64>` of length 1)
//! that the Levenberg–Marquardt driver in Phase 3 will drive to zero.
//!
//! Formulas are composed from primitives in [`crate::solver::math`]:
//!  - The signed perpendicular distance from `P` to the infinite
//!    line through `A,B` is `cross(P − A, B − A) / |B − A|` —
//!    `cross` for the signed area, `norm` for the segment length.
//!    The 2D cross product gives the side of the line as well as
//!    the magnitude.
//!  - For an arc, the underlying circle has radius
//!    `|start − center|`, so the point-on-arc residual reduces to
//!    `|P − center| − |start − center|`.
//!
//! Sign convention: the perpendicular-distance residual is signed
//! (the 2D cross product gives the "side" of the line as well as
//! the magnitude). The solver needs the sign so it can drive `P`
//! from either side of the line; bake/UI layers take the absolute
//! value when they need an unsigned distance.

use crate::entity::EntityKind;
use crate::error::SketchError;
use crate::id::SketchEntityId;
use crate::sketch::SketchData;
use crate::solver::math::{Vec2, cross, distance, norm, sub};
use crate::solver::state::{EntityIndex, arc_refs, circle_radius, line_endpoints, point_xy};

/// Tolerance below which a line's direction vector is treated as
/// degenerate (zero-length). Anything smaller is a malformed line for
/// which the perpendicular-distance formula has no defined direction.
pub(super) const DEGENERATE_LEN_EPS: f64 = 1e-12;

/// Signed perpendicular distance from point `p` to the infinite line
/// through `a, b`. `None` if `|b − a| < DEGENERATE_LEN_EPS` (line is
/// degenerate; perpendicular has no defined direction).
fn signed_perp_distance(p: Vec2, a: Vec2, b: Vec2) -> Option<f64> {
    let d = sub(b, a);
    let len = norm(d);
    if len < DEGENERATE_LEN_EPS {
        return None;
    }
    Some(cross(sub(p, a), d) / len)
}

/// Resolve `(point_xy, line_endpoints[0], line_endpoints[1])` for a
/// `point + line` constraint. Centralises the three `EntityNotFound`
/// error sites so [`point_on_line`] / [`distance_pt_line`] stay
/// short.
fn point_and_line(
    point: SketchEntityId,
    line: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<(Vec2, Vec2, Vec2), SketchError> {
    let (s, e) = line_endpoints(line, sketch).ok_or(SketchError::EntityNotFound(line))?;
    let p = point_xy(point, state, index, sketch).ok_or(SketchError::EntityNotFound(point))?;
    let a = point_xy(s, state, index, sketch).ok_or(SketchError::EntityNotFound(s))?;
    let b = point_xy(e, state, index, sketch).ok_or(SketchError::EntityNotFound(e))?;
    Ok((p, a, b))
}

/// PointOnLine: signed perpendicular distance from `point` to the
/// infinite line through `line`'s endpoints. Zero when `point` sits
/// on the line.
///
/// A degenerate line (`|B − A| < ε`) is treated as a malformed
/// entity and reported via `SketchError::EntityNotFound(line)`.
pub fn point_on_line(
    point: SketchEntityId,
    line: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    let (p, a, b) = point_and_line(point, line, state, index, sketch)?;
    let d = signed_perp_distance(p, a, b).ok_or(SketchError::EntityNotFound(line))?;
    Ok(vec![d])
}

/// PointOnArc: distance from `point` to the arc's centre equals the
/// arc's underlying radius. The radius is implied by the arc's start
/// Point — `radius = |start − center|`.
///
/// This residual constrains `point` to lie on the FULL circle
/// through the arc's start; the start/end-sweep envelope is enforced
/// by the bake layer when rasterising.
pub fn point_on_arc(
    point: SketchEntityId,
    arc: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    let (center, start, _end, _ccw) =
        arc_refs(arc, sketch).ok_or(SketchError::EntityNotFound(arc))?;
    let c = point_xy(center, state, index, sketch).ok_or(SketchError::EntityNotFound(center))?;
    let s = point_xy(start, state, index, sketch).ok_or(SketchError::EntityNotFound(start))?;
    let p = point_xy(point, state, index, sketch).ok_or(SketchError::EntityNotFound(point))?;
    Ok(vec![distance(p, c) - distance(s, c)])
}

/// DistancePtLine: signed perpendicular distance from `point` to the
/// infinite line, minus `target_mm`. Zero when `point` is exactly
/// `target_mm` away on the line's right-hand side (cross-product
/// sign convention; left-hand side requires negative target).
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
    let (p, a, b) = point_and_line(point, line, state, index, sketch)?;
    let d = signed_perp_distance(p, a, b).ok_or(SketchError::EntityNotFound(line))?;
    Ok(vec![d - target_mm])
}

/// v0.23 — DistancePtCircle: signed offset from `point` to the
/// boundary of `circle`. Residual is `|p - centre| - radius - target`.
/// `target = 0` reduces to "point on the circle". Positive target
/// offsets outward (further from centre); negative offsets inward.
///
/// Works on both `EntityKind::Circle { center, radius }` and
/// `EntityKind::Arc` — for arcs, the radius is derived from the
/// `start` point's distance to `center` (matching `point_on_arc`
/// semantics).
pub fn distance_pt_circle(
    point: SketchEntityId,
    circle: SketchEntityId,
    target_mm: f64,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    let p = point_xy(point, state, index, sketch).ok_or(SketchError::EntityNotFound(point))?;
    let entity = sketch
        .entities
        .iter()
        .find(|e| e.id == circle)
        .ok_or(SketchError::EntityNotFound(circle))?;
    let (centre_id, radius) = match entity.kind {
        EntityKind::Circle { center, .. } => {
            let r = circle_radius(circle, state, index)
                .ok_or(SketchError::EntityNotFound(circle))?;
            (center, r)
        }
        EntityKind::Arc { center, start, .. } => {
            let c = point_xy(center, state, index, sketch)
                .ok_or(SketchError::EntityNotFound(center))?;
            let s = point_xy(start, state, index, sketch)
                .ok_or(SketchError::EntityNotFound(start))?;
            (center, distance(s, c))
        }
        _ => return Err(SketchError::EntityNotFound(circle)),
    };
    let c = point_xy(centre_id, state, index, sketch)
        .ok_or(SketchError::EntityNotFound(centre_id))?;
    Ok(vec![distance(p, c) - radius - target_mm])
}
