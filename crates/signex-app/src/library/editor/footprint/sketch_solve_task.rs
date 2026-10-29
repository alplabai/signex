//! Running the footprint sketch solve off the UI thread (#1047).
//!
//! A drag re-solves the sketch on every cursor tick, and a solve slower
//! than a frame freezes the UI for that long. A drag tick therefore moves
//! the geometry at once, notes that a solve is owed, and returns a
//! `Task` that runs the solver on a snapshot; the answer comes back as
//! [`FootprintEditorMsg::SketchSolved`] and is applied only if no edit has
//! happened since.
//!
//! The pieces:
//! - [`SketchSolveRequest`] / [`SolveOutcome`] — the `Send + 'static`
//!   snapshot in and the solver's answer out. [`run_solve`] is the pure
//!   function in between; it touches no editor state.
//! - [`SketchSolveSlot`] — per-editor bookkeeping: the generation counter,
//!   whether a solve is in flight, and what a rejected result must undo.
//! - [`dispatch_pending`] — turns "a solve is owed" into at most one
//!   in-flight `Task`.
//!
//! Applying a result is `updates::sketch::solved`; baking it is
//! [`super::sketch_bake`], shared with the inline path.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use iced::Task;
use signex_sketch::expr::ExprError;
use signex_sketch::solver::{FullSolveOutput, Solver};
use signex_sketch::{SketchData, SolveError, parameter};
use uuid::Uuid;

use super::sketch_dispatch::apply_edit_inner;
use super::sketch_mode::SketchEdit;
use super::state::FootprintEditorState;
use crate::app::FootprintHistorySnapshot;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use signex_library::primitive::footprint::Footprint;

/// What the solver needs, owned: a snapshot of the sketch and the solver
/// settings, tagged with the generation it was taken at.
#[derive(Debug, Clone)]
pub struct SketchSolveRequest {
    instance: Uuid,
    generation: u64,
    footprint_idx: usize,
    sketch: SketchData,
    solver: Solver,
}

impl SketchSolveRequest {
    /// The generation this snapshot was taken at.
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// The solver's answer to one [`SketchSolveRequest`].
#[derive(Debug, Clone)]
pub enum SolveOutcome {
    Solved {
        resolved: HashMap<String, f64>,
        out: Box<FullSolveOutput>,
    },
    /// A parameter expression did not parse or evaluate; nothing solved.
    ResolveFailed(ExprError),
    SolveFailed(SolveError),
}

/// Solve the snapshot and package the answer as the message that carries
/// it back to the editor. Pure: runs on whichever thread the `Task`
/// lands on, and in tests runs inline.
///
/// A panic in the solver is caught and returned as a failed solve: a
/// `Task` that panicked would never deliver its message, and the editor
/// would wait for it forever.
pub fn run_solve(request: SketchSolveRequest) -> FootprintEditorMsg {
    run_solve_with(request, solve_sketch)
}

/// The real solve: resolve the parameters, then run the solver.
fn solve_sketch(sketch: &SketchData, solver: &Solver) -> SolveOutcome {
    match parameter::resolve(&sketch.parameters) {
        Err(e) => SolveOutcome::ResolveFailed(e),
        Ok(resolved) => match solver.solve(sketch, &resolved) {
            Ok(out) => SolveOutcome::Solved {
                resolved,
                out: Box::new(out),
            },
            Err(e) => SolveOutcome::SolveFailed(e),
        },
    }
}

/// [`run_solve`] with the solving step injectable, so a panicking one can
/// be tested.
pub(super) fn run_solve_with(
    request: SketchSolveRequest,
    solve: impl FnOnce(&SketchData, &Solver) -> SolveOutcome,
) -> FootprintEditorMsg {
    let SketchSolveRequest {
        instance,
        generation,
        footprint_idx,
        sketch,
        solver,
    } = request;
    let outcome =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| solve(&sketch, &solver)))
            .unwrap_or_else(|payload| {
                let reason = payload
                    .downcast_ref::<&str>()
                    .map(|s| (*s).to_owned())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "unknown panic".to_owned());
                SolveOutcome::SolveFailed(SolveError::Internal(format!(
                    "the solver panicked: {reason}"
                )))
            });
    solved_message(instance, generation, footprint_idx, outcome)
}

fn solved_message(
    instance: Uuid,
    generation: u64,
    footprint_idx: usize,
    outcome: SolveOutcome,
) -> FootprintEditorMsg {
    FootprintEditorMsg::SketchSolved {
        instance,
        generation,
        footprint_idx,
        outcome: Arc::new(outcome),
    }
}

