//! SCH Library panel handlers — switch / add / delete symbols
//! within the active `.snxsym` container.
//!
//! All three messages mutate the active `SymbolEditorState`, mark
//! the tab dirty, and clear the canvas cache. The actual save to
//! disk happens through the existing Save flow (`save_primitive_tab_at`)
//! so the panel never writes the file directly — keeps the dirty /
//! save semantics consistent with every other in-tab mutation.

use super::super::super::*;

impl Signex {
    pub(super) fn handle_dock_sch_library_message(
        &mut self,
        panel_msg: &crate::panels::PanelMsg,
    ) -> bool {
        match panel_msg {
            crate::panels::PanelMsg::SchLibrarySelectSymbol(idx) => {
                self.sch_library_select_symbol(*idx);
                true
            }
            crate::panels::PanelMsg::SchLibraryAddSymbol => {
                self.sch_library_add_symbol();
                true
            }
            crate::panels::PanelMsg::SchLibraryDeleteSymbol(idx) => {
                self.sch_library_delete_symbol(*idx);
                true
            }
            _ => false,
        }
    }

    fn sch_library_select_symbol(&mut self, idx: usize) {
        let Some(editor) = self.active_symbol_editor_mut() else {
            tracing::warn!(
                target: "signex::library",
                idx,
                "SCH Library: select fired without an active Symbol editor"
            );
            return;
        };
        if idx >= editor.file.symbols.len() {
            tracing::warn!(
                target: "signex::library",
                idx,
                len = editor.file.symbols.len(),
                "SCH Library: select index out of range"
            );
            return;
        }
        if editor.active_idx == idx {
            return;
        }
        editor.active_idx = idx;
        editor.selected = None;
        editor.canvas_cache.clear();
        self.refresh_panel_ctx();
    }

    fn sch_library_add_symbol(&mut self) {
        let Some(editor) = self.active_symbol_editor_mut() else {
            tracing::warn!(
                target: "signex::library",
                "SCH Library: add fired without an active Symbol editor"
            );
            return;
        };
        // Pick a fresh name that doesn't collide with any existing
        // symbol in the file. `NewSymbol`, then `NewSymbol-2`, etc.
        let used: std::collections::HashSet<&str> =
            editor.file.symbols.iter().map(|s| s.name.as_str()).collect();
        let mut name = "NewSymbol".to_string();
        if used.contains(name.as_str()) {
            for n in 2..=999 {
                let candidate = format!("NewSymbol-{n}");
                if !used.contains(candidate.as_str()) {
                    name = candidate;
                    break;
                }
            }
        }
        let sym = signex_library::Symbol::empty(name);
        editor.file.symbols.push(sym);
        editor.file.updated = chrono::Utc::now();
        editor.active_idx = editor.file.symbols.len() - 1;
        editor.selected = None;
        editor.canvas_cache.clear();
        editor.dirty = true;
        let path = editor.path.clone();
        self.document_state.dirty_paths.insert(path.clone());
        if let Some(tab) = self.document_state.tabs.iter_mut().find(|t| t.path == path) {
            tab.dirty = true;
        }
        self.refresh_panel_ctx();
    }

    fn sch_library_delete_symbol(&mut self, idx: usize) {
        let Some(editor) = self.active_symbol_editor_mut() else {
            tracing::warn!(
                target: "signex::library",
                idx,
                "SCH Library: delete fired without an active Symbol editor"
            );
            return;
        };
        if editor.file.symbols.len() <= 1 {
            tracing::warn!(
                target: "signex::library",
                "SCH Library: refusing to delete the last symbol in the file"
            );
            return;
        }
        if idx >= editor.file.symbols.len() {
            tracing::warn!(
                target: "signex::library",
                idx,
                len = editor.file.symbols.len(),
                "SCH Library: delete index out of range"
            );
            return;
        }
        editor.file.symbols.remove(idx);
        editor.file.updated = chrono::Utc::now();
        // Clamp active_idx into the new range — if the user deleted
        // the active symbol or one before it, the next-best is the
        // symbol that took its slot (or the last one if we removed
        // the tail).
        if editor.active_idx >= editor.file.symbols.len() {
            editor.active_idx = editor.file.symbols.len() - 1;
        }
        editor.selected = None;
        editor.canvas_cache.clear();
        editor.dirty = true;
        let path = editor.path.clone();
        self.document_state.dirty_paths.insert(path.clone());
        if let Some(tab) = self.document_state.tabs.iter_mut().find(|t| t.path == path) {
            tab.dirty = true;
        }
        self.refresh_panel_ctx();
    }

    /// Borrow-mut the active tab's `SymbolEditorState`, if the
    /// active tab is a Symbol editor. Returns `None` for any other
    /// tab kind so the SCH Library handlers can exit fast.
    fn active_symbol_editor_mut(
        &mut self,
    ) -> Option<&mut crate::app::SymbolEditorState> {
        let path = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|t| match &t.kind {
                crate::app::TabKind::SymbolEditor(p) => Some(p.clone()),
                _ => None,
            })?;
        self.document_state.symbol_editors.get_mut(&path)
    }
}
