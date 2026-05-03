//! Stub for Task 2.6 — EqualLength / EqualRadius / TangentLineArc /
//! TangentArcArc residuals. Filled by a parallel agent.

use crate::error::SketchError;
use crate::id::SketchEntityId;
use crate::sketch::SketchData;
use crate::solver::state::EntityIndex;

pub fn equal_length(
    _l1: SketchEntityId,
    _l2: SketchEntityId,
    _state: &[f64],
    _index: &EntityIndex,
    _sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    Ok(vec![0.0])
}

pub fn equal_radius(
    _e1: SketchEntityId,
    _e2: SketchEntityId,
    _state: &[f64],
    _index: &EntityIndex,
    _sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    Ok(vec![0.0])
}

pub fn tangent_line_arc(
    _line: SketchEntityId,
    _arc: SketchEntityId,
    _state: &[f64],
    _index: &EntityIndex,
    _sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    Ok(vec![0.0])
}

pub fn tangent_arc_arc(
    _a1: SketchEntityId,
    _a2: SketchEntityId,
    _internal: bool,
    _state: &[f64],
    _index: &EntityIndex,
    _sketch: &SketchData,
) -> Result<Vec<f64>, SketchError> {
    Ok(vec![0.0])
}
