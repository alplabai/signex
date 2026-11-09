//! Board edits, undo and redo as the app runs them (#661 slice 7).
//!
//! This is the board's mutation gateway, as `mutation_gateway.rs` is the
//! sheet's: no handler writes to a `PcbBoard`, every edit is a
//! `BoardCommand` run by the board's engine (`state/pcb_edit.rs`), and this
//! file repaints what showed the old board.

use std::path::Path;

use signex_engine::pcb::{BoardCommand, BoardPatch};

use crate::app::Signex;
use crate::app::state::pcb_edit::{AgentStep, BoardEditError};

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

    /// The board at `path` changed: repaint every window that shows it. The
    /// main window's canvas, ratsnest and pick index follow the revision when
    /// it is the active tab; an undocked window keeps its own camera. This
    /// repaints only: it does not touch the tool or the placement in progress.
    fn after_board_change(&mut self, path: &Path) {
        let active = self.active_tab_path().as_deref() == Some(path);
        if active {
            // Also repaints the undocked windows that show the active tab.
            self.sync_pcb_canvas_from_visible_board();
            let canvas = &mut self.interaction_state.pcb_canvas;
            canvas.clear_bg_cache();
            canvas.clear_content_cache();
            self.sync_panel_ctx_from_visible_document();
        } else {
            self.repaint_undocked_boards(path);
        }
    }

    /// Give each undocked window that shows the board at `path` its new
    /// geometry.
    fn repaint_undocked_boards(&mut self, path: &Path) {
        let Some(board) = self.document_state.pcb_board(path) else {
            return;
        };
        let snapshot = signex_renderer::pcb::PcbSnapshot::from_board(board);
        for (id, canvas) in &mut self.interaction_state.pcb_canvases {
            let shows = matches!(
                self.ui_state.windows.get(id),
                Some(crate::app::state::WindowKind::UndockedTab { path: shown, .. })
                    if shown == path
            );
            if shows {
                canvas.set_renderer_snapshot(Some(snapshot.clone()));
                canvas.clear_content_cache();
            }
        }
    }
}
