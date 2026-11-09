//! One board and its undo history: the only way an edit reaches a
//! [`PcbBoard`] (#661 slice 7, ADR-0005 addendum "Hosting board edits").
//!
//! The history holds the inverse commands of each step, never a board
//! snapshot, on [`History`] so undo depth, the cap and the save point behave
//! as they do for every other document.

use std::collections::BTreeMap;

use signex_history::{DEFAULT_UNDO_DEPTH, History};
use signex_types::pcb::PcbBoard;

use super::command::{BoardCommand, BoardPatch};
use super::error::BoardCommandError;

/// A board, the history of the edits made to it and its revision.
#[derive(Debug)]
pub struct BoardEngine {
    board: PcbBoard,
    /// Each step is the commands that undo one edit, in the order they run.
    /// Undoing a step swaps it for the commands that redo it.
    history: History<Vec<BoardCommand>>,
    /// Changes on every execute, undo and redo, and never repeats for one
    /// engine. Derived data (the ratsnest, the pick index) is keyed on it.
    revision: u64,
    /// Undo labels by timeline position (`History::position`), for steps
    /// made with [`Self::execute_labelled`].
    labels: BTreeMap<usize, String>,
}

impl BoardEngine {
    /// A board read from disk: the empty history is the saved state.
    /// `first_revision` must be above every revision an earlier engine for
    /// the same file reached, so a stale save completion cannot match.
    pub fn new(board: PcbBoard, first_revision: u64) -> Self {
        Self::with_history(board, first_revision, History::at_start(DEFAULT_UNDO_DEPTH))
    }

    /// A board not known to equal its file (a recovery copy): dirty until
    /// the next save.
    pub fn unsaved(board: PcbBoard, first_revision: u64) -> Self {
        Self::with_history(board, first_revision, History::unknown(DEFAULT_UNDO_DEPTH))
    }

    fn with_history(board: PcbBoard, revision: u64, history: History<Vec<BoardCommand>>) -> Self {
        Self {
            board,
            history,
            revision,
            labels: BTreeMap::new(),
        }
    }

    pub fn board(&self) -> &PcbBoard {
        &self.board
    }

    pub fn into_board(self) -> PcbBoard {
        self.board
    }

