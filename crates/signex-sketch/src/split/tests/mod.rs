//! `split_line` tests, grouped by concern (kept under the ~800-line
//! file cap — `signex-domain` §5):
//! - `carry_over` — attribute / flag / constraint / array / pad
//!   profile-seed carry-over onto the two replacement halves.
//! - `errors` — validation + the degenerate-input error taxonomy.
//! - `solver` — end-to-end solver acceptance (issue #360 blocker 3).

mod carry_over;
mod errors;
mod solver;

use super::*;
use crate::plane::PlaneId;

/// A single Line `start -> end` plus its two Point entities.
/// Returns `(sketch, line, start, end)`.
fn line_sketch(
    start: (f64, f64),
    end: (f64, f64),
) -> (SketchData, SketchEntityId, SketchEntityId, SketchEntityId) {
    let plane = PlaneId::new();
    let start_id = SketchEntityId::new();
    let end_id = SketchEntityId::new();
    let line_id = SketchEntityId::new();
    let mut sketch = SketchData::default();
    sketch.entities.push(Entity::new(
        start_id,
        plane,
        EntityKind::Point {
            x: start.0,
            y: start.1,
        },
    ));
    sketch.entities.push(Entity::new(
        end_id,
        plane,
        EntityKind::Point { x: end.0, y: end.1 },
    ));
    sketch.entities.push(Entity::new(
        line_id,
        plane,
        EntityKind::Line {
            start: start_id,
            end: end_id,
        },
    ));
    (sketch, line_id, start_id, end_id)
}

fn line_endpoints(sketch: &SketchData, line: SketchEntityId) -> (SketchEntityId, SketchEntityId) {
    match sketch.entities.iter().find(|e| e.id == line).unwrap().kind {
        EntityKind::Line { start, end } => (start, end),
        _ => panic!("not a line"),
    }
}
