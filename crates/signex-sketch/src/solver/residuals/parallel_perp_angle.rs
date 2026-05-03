//! Task 2.4 — Parallel / Perpendicular / Angle residuals.
//!
//! Each helper returns a single scalar residual (`Vec<f64>` of length 1)
//! that the Levenberg–Marquardt driver in Phase 3 will drive to zero.
//!
//! Geometry references:
//!  - Hearn & Baker, *Computer Graphics*, ch. 5 (2D vector geometry).
//!  - *Numerical Recipes*, §10.6 (atan2 branch handling).
//!
//! All formulas were derived from first-principles 2D vector algebra:
//!  - Two lines are parallel iff their direction-vector cross is zero.
//!  - Two lines are perpendicular iff their direction-vector dot is zero.
//!  - The signed CCW angle from `d1` to `d2` is `atan2(cross, dot)`.

use crate::error::SketchError;
use crate::id::SketchEntityId;
use crate::sketch::SketchData;
use crate::solver::state::{line_endpoints, point_xy, EntityIndex};

/// Resolve a line's direction vector `(dx, dy) = end − start` from the
/// current state vector. Returns `EntityNotFound` if the line itself or
/// either endpoint cannot be resolved.
fn line_dir(
    line: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<(f64, f64), SketchError> {
    let (s, e) = line_endpoints(line, sketch).ok_or(SketchError::EntityNotFound(line))?;
    let (sx, sy) = point_xy(s, state, index, sketch).ok_or(SketchError::EntityNotFound(s))?;
    let (ex, ey) = point_xy(e, state, index, sketch).ok_or(SketchError::EntityNotFound(e))?;
    Ok((ex - sx, ey - sy))
}

/// Wrap `θ` into the principal range `(−π, π]`.
///
/// Computed as `θ − 2π · round(θ / 2π)` then nudged so the value at
/// `θ = +π` stays `+π` (i.e. half-open interval on the negative side).
/// This keeps the Angle residual continuous across a sketch that
/// crosses the ±π branch cut, so Levenberg–Marquardt sees a well-formed
/// derivative instead of a 2π jump.
fn wrap_to_pi(theta: f64) -> f64 {
    use std::f64::consts::PI;
    let two_pi = 2.0 * PI;
    let mut t = theta - two_pi * (theta / two_pi).round();
    if t <= -PI {
        t += two_pi;
    } else if t > PI {
        t -= two_pi;
    }
    t
}

/// Parallel: `cross(d1, d2) = d1.x·d2.y − d1.y·d2.x = 0`.
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
    let (d1x, d1y) = line_dir(l1, state, index, sketch)?;
    let (d2x, d2y) = line_dir(l2, state, index, sketch)?;
    let cross = d1x * d2y - d1y * d2x;
    Ok(vec![cross])
}

/// Perpendicular: `dot(d1, d2) = d1.x·d2.x + d1.y·d2.y = 0`.
pub fn perpendicular(
    l1: SketchEntityId,
    l2: SketchEntityId,
    state: &[f64],
    index: &EntityIndex,
    sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    let (d1x, d1y) = line_dir(l1, state, index, sketch)?;
    let (d2x, d2y) = line_dir(l2, state, index, sketch)?;
    let dot = d1x * d2x + d1y * d2y;
    Ok(vec![dot])
}

/// Angle: signed CCW angle from `d1` to `d2` equals `target_rad`.
///
/// Residual = `wrap(atan2(cross, dot) − target_rad)` mapped into
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
    let (d1x, d1y) = line_dir(l1, state, index, sketch)?;
    let (d2x, d2y) = line_dir(l2, state, index, sketch)?;
    let cross = d1x * d2y - d1y * d2x;
    let dot = d1x * d2x + d1y * d2y;
    let measured = cross.atan2(dot);
    Ok(vec![wrap_to_pi(measured - target_rad)])
}
