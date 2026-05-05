//! Task 2.4 — Parallel / Perpendicular / Angle residuals.
//!
//! Each helper returns a single scalar residual (`Vec<f64>` of length 1)
//! that the Levenberg–Marquardt driver in Phase 3 will drive to zero.
//!
//! All formulas are composed from primitives in
//! [`crate::solver::math`]:
//!  - `cross` — two lines are parallel iff their direction-vector
//!    cross is zero.
//!  - `dot` — two lines are perpendicular iff their direction-vector
//!    dot is zero.
//!  - `wrap_to_pi` — the signed CCW angle from `d1` to `d2` is
//!    `atan2(cross, dot)`; the residual is wrapped into `(−π, π]`
//!    so a sketch crossing the ±π branch cut sees a continuous
//!    derivative.

use crate::error::SketchError;
use crate::id::SketchEntityId;
use crate::sketch::SketchData;
use crate::solver::math::{Vec2, cross as cross_2d, dot as dot_2d, sub, wrap_to_pi};
use crate::solver::state::{EntityIndex, line_endpoints, point_xy};

/// Resolve a line's direction vector `d = end − start` from the
/// current state vector. Returns `EntityNotFound` if the line itself
/// or either endpoint cannot be resolved.
fn line_dir(
    line: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec2, SketchError> {
    let (s, e) = line_endpoints(line, sketch).ok_or(SketchError::EntityNotFound(line))?;
    let p_s = point_xy(s, state, index, sketch).ok_or(SketchError::EntityNotFound(s))?;
    let p_e = point_xy(e, state, index, sketch).ok_or(SketchError::EntityNotFound(e))?;
    Ok(sub(p_e, p_s))
}

/// Parallel: `cross(d1, d2) = 0`.
///
/// Zero whether the lines point the same way or are antiparallel; the
/// solver doesn't need to distinguish, since both cases satisfy the
/// "same line direction modulo sign" definition of parallelism.
pub fn parallel(
    l1: SketchEntityId,
    l2: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    let d1 = line_dir(l1, state, index, sketch)?;
    let d2 = line_dir(l2, state, index, sketch)?;
    Ok(vec![cross_2d(d1, d2)])
}

/// Perpendicular: `dot(d1, d2) = 0`.
pub fn perpendicular(
    l1: SketchEntityId,
    l2: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    let d1 = line_dir(l1, state, index, sketch)?;
    let d2 = line_dir(l2, state, index, sketch)?;
    Ok(vec![dot_2d(d1, d2)])
}

/// Angle: signed CCW angle from `d1` to `d2` equals `target_rad`.
///
/// Residual = `wrap_to_pi(atan2(cross, dot) − target_rad)` mapped into
/// `(−π, π]` so the LM driver sees a continuous derivative across a
/// sketch that crosses the ±π branch cut.
pub fn angle(
    l1: SketchEntityId,
    l2: SketchEntityId,
    target_rad: f64,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    let d1 = line_dir(l1, state, index, sketch)?;
    let d2 = line_dir(l2, state, index, sketch)?;
    let measured = cross_2d(d1, d2).atan2(dot_2d(d1, d2));
    Ok(vec![wrap_to_pi(measured - target_rad)])
}