/// [`run_solve`] on tokio's blocking pool, so a CPU-bound solve does not
/// occupy an async worker. Lives in the `Task`'s future; the runtime is
/// iced's tokio executor.
async fn run_solve_blocking(request: SketchSolveRequest) -> FootprintEditorMsg {
    let (instance, generation, footprint_idx) =
        (request.instance, request.generation, request.footprint_idx);
    match tokio::task::spawn_blocking(move || run_solve(request)).await {
        Ok(msg) => msg,
        Err(e) => solved_message(
            instance,
            generation,
            footprint_idx,
            SolveOutcome::SolveFailed(SolveError::Internal(format!("the solve task failed: {e}"))),
        ),
    }
}

/// What a drag batch has to settle, anchored to where the batch began: the
/// undo history as it stood (`history_len_before` steps, the last of them
/// the last solved state) and the sketch at that moment. Nothing here counts
/// ticks, so a tick that records no step, or an extra one, cannot skew it.
#[derive(Debug, Clone)]
pub(crate) struct UnsolvedEdits {
    pub(crate) revert_to: Option<SketchData>,
    /// The first tick's undo step: the last solved state before the drag.
    /// When the batch is accepted, its steps collapse to this one.
    pub(crate) first_step: Option<FootprintHistorySnapshot>,
    /// `history.len()` before the first tick: how many undo steps predate
    /// the drag.
    pub(crate) history_len_before: usize,
    /// How many steps fell off the bottom of a full history while the batch
    /// ran. The pre-drag steps still present are the first
    /// `history_len_before - evictions`.
    pub(crate) evictions: usize,
    pub(crate) dirty_before: Option<bool>,
    /// The redo stack the first tick would have cleared, parked here so a
    /// rejection can give it back.
    pub(crate) redo_before: Option<Vec<FootprintHistorySnapshot>>,
    /// The pre-drag steps that fell off, oldest first. Evicted steps of the
    /// drag itself are not kept.
    pub(crate) evicted: Vec<FootprintHistorySnapshot>,
}

/// What one settled message left behind while a drag batch was pending.
pub(crate) struct SettledTick {
    pub(crate) dirty_before: bool,
    /// The message moved the sketch and owes a solve (as opposed to a drag
    /// tick that changed nothing).
    pub(crate) deferred: bool,
    pub(crate) kept: bool,
    pub(crate) history_len: usize,
    pub(crate) evicted: Option<FootprintHistorySnapshot>,
    /// The step this tick recorded, when it is the batch's first.
    pub(crate) step: Option<FootprintHistorySnapshot>,
    pub(crate) redo: Vec<FootprintHistorySnapshot>,
}

/// Per-editor bookkeeping for the off-thread solve. Lives in the editor
/// state, so it is per tab and gone with the tab.
#[derive(Debug, Clone)]
pub struct SketchSolveSlot {
    /// Unique per editor state: an answer for another instance (an editor
    /// that was closed and reopened, or whose state was rebuilt by a
    /// footprint switch) is never applied, whatever its generation.
    instance: Uuid,
    /// Bumped by every edit that moves geometry, every inline solve and
    /// every undo/redo. A result is applied only if its generation is
    /// still this one.
    generation: u64,
    /// The generation of the solve running off-thread, if one is. At most
    /// one runs at a time; a superseded one stays here until it answers.
    in_flight: Option<u64>,
    /// An edit is waiting for a solve that has not started.
    wanted: bool,
    unsolved: Option<UnsolvedEdits>,
    /// The next solve re-solves a sketch a rejection just put back, so it
    /// must not wipe the rejection's warning.
    after_revert: bool,
}

impl Default for SketchSolveSlot {
    fn default() -> Self {
        Self {
            instance: Uuid::now_v7(),
            generation: 0,
            in_flight: None,
            wanted: false,
            unsolved: None,
            after_revert: false,
        }
    }
}