    /// Give the board a new id of its own (#2172): the repair of a missing or
    /// repeated one. Not an edit: no history step and no revision, so it is
    /// never dirty by itself and the next save writes it.
    pub fn reassign_document_id(&mut self, id: uuid::Uuid) {
        self.board.uuid = id;
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Apply one command as one undo step. On error the board, the history
    /// and the revision are unchanged.
    pub fn execute(&mut self, command: BoardCommand) -> Result<BoardPatch, BoardCommandError> {
        let mut patches = self.execute_batch(vec![command])?;
        Ok(patches.remove(0))
    }

    /// Apply the commands in order as ONE undo step, all or nothing. An
    /// empty list changes nothing: no step, no new revision.
    pub fn execute_batch(
        &mut self,
        commands: Vec<BoardCommand>,
    ) -> Result<Vec<BoardPatch>, BoardCommandError> {
        if commands.is_empty() {
            return Ok(Vec::new());
        }
        self.check_lease(&commands)?;
        let (inverses, patches) =
            apply_all(&mut self.board, commands).inspect_err(|error| self.note_failure(error))?;
        // A new edit cuts the redo branch, labels and all.
        let applied = self.history.position();
        self.labels.retain(|at, _| *at <= applied);
        self.history.record(undo_step(inverses));
        self.revision += 1;
        self.prune_labels();
        Ok(patches)
    }

    /// [`Self::execute_batch`] for an agent: refused, with nothing applied,
    /// when the board moved past `expected_revision`; the step carries
    /// `label` as its undo label.
    pub fn execute_labelled(
        &mut self,
        commands: Vec<BoardCommand>,
        label: &str,
        expected_revision: u64,
    ) -> Result<Vec<BoardPatch>, BoardCommandError> {
        if self.revision != expected_revision {
            return Err(BoardCommandError::StaleRevision {
                expected: expected_revision,
                current: self.revision,
            });
        }
        let made_step = !commands.is_empty();
        let patches = self.execute_batch(commands)?;
        if made_step {
            self.labels
                .insert(self.history.position(), label.to_string());
        }
        Ok(patches)
    }

    /// Undo the newest step. `None` when there is nothing to undo. On error
    /// the board and the history are unchanged.
    pub fn undo(&mut self) -> Result<Option<Vec<BoardPatch>>, BoardCommandError> {
        self.step(true)
    }

    /// Redo the newest undone step, see [`Self::undo`].
    pub fn redo(&mut self) -> Result<Option<Vec<BoardPatch>>, BoardCommandError> {
        self.step(false)
    }

    fn step(&mut self, undo: bool) -> Result<Option<Vec<BoardPatch>>, BoardCommandError> {
        let stack = if undo {
            self.history.undo_steps()
        } else {
            self.history.redo_steps()
        };
        let Some(step) = stack.last().cloned() else {
            return Ok(None);
        };
        self.check_lease(&step)?;
        let (inverses, patches) =
            apply_all(&mut self.board, step).inspect_err(|error| self.note_failure(error))?;
        self.history.begin_step(undo);
        self.history.finish_step(undo_step(inverses), undo);
        self.revision += 1;
        self.prune_labels();
        Ok(Some(patches))
    }

    /// A refusal leaves the board as it was. A failed rollback leaves it
    /// part-edited, which no revision, history step or save point describes:
    /// take a new revision so derived data is rebuilt, and forget the save
    /// point so the board reads unsaved.
    pub(super) fn note_failure(&mut self, error: &BoardCommandError) {
        if matches!(error, BoardCommandError::RollbackFailed(_)) {
            self.revision += 1;
            self.history.forget();
        }
    }

    /// The hook where the collab lease check goes (COLLABORATION_PLAN,
    /// v3.4): geometry-sensitive commands (`RouteTrace`, `ModifyZone`, ...)
    /// will need the advisory lease of their region, net or layer before
    /// they run. Every execute, undo and redo passes through here before it
    /// touches the board, so the API does not change when leases arrive.
    /// Today every command is allowed.
    fn check_lease(&self, _commands: &[BoardCommand]) -> Result<(), BoardCommandError> {
        Ok(())
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// How many undo steps are held now.
    pub fn undo_len(&self) -> usize {
        self.history.undo_len()
    }

    /// How many undo steps the engine keeps at most.
    pub fn undo_depth(&self) -> usize {
        self.history.cap()
    }

    /// Keep `depth` steps from now on; lowering it drops the oldest at once.
    pub fn set_undo_depth(&mut self, depth: usize) {
        self.history.set_cap(depth);
        self.prune_labels();
    }

    /// The label of the step Undo would take back, if an agent made it.
    pub fn undo_label(&self) -> Option<&str> {
        self.labels
            .get(&self.history.position())
            .map(String::as_str)
    }

    /// The label of the step Redo would apply, if an agent made it.
    pub fn redo_label(&self) -> Option<&str> {
        self.labels
            .get(&(self.history.position() + 1))
            .map(String::as_str)
    }

    /// Does the board equal the last saved or loaded state, judged by the
    /// history alone? Undo back to the save point reads true again.
    pub fn is_at_save_point(&self) -> bool {
        !self.history.is_dirty()
    }

    /// The board on screen is what a save is writing.
    pub fn mark_saved(&mut self) {
        self.history.mark_saved();
    }

    /// The file holds something no step reproduces: dirty until the next
    /// [`Self::mark_saved`].
    pub fn forget_save_point(&mut self) {
        self.history.forget();
    }

    /// Drop the labels of steps the history no longer holds (evicted by the
    /// cap, or cut with the redo branch).
    fn prune_labels(&mut self) {
        let position = self.history.position();
        let oldest = position.saturating_sub(self.history.undo_len());
        let newest = position + self.history.redo_len();
        self.labels.retain(|at, _| *at > oldest && *at <= newest);
    }
}

/// Run `commands` on `board`, returning their inverses in the order the
/// commands ran. All or nothing: a refusal puts back what ran before it.
fn apply_all(
    board: &mut PcbBoard,
    commands: Vec<BoardCommand>,
) -> Result<(Vec<BoardCommand>, Vec<BoardPatch>), BoardCommandError> {
    let mut inverses = Vec::with_capacity(commands.len());
    let mut patches = Vec::with_capacity(commands.len());
    for command in commands {
        match command.apply(board) {
            Ok(applied) => {
                inverses.push(applied.inverse);
                patches.push(applied.patch);
            }
            Err(error) => {
                for inverse in inverses.into_iter().rev() {
                    inverse
                        .apply(board)
                        .map_err(|e| BoardCommandError::RollbackFailed(e.to_string()))?;
                }
                return Err(error);
            }
        }
    }
    Ok((inverses, patches))
}

/// The step that undoes a run whose inverses are `inverses`: the same
/// commands, last first.
fn undo_step(mut inverses: Vec<BoardCommand>) -> Vec<BoardCommand> {
    inverses.reverse();
    inverses
}
