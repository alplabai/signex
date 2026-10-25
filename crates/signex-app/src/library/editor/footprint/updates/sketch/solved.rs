//! Footprint sketch updates — applying a drag solve that finished off the
//! UI thread (#1047).
//!
//! The drag tick already moved the geometry; this applies the solver's
//! answer to it. The answer is for one generation of the sketch and is
//! applied only if that generation is still current — an edit made while
//! the solve ran makes it stale, and a stale answer changes nothing (the
//! follow-up solve for the newer sketch is started by the caller).

use std::sync::Arc;

use signex_sketch::{SketchError, SolveError};

use crate::app::FootprintEditorState as Editor;
use crate::library::editor::footprint::sketch_bake::{bake_solution, discard_failed_solve};
use crate::library::editor::footprint::sketch_conflict::drop_dangling_selection;
use crate::library::editor::footprint::sketch_dispatch::report_solve_failure;
use crate::library::editor::footprint::sketch_solve_task::{SolveOutcome, UnsolvedEdits};
use crate::library::messages::FootprintEditorMsg;

pub(in crate::library::editor::footprint::updates) fn apply(
    editor: &mut Editor,
    msg: FootprintEditorMsg,
) {
    let FootprintEditorMsg::SketchSolved {
        generation,
        footprint_idx,
        outcome,
    } = msg
    else {
        unreachable!("non-SketchSolved variant routed to sketch/solved.rs");
    };
    apply_solved(editor, generation, footprint_idx, outcome);
}

fn apply_solved(
    editor: &mut Editor,
    generation: u64,
    footprint_idx: usize,
    outcome: Arc<SolveOutcome>,
) {
    editor.state.sketch_solve.finish_flight(generation);
    if generation != editor.state.sketch_solve.generation() || footprint_idx != editor.active_idx {
        return;
    }
    let outcome = Arc::try_unwrap(outcome).unwrap_or_else(|shared| (*shared).clone());
    let (unsolved, after_revert) = editor.state.sketch_solve.take_settled();
    if !after_revert {
        editor.state.solve_warnings.clear();
    }
    match outcome {
        SolveOutcome::Solved { resolved, out } => {
            editor.with_parts(|state, primitive| {
                state.conflict.clear();
                if let Err(e) = bake_solution(state, primitive, &resolved, *out) {
                    report_solve_failure(state, &e);
                }
            });
        }
        SolveOutcome::ResolveFailed(e) => {
            let error = SketchError::Expr(e);
            editor.with_parts(|state, _| report_solve_failure(state, &error));
        }
        SolveOutcome::SolveFailed(SolveError::OverConstrained(ids)) if !ids.is_empty() => {
            reject_conflicting_edit(editor, ids, unsolved);
        }
        SolveOutcome::SolveFailed(e) => {
            // A failed re-solve of a sketch a rejection just restored is
            // logged only, as the inline path does.
            let error = discard_failed_solve(&mut editor.state, e);
            if !after_revert {
                editor.with_parts(|state, _| report_solve_failure(state, &error));
            }
        }
    }
    editor.canvas_cache.clear();
}

/// The drag over-constrained the sketch: put it back as it was before the
/// first unsolved tick, take back the undo steps and dirty flag those ticks
/// left, and highlight the conflict — what the inline path does for a
/// rejected edit. The restored sketch is solved again by a follow-up solve.
fn reject_conflicting_edit(
    editor: &mut Editor,
    ids: Vec<signex_sketch::id::ConstraintId>,
    unsolved: Option<UnsolvedEdits>,
) {
    let error = discard_failed_solve(&mut editor.state, SolveError::OverConstrained(ids.clone()));
    let Some(unsolved) = unsolved else {
        editor.with_parts(|state, _| report_solve_failure(state, &error));
        return;
    };
    editor.with_parts(|state, primitive| {
        // Read the named constraints' entities now: a rejected edit's own
        // constraint disappears with the revert below.
        let named_entities = primitive
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
        primitive.sketch = unsolved.revert_to;
        drop_dangling_selection(state, primitive);
        state.conflict.record_rejection(ids, named_entities);
        state.sketch_solve.resolve_after_revert();
        report_solve_failure(state, &error);
    });
    for _ in 0..unsolved.history_steps {
        editor.history.pop();
    }
    if let Some(dirty_before) = unsolved.dirty_before {
        editor.dirty = dirty_before;
    }
}