impl SketchSolveSlot {
    pub fn instance(&self) -> Uuid {
        self.instance
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn is_in_flight(&self) -> bool {
        self.in_flight.is_some()
    }

    /// A solve is owed: an edit moved geometry that no applied result
    /// covers yet, and either no solve has started for it or the one
    /// running is for the current sketch.
    pub fn is_solve_owed(&self) -> bool {
        self.wanted || self.in_flight == Some(self.generation)
    }

    /// An edit moved geometry; the sketch needs a solve.
    pub(crate) fn note_edit(&mut self, sketch_before: &Option<SketchData>) {
        self.generation += 1;
        self.wanted = true;
        self.after_revert = false;
        if self.unsolved.is_none() {
            self.unsolved = Some(UnsolvedEdits {
                revert_to: sketch_before.clone(),
                first_step: None,
                history_len_before: 0,
                evictions: 0,
                dirty_before: None,
                redo_before: None,
                evicted: Vec::new(),
            });
        }
    }

    /// Whether the message that just ran left a drag solve owed: an edit
    /// moved geometry since `generation_before` and nothing solved it.
    pub(crate) fn tick_pending(&self, generation_before: u64) -> bool {
        self.generation != generation_before && self.unsolved.is_some()
    }

    /// Whether a drag batch is pending.
    pub(crate) fn has_unsolved(&self) -> bool {
        self.unsolved.is_some()
    }

    /// The editor settled a message while a batch was pending: remember
    /// what it left in undo history and the dirty flag, in case the solve
    /// rejects the batch.
    pub(crate) fn note_settled(&mut self, tick: SettledTick) {
        let Some(unsolved) = self.unsolved.as_mut() else {
            return;
        };
        if tick.deferred {
            unsolved.dirty_before.get_or_insert(tick.dirty_before);
            unsolved.redo_before.get_or_insert(tick.redo);
        }
        if !tick.kept {
            return;
        }
        if tick.deferred && unsolved.first_step.is_none() {
            // The batch's first step went on, and one fell off the bottom
            // if the history was already full.
            unsolved.first_step = tick.step;
            unsolved.history_len_before =
                (tick.history_len + usize::from(tick.evicted.is_some())).saturating_sub(1);
        }
        if let Some(evicted) = tick.evicted {
            unsolved.evictions += 1;
            // Only steps that predate the drag are worth giving back.
            if unsolved.evicted.len() < unsolved.history_len_before {
                unsolved.evicted.push(evicted);
            }
        }
    }

    /// Something other than a drag solve now owns the sketch (an inline
    /// solve, an undo, a redo): any result still in flight is stale and
    /// nothing is owed. The stale task keeps its slot until its answer
    /// arrives, so a new solve never overlaps it on the blocking pool.
    pub fn supersede(&mut self) {
        self.generation += 1;
        self.wanted = false;
        self.unsolved = None;
        self.after_revert = false;
    }

    /// Whether the next settled tick is the first of its drag batch, whose
    /// undo step is the one the batch collapses to.
    pub(crate) fn awaiting_first_step(&self) -> bool {
        self.unsolved
            .as_ref()
            .is_some_and(|u| u.first_step.is_none())
    }

    /// A rejection put the sketch back; solve it again so the canvas and
    /// the baked geometry match.
    pub(crate) fn resolve_after_revert(&mut self) {
        self.generation += 1;
        self.wanted = true;
        self.after_revert = true;
    }

    /// The solve for `generation` landed. Only the running solve frees the
    /// slot: an answer for any other generation is not the one in flight.
    pub(crate) fn finish_flight(&mut self, generation: u64) {
        if self.in_flight == Some(generation) {
            self.in_flight = None;
        }
    }

    /// Hand over what a current result settles.
    pub(crate) fn take_settled(&mut self) -> (Option<UnsolvedEdits>, bool) {
        (self.unsolved.take(), std::mem::take(&mut self.after_revert))
    }
}

/// Apply a drag-tick edit to the sketch without solving it: the geometry
/// moves now, the solve follows off-thread. Mirrors the first half of the
/// inline path (`mutate_solve_or_revert`): the old conflict highlight goes
/// as soon as the user moves again.
pub(super) fn apply_sketch_edit_deferred(
    state: &mut FootprintEditorState,
    footprint: &mut Footprint,
    edit: SketchEdit,
) {
    let before = if state.sketch_solve.unsolved.is_none() {
        footprint.sketch.clone()
    } else {
        None
    };
    state.conflict.clear();
    apply_edit_inner(footprint, edit);
    state.sketch_solve.note_edit(&before);
}

/// The snapshot of the editor's sketch a solve would run on right now, or
/// `None` when it has no sketch.
pub fn snapshot_request(editor: &crate::app::FootprintDocument) -> Option<SketchSolveRequest> {
    Some(SketchSolveRequest {
        instance: editor.state.sketch_solve.instance,
        generation: editor.state.sketch_solve.generation,
        footprint_idx: editor.active_idx,
        sketch: editor.primitive().sketch.clone()?,
        solver: editor.state.sketch_solver.clone(),
    })
}

/// Start the solve an edit is owed, unless one is already running: while
/// a solve is in flight further edits only record that another is owed,
/// and the next call (made when the running one lands) starts the single
/// follow-up on the newest sketch.
pub(crate) fn dispatch_pending(
    editor: &mut crate::app::FootprintDocument,
    path: &Path,
) -> Task<LibraryMessage> {
    let slot = &editor.state.sketch_solve;
    if !slot.wanted || slot.in_flight.is_some() {
        return Task::none();
    }
    let request = snapshot_request(editor);
    let slot = &mut editor.state.sketch_solve;
    slot.wanted = false;
    let Some(request) = request else {
        return Task::none();
    };
    slot.in_flight = Some(request.generation);
    let path = path.to_path_buf();
    Task::perform(run_solve_blocking(request), move |msg| {
        LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Footprint(msg),
        }
    })
}
