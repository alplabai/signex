//! Undo / redo in a Component editor tab (#1854).
//!
//! The tab keeps its own row history (`ComponentPreviewState::history`), so
//! Ctrl+Z / Ctrl+Y there step the row and never reach a schematic engine.

use crate::app::Signex;
use crate::library::state::EditorAddress;

impl Signex {
    /// The Component editor shown in the active tab, if the active tab is one.
    fn active_component_editor(&self) -> Option<EditorAddress> {
        let tab = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)?;
        let editor = tab.kind.as_component_editor()?;
        Some(EditorAddress::new(
            editor.library_path.clone(),
            editor.table.clone(),
            editor.row_id,
        ))
    }

    /// Undo (`undo`) or redo one edit of the active Component editor.
    /// `false` when the active tab is not one, so the caller goes on.
    pub(crate) fn step_component_editor_history(&mut self, undo: bool) -> bool {
        let Some(address) = self.active_component_editor() else {
            return false;
        };
        if let Some(editor) = self.library.editors.get_mut(&address)
            && editor.step_history(undo)
        {
            self.refresh_panel_ctx();
        }
        true
    }
}
