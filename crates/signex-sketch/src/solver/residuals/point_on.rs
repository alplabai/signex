//! Stub for Task 2.5 — PointOnLine / PointOnArc / DistancePtLine residuals.
//! Filled by a parallel agent.

use crate::error::SketchError;
use crate::id::SketchEntityId;
use crate::sketch::SketchData;
use crate::solver::state::EntityIndex;

pub fn point_on_line(
    _point: SketchEntityId,
    _line: SketchEntityId,
    _state: &[f64],
    _index: &EntityIndex,
    _sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    Ok(vec![0.0])
}

pub fn point_on_arc(
    _point: SketchEntityId,
    _arc: SketchEntityId,
    _state: &[f64],
    _index: &EntityIndex,
    _sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    Ok(vec![0.0])
}

pub fn distance_pt_line(
    _point: SketchEntityId,
    _line: SketchEntityId,
    _target_mm: f64,
    _state: &[f64],
    _index: &EntityIndex,
    _sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    Ok(vec![0.0])
}
