//! Per-bake record of which edges already belong to a baked closed profile.
//!
//! A user normally tags every edge of a loop (a rectangle's four sides) with
//! the same attribute. Tracing from each tagged edge would bake the same loop
//! once per edge; a bake pass keeps one [`TracedLoops`] and skips seeds whose
//! loop it already emitted.

use std::collections::HashSet;

use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;

use crate::profile::{TraceResult, trace_closed_profile, trace_closed_profile_entities};

/// Edge ids of every closed profile traced so far in one bake pass, per
/// attribute. A loop is baked once for each distinct attribute (layer, ...)
/// tagged on its edges, so the same loop tagged `Top` on one edge and `Bottom`
/// on another bakes twice, while two edges tagged identically bake once.
#[derive(Debug, Default)]
pub struct TracedLoops {
    edges: HashSet<(String, SketchEntityId)>,
}

impl TracedLoops {
    /// Whether `edge` belongs to a loop already traced for `attribute`, a
    /// stable text form of the attribute value the edge is tagged with.
    pub fn contains(&self, attribute: &str, edge: SketchEntityId) -> bool {
        self.edges.contains(&(attribute.to_owned(), edge))
    }

    /// Trace the closed profile through `seed`. On success every edge of the
    /// loop is recorded for `attribute`, so [`Self::contains`] is true for
    /// all of them.
    pub fn trace(
        &mut self,
        attribute: &str,
        sketch: &SketchData,
        solve: &FullSolveOutput,
        seed: SketchEntityId,
    ) -> TraceResult {
        let vertices = trace_closed_profile(sketch, solve, seed)?;
        let entities = trace_closed_profile_entities(sketch, seed)?;
        self.edges.extend(
            entities
                .edges
                .iter()
                .map(|edge| (attribute.to_owned(), edge.id)),
        );
        Ok(vertices)
    }
}
