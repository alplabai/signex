//! The only door to a board's contents after it opened (#661 slice 7).
//!
//! Every edit, undo and redo of a PCB passes through these three methods,
//! which run the board's `BoardEngine`, then keep the two things derived from
//! it in step in one place: the revision map (the ratsnest, the pick index
//! and a save completion key on it) and the dirty mark (clean again when the
//! history returns to its save point). Design: ADR-0005, "Hosting board
//! edits".

use std::path::{Path, PathBuf};

use signex_engine::pcb::{BoardCommand, BoardCommandError, BoardPatch};

use super::DocumentState;

/// Why a board edit did not run. Nothing changed.
#[derive(Debug, Clone, PartialEq)]
pub enum BoardEditError {
    NotLoaded(PathBuf),
    /// The tab is closed and its unsaved board is only kept for saving.
    Parked(PathBuf),
    /// The board opened read-only (a newer format) and is never written.
    ReadOnly(PathBuf),
    Refused(BoardCommandError),
}

impl std::fmt::Display for BoardEditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotLoaded(path) => write!(f, "no PCB is open at {}", path.display()),
            Self::Parked(path) => write!(
                f,
                "{} is closed with unsaved edits kept for saving; reopen it to edit or undo",
                path.display()
            ),
            Self::ReadOnly(path) => write!(
                f,
                "{} opened read-only (newer format); it cannot be edited",
                path.display()
            ),
            Self::Refused(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for BoardEditError {}

impl From<BoardCommandError> for BoardEditError {
    fn from(error: BoardCommandError) -> Self {
        Self::Refused(error)
    }
}

/// A labelled edit checked against the revision its author read: an agent's
/// step, or an Update PCB change list built from one revision of the board.
#[derive(Debug, Clone, Copy)]
pub struct CheckedStep<'a> {
    pub label: &'a str,
    pub expected_revision: u64,
}

impl DocumentState {
    /// Run `commands` on the board at `path` as one undo step, all or
    /// nothing. With `checked`, the step is labelled and refused when the
    /// board moved past the revision it was built from.
    pub(crate) fn execute_board(
        &mut self,
        path: &Path,
        commands: Vec<BoardCommand>,
        checked: Option<CheckedStep<'_>>,
    ) -> Result<Vec<BoardPatch>, BoardEditError> {
        let engine = self.editable_board(path)?;
        let patches = match checked {
            Some(step) => engine.execute_labelled(commands, step.label, step.expected_revision),
            None => engine.execute_batch(commands),
        }?;
        if !patches.is_empty() {
            self.board_history_moved(path);
        }
        Ok(patches)
    }

    /// Undo the newest step of the board at `path`. `Ok(false)` when there is
    /// nothing to undo.
    pub(crate) fn undo_board(&mut self, path: &Path) -> Result<bool, BoardEditError> {
        self.step_board(path, true)
    }

    /// Redo the newest undone step of the board at `path`, see
    /// [`Self::undo_board`].
    pub(crate) fn redo_board(&mut self, path: &Path) -> Result<bool, BoardEditError> {
        self.step_board(path, false)
    }

    fn step_board(&mut self, path: &Path, undo: bool) -> Result<bool, BoardEditError> {
        let engine = self.editable_board(path)?;
        let moved = if undo { engine.undo() } else { engine.redo() }?.is_some();
        if moved {
            self.board_history_moved(path);
        }
        Ok(moved)
    }

    fn editable_board(
        &mut self,
        path: &Path,
    ) -> Result<&mut signex_engine::pcb::BoardEngine, BoardEditError> {
        if !self.has_pcb_tab(path) && self.parked_pcb_boards.contains_key(path) {
            return Err(BoardEditError::Parked(path.to_path_buf()));
        }
        let engine = self
            .board_engine_mut(path)
            .ok_or_else(|| BoardEditError::NotLoaded(path.to_path_buf()))?;
        if engine.board().unknown.newer_minor_source.is_some() {
            return Err(BoardEditError::ReadOnly(path.to_path_buf()));
        }
        Ok(engine)
    }

    /// The engine at `path` executed, undid or redid a step.
    fn board_history_moved(&mut self, path: &Path) {
        let Some(revision) = self.board_engine(path).map(|e| e.revision()) else {
            return;
        };
        self.note_pcb_revision(path, revision);
        self.sync_history_dirty(path);
    }

    /// `(can_undo, can_redo)` of the board at `path`; none for a parked one.
    pub(crate) fn board_history_available(&self, path: &Path) -> (bool, bool) {
        if !self.has_pcb_tab(path) {
            return (false, false);
        }
        self.board_engine(path)
            .map_or((false, false), |e| (e.can_undo(), e.can_redo()))
    }
}
