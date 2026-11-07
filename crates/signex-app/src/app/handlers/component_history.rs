//! Undo / redo in a Component editor tab (#1854).
//!
//! The tab keeps its own row history (`ComponentPreviewState::history`), so
//! Ctrl+Z / Ctrl+Y there step the row and never reach a schematic engine.

use crate::app::Signex;
use crate::library::state::EditorAddress;

impl Signex {
    /// `(can_undo, can_redo)` of the Component editor at `address`: an edit
    /// to step, or (undo only) half-typed text to drop.
    pub(crate) fn component_history_of(&self, address: &EditorAddress) -> (bool, bool) {
        self.library
            .editors
            .get(address)
            .map_or((false, false), |e| (e.can_undo_step(), e.can_redo_step()))
    }

    /// Undo or redo one edit of the Component editor at `address`.
    pub(crate) fn step_component_editor_at(&mut self, address: &EditorAddress, undo: bool) -> bool {
        let Some(editor) = self.library.editors.get_mut(address) else {
            return false;
        };
        let moved = editor.step_history(undo);
        if moved {
            self.refresh_panel_ctx();
        }
        moved
    }
}
