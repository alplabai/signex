//! Rejecting an edit that over-constrains the sketch.
//!
//! The dispatcher snapshots the sketch before an edit. When the solve
//! then reports a conflict, the sketch goes back to that snapshot (the
//! last good state), the conflicting constraints are recorded in
//! `state.conflict` for the canvas to paint, and the error is returned so
//! the caller can report it.

use signex_library::primitive::footprint::Footprint;
use signex_sketch::{SketchError, SolveError};

use super::sketch_dispatch::solve_and_bake;
use super::state::FootprintEditorState;

/// True when `error` is a named conflict, the one failure for which
/// [`mutate_solve_or_revert`] puts the sketch back. Every other solve
/// failure leaves the edit in place.
pub(super) fn was_reverted(error: &SketchError) -> bool {
    matches!(
        error,
        SketchError::SolveFailed(SolveError::OverConstrained(ids)) if !ids.is_empty()
    )
}

/// Run `mutate`, then solve and bake. An over-constrained result reverts
/// the sketch to its pre-edit state, re-solves that state so the canvas
/// and baked geometry stay consistent, and highlights the conflict.
pub(super) fn mutate_solve_or_revert(
    state: &mut FootprintEditorState,
    footprint: &mut Footprint,
    mutate: impl FnOnce(&mut Footprint),
) -> Result<(), SketchError> {
    let before = footprint.sketch.clone();
    state.conflict.clear();
    mutate(footprint);
    match solve_and_bake(state, footprint) {
        Err(SketchError::SolveFailed(SolveError::OverConstrained(ids))) if !ids.is_empty() => {
            // Read the named constraints' entities now: the rejected edit's
            // own constraint disappears with the revert below.
            let named_entities = footprint
                .sketch
                .as_ref()
                .map(|s| {
                    s.constraints
                        .iter()
                        .filter(|c| ids.contains(&c.id))
                        .flat_map(|c| c.kind.entity_refs())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            footprint.sketch = before;
            drop_dangling_selection(state, footprint);
            if let Err(e) = solve_and_bake(state, footprint) {
                tracing::warn!(
                    target: "signex::sketch",
                    error = %e,
                    "the sketch reverted after a conflict could not be re-solved"
                );
            }
            state.conflict.record_rejection(ids.clone(), named_entities);
            Err(SketchError::SolveFailed(SolveError::OverConstrained(ids)))
        }
        other => other,
    }
}

/// A reverted edit may have created the entity the user had selected.
fn drop_dangling_selection(state: &mut FootprintEditorState, footprint: &Footprint) {
    let exists = |id| {
        footprint
            .sketch
            .as_ref()
            .is_some_and(|s| s.entities.iter().any(|e| e.id == id))
    };
    if state.selected_sketch.is_some_and(|id| !exists(id)) {
        state.selected_sketch = None;
    }
    if state
        .selected_sketch_secondary
        .is_some_and(|id| !exists(id))
    {
        state.selected_sketch_secondary = None;
    }
    state.selected_sketch_extra.retain(|id| exists(*id));
}
