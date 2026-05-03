//! Stub for Task 2.4 — Parallel / Perpendicular / Angle residuals.
//! Filled by a parallel agent.

use crate::error::SketchError;
use crate::id::SketchEntityId;
use crate::sketch::SketchData;
use crate::solver::state::EntityIndex;

pub fn parallel(
    _l1: SketchEntityId,
    _l2: SketchEntityId,
    _state: &[f64],
    _index: &EntityIndex,
    _sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    Ok(vec![0.0])
}

pub fn perpendicular(
    _l1: SketchEntityId,
    _l2: SketchEntityId,
    _state: &[f64],
    _index: &EntityIndex,
    _sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    Ok(vec![0.0])
}

pub fn angle(
    _l1: SketchEntityId,
    _l2: SketchEntityId,
    _target_rad: f64,
    _state: &[f64],
    _index: &EntityIndex,
    _sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    Ok(vec![0.0])
}
