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

use super::sketch_dispatch::apply_edit_inner;
use super::sketch_mode::SketchEdit;
use super::state::FootprintEditorState as CanvasState;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use signex_library::primitive::footprint::Footprint;

/// What the solver needs, owned: a snapshot of the sketch and the solver
/// settings, tagged with the generation it was taken at.
#[derive(Debug, Clone)]
pub struct SketchSolveRequest {
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
pub fn run_solve(request: SketchSolveRequest) -> FootprintEditorMsg {
    let outcome = match parameter::resolve(&request.sketch.parameters) {
        Err(e) => SolveOutcome::ResolveFailed(e),
        Ok(resolved) => match request.solver.solve(&request.sketch, &resolved) {
            Ok(out) => SolveOutcome::Solved {
                resolved,
                out: Box::new(out),
            },
            Err(e) => SolveOutcome::SolveFailed(e),
        },
    };
    FootprintEditorMsg::SketchSolved {
        generation: request.generation,
        footprint_idx: request.footprint_idx,
        outcome: Arc::new(outcome),
    }
}

/// What a rejected result has to put back: the sketch as it was before the
/// first edit nobody has solved yet, the undo steps those edits recorded,
/// and the dirty flag they set.
#[derive(Debug, Clone)]
pub(crate) struct UnsolvedEdits {
    pub(crate) revert_to: Option<SketchData>,
    pub(crate) history_steps: usize,
    pub(crate) dirty_before: Option<bool>,
}

/// Per-editor bookkeeping for the off-thread solve. Lives in the editor
/// state, so it is per tab and gone with the tab.
#[derive(Debug, Clone, Default)]
pub struct SketchSolveSlot {
    /// Bumped by every edit that moves geometry, every inline solve and
    /// every undo/redo. A result is applied only if its generation is
    /// still this one.
    generation: u64,
    /// The generation of the solve running off-thread, if one is. At most
    /// one runs at a time.
    in_flight: Option<u64>,
    /// An edit is waiting for a solve that has not started.
    wanted: bool,
    unsolved: Option<UnsolvedEdits>,
    /// The next solve re-solves a sketch a rejection just put back, so it
    /// must not wipe the rejection's warning.
    after_revert: bool,
}

impl SketchSolveSlot {
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
                history_steps: 0,
                dirty_before: None,
            });
        }
    }

    /// The editor settled the message that called [`Self::note_edit`]:
    /// remember the undo step and dirty flag it left, in case the solve
    /// rejects the edit.
    pub(crate) fn note_settled(&mut self, generation_before: u64, dirty_before: bool, kept: bool) {
        if self.generation == generation_before {
            return;
        }
        if let Some(unsolved) = self.unsolved.as_mut() {
            unsolved.dirty_before.get_or_insert(dirty_before);
            unsolved.history_steps += usize::from(kept);
        }
    }

    /// Something other than a drag solve now owns the sketch (an inline
    /// solve, an undo, a redo): any result still in flight is stale and
    /// nothing is owed.
    pub fn supersede(&mut self) {
        self.generation += 1;
        self.wanted = false;
        self.unsolved = None;
        self.after_revert = false;
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
    state: &mut CanvasState,
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
pub fn snapshot_request(editor: &crate::app::FootprintEditorState) -> Option<SketchSolveRequest> {
    Some(SketchSolveRequest {
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
    editor: &mut crate::app::FootprintEditorState,
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
    Task::perform(async move { run_solve(request) }, move |msg| {
        LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Footprint(msg),
        }
    })
}
