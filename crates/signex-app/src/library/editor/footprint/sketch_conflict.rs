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
use super::state::conflict::name_conflict;

/// True when `error` is a named conflict, the one failure for which
/// [`mutate_solve_or_revert`] puts the sketch back. Every other solve
/// failure leaves the edit in place. That includes `DegenerateLine`: its
/// message already says how to fix it (move an endpoint or delete the line),
/// and reverting would also discard a drag or an unrelated edit the user
/// made on a sketch that already held a zero-length line.
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
            let (rows, named_entities) = name_conflict(&ids, footprint.sketch.as_ref());
            footprint.sketch = before;
            drop_dangling_selection(state, footprint);
            // The re-solve dedups against what is already logged, then the
            // set is put back: it is the clean sketch's, without the
            // conflict that is about to be reported, and keeping it would
            // make the next identical rejection log again.
            let logged = state.logged_solve_warnings.clone();
            let resolved = solve_and_bake(state, footprint);
            state.logged_solve_warnings = logged;
            if let Err(e) = resolved {
                tracing::warn!(
                    target: "signex::sketch",
                    error = %e,
                    "the sketch reverted after a conflict could not be re-solved"
                );
            }
            state
                .conflict
                .record_rejection(ids.clone(), rows, named_entities);
            Err(SketchError::SolveFailed(SolveError::OverConstrained(ids)))
        }
        other => other,
    }
}

/// A reverted edit may have created the entity the user had selected.
pub(super) fn drop_dangling_selection(state: &mut FootprintEditorState, footprint: &Footprint) {
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
