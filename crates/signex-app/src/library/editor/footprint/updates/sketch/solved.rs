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

use crate::app::FootprintEditorState as Editor;
use crate::library::editor::footprint::sketch_bake::{bake_solution, discard_failed_solve};
use crate::library::editor::footprint::sketch_conflict::drop_dangling_selection;
use crate::library::editor::footprint::sketch_dispatch::report_solve_failure;
use crate::library::editor::footprint::sketch_solve_task::{
    SolveOutcome, UnsolvedEdits, run_solve, snapshot_request,
};
use crate::library::messages::FootprintEditorMsg;

pub(in crate::library::editor::footprint::updates) fn apply(
    editor: &mut Editor,
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
pub(crate) fn settle_pending_inline(editor: &mut Editor) {
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
    editor: &mut Editor,
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

/// The drag's ticks each recorded an undo step, and every one of them but
/// the first is a state nobody solved. The solve has landed, so the batch
/// becomes the one step a committed edit is: the first tick's, the last
/// solved state before the drag. History then holds only solved states,
/// which is what lets undo, redo and save trust what they restore.
fn collapse_batch_history(editor: &mut Editor, batch: UnsolvedEdits) {
    let Some(first_step) = batch.first_step else {
        return;
    };
    if editor.history.len() != batch.history_len_after || batch.history_steps == 0 {
        return;
    }
    // A drag long enough to fill the history pushed some of its own steps,
    // and the steps before it, off the bottom; the earlier ones come back.
    let surviving_ticks = batch.history_steps.min(batch.history_len_after);
    editor
        .history
        .truncate(batch.history_len_after - surviving_ticks);
    editor.history.splice(
        0..0,
        batch.evicted.into_iter().take(batch.history_len_before),
    );
    editor.history.push(first_step);
    if editor.history.len() > Editor::HISTORY_DEPTH {
        editor.history.remove(0);
    }
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
    restore_history_and_dirty(
        editor,
        UnsolvedHistory {
            steps: unsolved.history_steps,
            len_before: unsolved.history_len_before,
            len_after: unsolved.history_len_after,
            dirty_before: unsolved.dirty_before,
            redo_before: unsolved.redo_before,
            evicted: unsolved.evicted,
        },
    );
}

/// Take back what the rejected ticks did to undo history and the dirty
/// flag. Only when nothing else has touched the history since: anything
/// that records its own step settles the pending solve first, so a history
/// of another length means the ticks' steps are no longer on top, and
/// popping would remove someone else's. The dirty flag is left set then
/// too: the document has changed in some other way.
fn restore_history_and_dirty(editor: &mut Editor, unsolved: UnsolvedHistory) {
    let UnsolvedHistory {
        steps,
        len_before,
        len_after,
        dirty_before,
        redo_before,
        evicted,
    } = unsolved;
    if editor.history.len() != len_after {
        return;
    }
    // The ticks' steps are the newest ones; a drag long enough to fill the
    // history has pushed some of its own off the bottom, so only as many
    // survive as fit. The steps that predate the drag come back from
    // `evicted`, which holds nothing else.
    let surviving_ticks = steps.min(len_after);
    editor.history.truncate(len_after - surviving_ticks);
    editor
        .history
        .splice(0..0, evicted.into_iter().take(len_before));
    if let Some(redo) = redo_before
        && editor.redo.is_empty()
    {
        editor.redo = redo;
    }
    if let Some(dirty_before) = dirty_before {
        editor.dirty = dirty_before;
        if !dirty_before {
            editor.clean_mirror_owed = true;
        }
    }
}

/// What a rejection hands back to [`restore_history_and_dirty`].
struct UnsolvedHistory {
    steps: usize,
    len_before: usize,
    len_after: usize,
    dirty_before: Option<bool>,
    redo_before: Option<Vec<crate::app::FootprintHistorySnapshot>>,
    evicted: Vec<crate::app::FootprintHistorySnapshot>,
}
