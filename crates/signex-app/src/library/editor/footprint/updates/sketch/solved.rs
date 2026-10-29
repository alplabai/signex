//! Footprint sketch updates — applying a drag solve that finished off the
//! UI thread (#1047).
//!
//! The drag tick already moved the geometry; this applies the solver's
//! answer to it. The answer is for one generation of the sketch and is
//! applied only if that generation is still current — an edit made while
//! the solve ran makes it stale, and a stale answer changes nothing (the
//! follow-up solve for the newer sketch is started by the caller).

use std::sync::Arc;

use uuid::Uuid;

use signex_sketch::{SketchError, SolveError};

use crate::app::FootprintDocument;
use crate::library::editor::footprint::sketch_bake::{bake_solution, discard_failed_solve};
use crate::library::editor::footprint::sketch_conflict::drop_dangling_selection;
use crate::library::editor::footprint::sketch_dispatch::report_solve_failure;
use crate::library::editor::footprint::sketch_solve_task::{
    SolveOutcome, UnsolvedEdits, run_solve, snapshot_request,
};
use crate::library::editor::footprint::state::conflict::name_conflict;
use crate::library::messages::FootprintEditorMsg;

pub(in crate::library::editor::footprint::updates) fn apply(
    editor: &mut FootprintDocument,
    msg: FootprintEditorMsg,
) {
    let FootprintEditorMsg::SketchSolved {
        instance,
        generation,
        footprint_idx,
        outcome,
    } = msg
    else {
        unreachable!("non-SketchSolved variant routed to sketch/solved.rs");
    };
    apply_solved(editor, instance, generation, footprint_idx, outcome);
}

/// Solve now, on this thread, whatever drag solve is owed — for the moment
/// something is about to read or replace the sketch and must not see it
/// half-solved: a save, an edit that records its own undo step, a switch
/// to another footprint. A rejection is handled exactly as when the result
/// arrives from the task. Any task still running is made stale.
pub(crate) fn settle_pending_inline(editor: &mut FootprintDocument) {
    /// A rejection owes one more solve, of the sketch it restored.
    const MAX_ROUNDS: usize = 3;
    if !editor.state.sketch_solve.is_solve_owed() {
        return;
    }
    for _ in 0..MAX_ROUNDS {
        if !editor.state.sketch_solve.is_solve_owed() {
            break;
        }
        let Some(request) = snapshot_request(editor) else {
            break;
        };
        apply(editor, run_solve(request));
    }
    editor.state.sketch_solve.supersede();
}

fn apply_solved(
    editor: &mut FootprintDocument,
    instance: Uuid,
    generation: u64,
    footprint_idx: usize,
    outcome: Arc<SolveOutcome>,
) {
    if instance != editor.state.sketch_solve.instance() {
        return;
    }
    editor.state.sketch_solve.finish_flight(generation);
    if generation != editor.state.sketch_solve.generation() || footprint_idx != editor.active_idx {
        return;
    }
    let outcome = Arc::try_unwrap(outcome).unwrap_or_else(|shared| (*shared).clone());
    let (mut unsolved, after_revert) = editor.state.sketch_solve.take_settled();
    if !after_revert {
        editor.state.solve_warnings.clear();
    }
    let rejected = matches!(
        &outcome,
        SolveOutcome::SolveFailed(SolveError::OverConstrained(ids)) if !ids.is_empty()
    );
    // An accepted batch (anything but a rejection) leaves one undo step.
    if !rejected && let Some(batch) = unsolved.take() {
        collapse_batch_history(editor, batch);
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

/// Put the history back to where the batch began, against the anchor the
/// batch recorded rather than a count of its ticks: the steps that predate
/// the drag (the first `history_len_before - evictions` of the history, with
/// any that fell off restored from `evicted`) and nothing after them. Warns
/// if the history is not what the anchor says, and still settles against the
/// anchor: the batch's steps are unsolved states and must not survive.
fn rewind_history_to_batch_start(editor: &mut FootprintDocument, batch: &mut UnsolvedEdits) {
    let prefix = batch.history_len_before.saturating_sub(batch.evictions);
    // A drag long enough to push its own first step off the bottom leaves
    // nothing at the anchor to compare; that is expected, not an anomaly.
    let first_step_evicted = batch.evictions > batch.history_len_before;
    let intact = editor.history.len() >= prefix
        && (first_step_evicted
            || match (&batch.first_step, editor.history.get(prefix)) {
                (Some(first), Some(at)) => at.file == first.file,
                (None, None) => true,
                _ => false,
            });
    if !intact {
        crate::diagnostics::log_warning(
            "Sketch drag: the undo history changed while its solve was pending; \
             the drag's steps were reset to the state before it",
        );
    }
    editor.history.truncate(prefix);
    let evicted = std::mem::take(&mut batch.evicted);
    editor
        .history
        .splice(0..0, evicted.into_iter().take(batch.history_len_before));
}

/// The drag's ticks each recorded an undo step, and every one of them but
/// the first is a state nobody solved. The solve has landed, so the batch
/// becomes the one step a committed edit is: the first tick's, the last
/// solved state before the drag. History then holds only solved states,
/// which is what lets undo, redo and save trust what they restore.
fn collapse_batch_history(editor: &mut FootprintDocument, mut batch: UnsolvedEdits) {
    let Some(first_step) = batch.first_step.clone() else {
        return;
    };
    rewind_history_to_batch_start(editor, &mut batch);
    editor.history.push(first_step);
    if editor.history.len() > FootprintDocument::HISTORY_DEPTH {
        editor.history.remove(0);
    }
}

/// The drag over-constrained the sketch: put it back as it was before the
/// first unsolved tick, take back the undo steps and dirty flag those ticks
/// left, and highlight the conflict — what the inline path does for a
/// rejected edit. The restored sketch is solved again by a follow-up solve.
fn reject_conflicting_edit(
    editor: &mut FootprintDocument,
    ids: Vec<signex_sketch::id::ConstraintId>,
    unsolved: Option<UnsolvedEdits>,
) {
    let error = discard_failed_solve(&mut editor.state, SolveError::OverConstrained(ids.clone()));
    let Some(mut unsolved) = unsolved else {
        editor.with_parts(|state, _| report_solve_failure(state, &error));
        return;
    };
    editor.with_parts(|state, primitive| {
        // Read the named constraints' entities now: a rejected edit's own
        // constraint disappears with the revert below.
        let (rows, named_entities) = name_conflict(&ids, primitive.sketch.as_ref());
        primitive.sketch = unsolved.revert_to.take();
        drop_dangling_selection(state, primitive);
        state.conflict.record_rejection(ids, rows, named_entities);
        state.sketch_solve.resolve_after_revert();
        report_solve_failure(state, &error);
    });
    if unsolved.first_step.is_some() {
        rewind_history_to_batch_start(editor, &mut unsolved);
    }
    if let Some(redo) = unsolved.redo_before.take()
        && editor.redo.is_empty()
    {
        editor.redo = redo;
    }
    if let Some(dirty_before) = unsolved.dirty_before {
        let owes_rows = !editor.cascade_owed.is_empty();
        editor.set_dirty(dirty_before || owes_rows);
    }
}
