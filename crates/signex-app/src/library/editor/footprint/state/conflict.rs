//! Which constraints the last rejected edit conflicted with.
//!
//! Set by the sketch dispatcher when a solve reports an over-constraint,
//! cleared by the next edit or successful solve. The canvas reads it to
//! paint those constraints (and the entities they name) in the error
//! colour; nothing here is derived from the render pass.

use std::collections::HashSet;

use signex_sketch::SketchData;
use signex_sketch::id::{ConstraintId, SketchEntityId};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConflictHighlight {
    constraints: Vec<ConstraintId>,
}

impl ConflictHighlight {
    pub fn new(constraints: Vec<ConstraintId>) -> Self {
        Self { constraints }
    }

    pub fn clear(&mut self) {
        self.constraints.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }

    pub fn constraints(&self) -> &[ConstraintId] {
        &self.constraints
    }

    pub fn contains_constraint(&self, id: ConstraintId) -> bool {
        self.constraints.contains(&id)
    }

    /// Entities named by the highlighted constraints, resolved against
    /// `sketch`. Pure; recomputed per draw from the stored ids.
    pub fn entities(&self, sketch: &SketchData) -> HashSet<SketchEntityId> {
        sketch
            .constraints
            .iter()
            .filter(|c| self.contains_constraint(c.id))
            .flat_map(|c| c.kind.entity_refs())
            .collect()
    }
}
