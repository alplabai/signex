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

    /// `(can_undo, can_redo)` of the row history of the Component editor in
    /// `tab`.
    pub(crate) fn component_history_available(
        &self,
        tab: &crate::app::ComponentEditorTab,
    ) -> (bool, bool) {
        let address = EditorAddress::new(tab.library_path.clone(), tab.table.clone(), tab.row_id);
        self.component_history_of(&address)
    }

    /// `(can_undo, can_redo)` of the row history of the editor at `address`.
    pub(crate) fn component_history_of(&self, address: &EditorAddress) -> (bool, bool) {
        self.library
            .editors
            .get(address)
            .map_or((false, false), |e| {
                (e.history.can_undo(), e.history.can_redo())
            })
    }

    /// Undo (`undo`) or redo one edit of the active Component editor.
    /// `false` when the active tab is not one, so the caller goes on.
    pub(crate) fn step_component_editor_history(&mut self, undo: bool) -> bool {
        let Some(address) = self.active_component_editor() else {
            return false;
        };
        self.step_component_editor_at(&address, undo);
        true
    }

    /// Undo or redo one edit of the Component editor at `address`.
    pub(crate) fn step_component_editor_at(&mut self, address: &EditorAddress, undo: bool) {
        if let Some(editor) = self.library.editors.get_mut(address)
            && editor.step_history(undo)
        {
            self.refresh_panel_ctx();
        }
    }
}
