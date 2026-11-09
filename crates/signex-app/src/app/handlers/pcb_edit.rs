//! Board edits, undo and redo as the app runs them (#661 slice 7).
//!
//! This is the board's mutation gateway, as `mutation_gateway.rs` is the
//! sheet's: no handler writes to a `PcbBoard`, every edit is a
//! `BoardCommand` run by the board's engine (`state/pcb_edit.rs`), and this
//! file repaints what showed the old board.

use std::path::Path;

use signex_engine::pcb::{BoardCommand, BoardPatch};

use crate::app::Signex;
use crate::app::state::{AgentStep, BoardEditError};

impl Signex {
    /// Run `commands` on the board at `path` as one undo step, all or
    /// nothing. With `agent`, the step is labelled and refused when the board
    /// moved past the revision the agent read.
    pub fn execute_board_commands(
        &mut self,
        path: &Path,
        commands: Vec<BoardCommand>,
        agent: Option<AgentStep<'_>>,
    ) -> Result<Vec<BoardPatch>, BoardEditError> {
        let patches = self.document_state.execute_board(path, commands, agent)?;
        if !patches.is_empty() {
            self.after_board_change(path);
        }
        Ok(patches)
    }

    /// One undo (`undo`) or redo of the board at `path`. Whether it moved; a
    /// refusal is logged and reads as not moved.
    pub(crate) fn step_board_history(&mut self, path: &Path, undo: bool) -> bool {
        let result = if undo {
            self.document_state.undo_board(path)
        } else {
            self.document_state.redo_board(path)
        };
        match result {
            Ok(true) => {
                self.after_board_change(path);
                true
            }
            Ok(false) => false,
            Err(error) => {
                crate::diagnostics::log_warning(error.to_string());
                false
            }
        }
    }

    /// The board at `path` changed: repaint the canvas, the ratsnest and the
    /// pick index (all keyed on its revision) when it is the one in view.
    fn after_board_change(&mut self, path: &Path) {
        if self.active_tab_path().as_deref() == Some(path) {
            self.apply_loaded_pcb_document(false, false);
        }
    }
}
