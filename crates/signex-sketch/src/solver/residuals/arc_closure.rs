//! Implicit arc closure: an arc's end point lies on the circle
//! through its start point about its centre.
//!
//! An arc stores `(centre, start, end)` as three independent Points, so
//! nothing but this row ties `end` to the circle. With centre `c`,
//! start `s` and end `e` the circle has radius `|s − c|`, and the row is
//! `|e − c| − |s − c| = 0`.

use crate::entity::EntityKind;
use crate::error::SketchError;
use crate::sketch::SketchData;
use crate::solver::math::distance;
use crate::solver::state::{EntityIndex, arc_refs, point_xy};

/// One residual per Arc entity in `sketch`, in entity order.
pub fn arc_closure_rows(
    sketch: &SketchData,
    state: &[f64],
    index: &EntityIndex,
) -> Result<Vec<f64>, SketchError> {
    let mut rows = Vec::new();
    for entity in &sketch.entities {
        if !matches!(entity.kind, EntityKind::Arc { .. }) {
            continue;
        }
        let (center, start, end, _ccw) =
            arc_refs(entity.id, sketch).ok_or(SketchError::EntityNotFound(entity.id))?;
        let at = |id| point_xy(id, state, index, sketch).ok_or(SketchError::EntityNotFound(id));
        let c = at(center)?;
        rows.push(distance(at(end)?, c) - distance(at(start)?, c));
    }
    Ok(rows)
}
