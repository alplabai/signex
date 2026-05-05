//! Task 2.6 — EqualLength / EqualRadius / TangentLineArc / TangentArcArc
//! residuals.
//!
//! Each helper returns a single scalar residual (`Vec<f64>` of length 1)
//! that the Levenberg–Marquardt driver in Phase 3 will drive to zero.
//!
//! Formulas compose from primitives in [`crate::solver::math`]:
//!  - Line length = `norm(end − start)`.
//!  - Circle radius is stored explicitly; arc radius is
//!    `distance(start, center)`.
//!  - Line/arc tangency: the signed perpendicular distance from the
//!    arc centre to the line equals the arc radius (in absolute
//!    value, since the line can sit on either side of the centre).
//!  - Arc/arc tangency:
//!      external — `|C2 − C1| = r1 + r2`
//!      internal — `|C2 − C1| = |r1 − r2|`

use crate::entity::EntityKind;
use crate::error::SketchError;
use crate::id::SketchEntityId;
use crate::sketch::SketchData;
use crate::solver::math::{Vec2, cross, distance, norm, sub};
use crate::solver::state::{
    EntityIndex, arc_refs, circle_radius, find_entity, line_endpoints, point_xy,
};

/// Resolve the length of a line from the current state vector.
fn line_length(
    line: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<f64, SketchError> {
    let (s, e) = line_endpoints(line, sketch).ok_or(SketchError::EntityNotFound(line))?;
    let p_s = point_xy(s, state, index, sketch).ok_or(SketchError::EntityNotFound(s))?;
    let p_e = point_xy(e, state, index, sketch).ok_or(SketchError::EntityNotFound(e))?;
    Ok(norm(sub(p_e, p_s)))
}

/// Resolve the radius of a Circle (from state vector) or an Arc
/// (computed as `|start − center|`). Returns `EntityNotFound` if the
/// entity is neither.
fn entity_radius(
    id: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<f64, SketchError> {
    let entity = find_entity(id, sketch).ok_or(SketchError::EntityNotFound(id))?;
    match entity.kind {
        EntityKind::Circle { .. } => {
            circle_radius(id, state, index).ok_or(SketchError::EntityNotFound(id))
        }
        EntityKind::Arc { .. } => {
            let (center, start, _end, _ccw) =
                arc_refs(id, sketch).ok_or(SketchError::EntityNotFound(id))?;
            let c = point_xy(center, state, index, sketch)
                .ok_or(SketchError::EntityNotFound(center))?;
            let s =
                point_xy(start, state, index, sketch).ok_or(SketchError::EntityNotFound(start))?;
            Ok(distance(s, c))
        }
        _ => Err(SketchError::EntityNotFound(id)),
    }
}

/// Resolve the centre `(x, y)` of an Arc or Circle.
fn entity_center_xy(
    id: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec2, SketchError> {
    let entity = find_entity(id, sketch).ok_or(SketchError::EntityNotFound(id))?;
    let center_id = match entity.kind {
        EntityKind::Circle { center, .. } => center,
        EntityKind::Arc { center, .. } => center,
        _ => return Err(SketchError::EntityNotFound(id)),
    };
    point_xy(center_id, state, index, sketch).ok_or(SketchError::EntityNotFound(center_id))
}

/// EqualLength: `|d2| − |d1| = 0`.
pub fn equal_length(
    l1: SketchEntityId,
    l2: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    let len1 = line_length(l1, state, index, sketch)?;
    let len2 = line_length(l2, state, index, sketch)?;
    Ok(vec![len2 - len1])
}

/// EqualRadius: `r2 − r1 = 0`. Each entity may be a Circle or an
/// Arc; the dispatch is handled by [`entity_radius`].
pub fn equal_radius(
    e1: SketchEntityId,
    e2: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    let r1 = entity_radius(e1, state, index, sketch)?;
    let r2 = entity_radius(e2, state, index, sketch)?;
    Ok(vec![r2 - r1])
}

/// TangentLineArc: perpendicular distance from the arc centre to the
/// line equals the arc radius.
///
/// Residual = `|signed_perp_dist| − r_arc`. The line can sit on
/// either side of the centre and still be tangent, so the absolute
/// value is required. A degenerate (zero-length) line collapses to
/// `0 − r` so LM still has gradient information to grow the line.
pub fn tangent_line_arc(
    line: SketchEntityId,
    arc: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    let (s, e) = line_endpoints(line, sketch).ok_or(SketchError::EntityNotFound(line))?;
    let p_s = point_xy(s, state, index, sketch).ok_or(SketchError::EntityNotFound(s))?;
    let p_e = point_xy(e, state, index, sketch).ok_or(SketchError::EntityNotFound(e))?;
    let c = entity_center_xy(arc, state, index, sketch)?;
    let r = entity_radius(arc, state, index, sketch)?;

    let d = sub(p_e, p_s);
    let len = norm(d);
    // Match the `point_on` family's eps; near-degenerate lines (sub-ULP
    // norm after `f64::hypot`) would otherwise silently divide by ~0.
    let signed = if len < super::point_on::DEGENERATE_LEN_EPS {
        0.0
    } else {
        cross(sub(c, p_s), d) / len
    };
    Ok(vec![signed.abs() - r])
}

/// TangentArcArc:
///   external (`internal = false`): `|C2 − C1| − (r1 + r2) = 0`.
///   internal (`internal = true`):  `|C2 − C1| − |r1 − r2| = 0`.
///
/// Each entity may be Arc or Circle (dispatch via
/// [`entity_center_xy`] + [`entity_radius`]).
pub fn tangent_arc_arc(
    a1: SketchEntityId,
    a2: SketchEntityId,
    internal: bool,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    let c1 = entity_center_xy(a1, state, index, sketch)?;
    let c2 = entity_center_xy(a2, state, index, sketch)?;
    let r1 = entity_radius(a1, state, index, sketch)?;
    let r2 = entity_radius(a2, state, index, sketch)?;

    let dist = distance(c2, c1);
    let target = if internal { (r1 - r2).abs() } else { r1 + r2 };
    Ok(vec![dist - target])
}
