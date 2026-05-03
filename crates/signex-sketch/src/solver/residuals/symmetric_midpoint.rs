//! Stub for Task 2.7 — SymmetricAboutLine / SymmetricAboutPoint /
//! Midpoint residuals. Filled by a parallel agent.

use crate::error::SketchError;
use crate::id::SketchEntityId;
use crate::sketch::SketchData;
use crate::solver::state::EntityIndex;

pub fn symmetric_about_line(
    _p1: SketchEntityId,
    _p2: SketchEntityId,
    _line: SketchEntityId,
    _state: &[f64],
    _index: &EntityIndex,
    _sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    Ok(vec![0.0, 0.0])
}

pub fn symmetric_about_point(
    _p1: SketchEntityId,
    _p2: SketchEntityId,
    _center: SketchEntityId,
    _state: &[f64],
    _index: &EntityIndex,
    _sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    Ok(vec![0.0, 0.0])
}

pub fn midpoint(
    _point: SketchEntityId,
    _line: SketchEntityId,
    _state: &[f64],
    _index: &EntityIndex,
    _sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    Ok(vec![0.0, 0.0])
}
