//! GUI state that points into a schematic engine about to be replaced.

use crate::app::Signex;

impl Signex {
    /// Whether the open inline text edit targets an object of the engine at
    /// `path`.
    pub(crate) fn text_edit_targets_sheet_at(&self, path: &std::path::Path) -> bool {
        use signex_types::schematic::SelectedKind;
        let (Some(edit), Some(engine)) = (
            self.interaction_state.editing_text.as_ref(),
            self.document_state.engines.get(path),
        ) else {
            return false;
        };
        let sheet = engine.document();
        match edit.kind {
            SelectedKind::Label => sheet.labels.iter().any(|l| l.uuid == edit.uuid),
            SelectedKind::TextNote => sheet.text_notes.iter().any(|t| t.uuid == edit.uuid),
            SelectedKind::SymbolRefField | SelectedKind::SymbolValField | SelectedKind::Symbol => {
                sheet.symbols.iter().any(|s| s.uuid == edit.uuid)
            }
            _ => false,
        }
    }

    /// A restore is about to replace (or drop) the engine behind the visible
    /// schematic. Inline text, a pending move and the selection point into
    /// the old document, so they must not outlive it.
    pub(crate) fn discard_state_of_replaced_engine(&mut self) {
        self.interaction_state.editing_text = None;
        self.cancel_move_pick();
        self.interaction_state.active_canvas_mut().selected.clear();
        self.update_selection_info();
    }
}
