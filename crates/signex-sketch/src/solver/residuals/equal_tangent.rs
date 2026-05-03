//! Task 2.6 — EqualLength / EqualRadius / TangentLineArc / TangentArcArc
//! residuals.
//!
//! Each helper returns a single scalar residual (`Vec<f64>` of length 1)
//! that the Levenberg–Marquardt driver in Phase 3 will drive to zero.
//!
//! Geometry references:
//!  - Hearn & Baker, *Computer Graphics*, ch. 5 (2D vector geometry).
//!  - *Numerical Recipes*, §10 (linear algebra primitives).
//!
//! All formulas were derived from first-principles 2D vector algebra:
//!  - Line length = `sqrt(dx*dx + dy*dy)`.
//!  - Circle radius is stored explicitly; arc radius is the distance
//!    from the centre Point to the start Point.
//!  - Line/arc tangency: signed perpendicular distance from arc centre
//!    to the line equals the arc radius (in absolute value).
//!  - Arc/arc tangency:
//!      external — `|C2 − C1| = r1 + r2`
//!      internal — `|C2 − C1| = |r1 − r2|`

use crate::entity::EntityKind;
use crate::error::SketchError;
use crate::id::SketchEntityId;
use crate::sketch::SketchData;
use crate::solver::state::{
    arc_refs, circle_radius, find_entity, line_endpoints, point_xy, EntityIndex,
};

/// Resolve the length of a line from the current state vector.
fn line_length(
    line: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<f64, SketchError> {
    let (s, e) = line_endpoints(line, sketch).ok_or(SketchError::EntityNotFound(line))?;
    let (sx, sy) = point_xy(s, state, index, sketch).ok_or(SketchError::EntityNotFound(s))?;
    let (ex, ey) = point_xy(e, state, index, sketch).ok_or(SketchError::EntityNotFound(e))?;
    let dx = ex - sx;
    let dy = ey - sy;
    Ok((dx * dx + dy * dy).sqrt())
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
            let (cx, cy) =
                point_xy(center, state, index, sketch).ok_or(SketchError::EntityNotFound(center))?;
            let (sx, sy) =
                point_xy(start, state, index, sketch).ok_or(SketchError::EntityNotFound(start))?;
            let dx = sx - cx;
            let dy = sy - cy;
            Ok((dx * dx + dy * dy).sqrt())
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
) -> Result<(f64, f64), SketchError> {
    let entity = find_entity(id, sketch).ok_or(SketchError::EntityNotFound(id))?;
    let center_id = match entity.kind {
        EntityKind::Circle { center, .. } => center,
        EntityKind::Arc { center, .. } => center,
        _ => return Err(SketchError::EntityNotFound(id)),
    };
    point_xy(center_id, state, index, sketch).ok_or(SketchError::EntityNotFound(center_id))
}

/// EqualLength: `|d2| − |d1| = 0`.
///
/// One scalar residual; zero when both line lengths agree. Sign carries
/// the direction of correction so Levenberg–Marquardt can converge.
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

/// EqualRadius: `r2 − r1 = 0`.
///
/// Each entity may be a Circle (radius from state vector) or an Arc
/// (radius = `|start − center|`). One scalar residual.
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
/// Residual = `|signed_perp_dist| − r_arc`. The line could be on
/// either side of the centre and still be tangent, hence the absolute
/// value. The signed distance uses the same `cross / |d|` formulation
/// as `point_on::distance_pt_line`:
/// `signed_dist = ((cx − sx) · dy − (cy − sy) · dx) / |d|`.
pub fn tangent_line_arc(
    line: SketchEntityId,
    arc: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    let (s, e) = line_endpoints(line, sketch).ok_or(SketchError::EntityNotFound(line))?;
    let (sx, sy) = point_xy(s, state, index, sketch).ok_or(SketchError::EntityNotFound(s))?;
    let (ex, ey) = point_xy(e, state, index, sketch).ok_or(SketchError::EntityNotFound(e))?;
    let (cx, cy) = entity_center_xy(arc, state, index, sketch)?;
    let r = entity_radius(arc, state, index, sketch)?;

    let dx = ex - sx;
    let dy = ey - sy;
    let len = (dx * dx + dy * dy).sqrt();
    // For a degenerate (zero-length) line, residual collapses to −r so
    // the solver still has gradient information to grow the line.
    let signed = if len == 0.0 {
        0.0
    } else {
        ((cx - sx) * dy - (cy - sy) * dx) / len
    };
    Ok(vec![signed.abs() - r])
}

/// TangentArcArc:
///   external (`internal = false`): `|C2 − C1| − (r1 + r2) = 0`.
///   internal (`internal = true`):  `|C2 − C1| − |r1 − r2| = 0`.
///
/// One scalar residual. Each arc may be either an `EntityKind::Arc`
/// or `EntityKind::Circle` — `entity_center_xy` and `entity_radius`
/// dispatch accordingly.
pub fn tangent_arc_arc(
    a1: SketchEntityId,
    a2: SketchEntityId,
    internal: bool,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    let (c1x, c1y) = entity_center_xy(a1, state, index, sketch)?;
    let (c2x, c2y) = entity_center_xy(a2, state, index, sketch)?;
    let r1 = entity_radius(a1, state, index, sketch)?;
    let r2 = entity_radius(a2, state, index, sketch)?;

    let dx = c2x - c1x;
    let dy = c2y - c1y;
    let dist = (dx * dx + dy * dy).sqrt();
    let target = if internal {
        (r1 - r2).abs()
    } else {
        r1 + r2
    };
    Ok(vec![dist - target])
}
