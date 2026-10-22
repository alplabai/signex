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
    /// Entities the conflicting constraints named, captured before the
    /// revert. A rejected edit's own constraint is gone from the reverted
    /// sketch, so without this the user would not see what it clashed with.
    rejected_entities: Vec<SketchEntityId>,
    /// How many edits have been rejected so far. The dispatcher compares it
    /// before and after a message to learn whether that message was
    /// rejected; [`Self::clear`] does not reset it.
    rejections: u32,
}

impl ConflictHighlight {
    /// Record a rejection: the constraints the solver named and the entities
    /// they referred to, read from the sketch before it was reverted.
    pub fn record_rejection(
        &mut self,
        constraints: Vec<ConstraintId>,
        entities: Vec<SketchEntityId>,
    ) {
        self.constraints = constraints;
        self.rejected_entities = entities;
        self.rejections += 1;
    }

    pub fn clear(&mut self) {
        self.constraints.clear();
        self.rejected_entities.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }

    pub fn rejections(&self) -> u32 {
        self.rejections
    }

    pub fn constraints(&self) -> &[ConstraintId] {
        &self.constraints
    }

    pub fn contains_constraint(&self, id: ConstraintId) -> bool {
        self.constraints.contains(&id)
    }

    /// Entities to paint as conflicting: those named by the highlighted
    /// constraints still in `sketch`, plus the ones the rejected constraint
    /// named before it was reverted. Pure; recomputed per draw.
    pub fn entities(&self, sketch: &SketchData) -> HashSet<SketchEntityId> {
        sketch
            .constraints
            .iter()
            .filter(|c| self.contains_constraint(c.id))
            .flat_map(|c| c.kind.entity_refs())
            .chain(self.rejected_entities.iter().copied())
            .collect()
    }
}
