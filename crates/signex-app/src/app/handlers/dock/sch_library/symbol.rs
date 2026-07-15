//! Symbol-editor + SCH-library-panel state mutators — the helper
//! methods behind the `SchLibrary*` and `SymEditor*` dock-panel
//! messages. Each resolves the active `.snxsym` tab, mutates its
//! `SymbolEditorState`, marks the tab dirty, and clears the canvas
//! cache; the dispatcher in `mod.rs` routes the panel messages here.
//!
//! Pure code motion out of the former `sch_library.rs` god-file
//! (ADR-0001 #163); zero behaviour change.

use super::*;

impl Signex {
    /// Resolve the active `.snxsym` tab → its containing `.snxlib`,
    /// run `mutator` on the library's display settings, then clear
    /// the active editor's canvas cache so the change paints
    /// immediately. Silently no-ops on lone-file edits or when
    /// no Symbol editor is active.
    pub(super) fn sym_editor_mutate_display<F>(&mut self, mutator: F) -> bool
    where
        F: FnOnce(&mut crate::library::state::LibraryDisplaySettings),
    {
        let Some(path) = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|t| match &t.kind {
                crate::app::TabKind::SymbolEditor(p) => Some(p.clone()),
                _ => None,
            })
        else {
            return true;
        };
        if let Some(lib) = self.library.containing_library_mut(&path) {
            mutator(&mut lib.display);
        }
        if let Some(editor) = self.document_state.symbol_editors.get_mut(&path) {
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }

    /// Helper — apply a closure to the pin at `pin_idx` on the active
    /// Symbol editor and run the standard dirty/refresh cycle. Returns
    /// silently when no Symbol editor is active or the index is out of
    /// range so callers don't have to gate the call with their own
    /// match.
    pub(super) fn sym_editor_mutate_pin<F>(&mut self, pin_idx: usize, mutator: F) -> bool
    where
        F: FnOnce(&mut signex_library::SymbolPin),
    {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        let Some(pin) = editor.primitive_mut().pins.get_mut(pin_idx) else {
            return true;
        };
        mutator(pin);
        editor.dirty = true;
        editor.canvas_cache.clear();
        self.mark_active_symbol_tab_dirty();
        self.refresh_panel_ctx();
        true
    }

    /// Helper — apply a closure to the active symbol (`Symbol`) on
    /// the active Symbol editor. Used by Properties Component
    /// section edits (designator / comment / description / type /
    /// mirrored). Runs the standard dirty/refresh cycle. No-op when
    /// no Symbol editor is the active tab.
    pub(super) fn sym_editor_mutate_symbol<F>(&mut self, mutator: F) -> bool
    where
        F: FnOnce(&mut signex_library::Symbol),
    {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        mutator(editor.primitive_mut());
        editor.dirty = true;
        editor.canvas_cache.clear();
        self.mark_active_symbol_tab_dirty();
        self.refresh_panel_ctx();
        true
    }

    /// Helper — apply a closure to the graphic at `idx` on the active
    /// Symbol editor. Sibling of [`sym_editor_mutate_pin`] for
    /// per-shape Properties edits. Silently returns when no Symbol
    /// editor is active or the index is out of range.
    pub(super) fn sym_editor_mutate_graphic<F>(&mut self, idx: usize, mutator: F) -> bool
    where
        F: FnOnce(&mut signex_library::SymbolGraphic),
    {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        let Some(g) = editor.primitive_mut().graphics.get_mut(idx) else {
            return true;
        };
        mutator(g);
        editor.dirty = true;
        editor.canvas_cache.clear();
        self.mark_active_symbol_tab_dirty();
        self.refresh_panel_ctx();
        true
    }

    // ── Graphic-fill colour picker (transient UI state) ─────────────
    // These mirror the child-sheet colour-picker state transitions:
    // opening one picker closes the other; "advanced" flips the open
    // picker into the HSV overlay; cancel closes it. Only set / clear
    // mark the tab dirty — open / close / cancel are UI-only.

    /// Toggle the placed graphic's fill picker open / closed. Opening it
    /// closes any local-colour picker. No dirty.
    pub(super) fn sym_editor_toggle_graphic_fill_picker(&mut self, idx: usize) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        let was_open = matches!(editor.graphic_fill_picker, Some(p) if p.idx == idx);
        editor.graphic_fill_picker = if was_open {
            None
        } else {
            Some(crate::app::GraphicFillPicker {
                idx,
                advanced: false,
            })
        };
        editor.local_color_picker = None;
        self.refresh_panel_ctx();
        true
    }

    /// Expand the graphic's fill picker into the HSV / RGB overlay. No
    /// dirty.
    pub(super) fn sym_editor_open_graphic_fill_advanced(&mut self, idx: usize) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        editor.graphic_fill_picker = Some(crate::app::GraphicFillPicker {
            idx,
            advanced: true,
        });
        editor.local_color_picker = None;
        self.refresh_panel_ctx();
        true
    }

    /// Close the graphic's fill picker without committing. No dirty.
    pub(super) fn sym_editor_cancel_graphic_fill_picker(&mut self) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        editor.graphic_fill_picker = None;
        self.refresh_panel_ctx();
        true
    }

    /// Set (or clear, when `color` is `None`) the placed graphic's fill
    /// and close the picker. Dirties + clears the canvas cache.
    pub(super) fn sym_editor_set_graphic_fill(
        &mut self,
        idx: usize,
        color: Option<[u8; 4]>,
    ) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        let mut applied = false;
        if let Some(g) = editor.primitive_mut().graphics.get_mut(idx) {
            g.fill = color;
            applied = true;
        }
        editor.graphic_fill_picker = None;
        if applied {
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        if applied {
            self.mark_active_symbol_tab_dirty();
        }
        self.refresh_panel_ctx();
        true
    }

    // ── Symbol-level local-colour pickers (transient UI state) ──────

    /// Toggle a local-colour slot's picker open / closed. Opening it
    /// closes any graphic-fill picker. No dirty.
    pub(super) fn sym_editor_toggle_local_color_picker(
        &mut self,
        slot: crate::app::LocalColorSlot,
    ) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        let was_open = matches!(editor.local_color_picker, Some(p) if p.slot == slot);
        editor.local_color_picker = if was_open {
            None
        } else {
            Some(crate::app::LocalColorPicker {
                slot,
                advanced: false,
            })
        };
        editor.graphic_fill_picker = None;
        self.refresh_panel_ctx();
        true
    }

    /// Expand a local-colour slot's picker into the HSV / RGB overlay.
    /// No dirty.
    pub(super) fn sym_editor_open_local_color_advanced(
        &mut self,
        slot: crate::app::LocalColorSlot,
    ) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        editor.local_color_picker = Some(crate::app::LocalColorPicker {
            slot,
            advanced: true,
        });
        editor.graphic_fill_picker = None;
        self.refresh_panel_ctx();
        true
    }

    /// Close the local-colour picker without committing. No dirty.
    pub(super) fn sym_editor_cancel_local_color_picker(&mut self) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        editor.local_color_picker = None;
        self.refresh_panel_ctx();
        true
    }

    /// Set (or clear, when `color` is `None`) a symbol-level local
    /// colour and close the picker. Dirties + clears the canvas cache.
    pub(super) fn sym_editor_set_local_color(
        &mut self,
        slot: crate::app::LocalColorSlot,
        color: Option<[u8; 4]>,
    ) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        {
            let sym = editor.primitive_mut();
            match slot {
                crate::app::LocalColorSlot::Fill => sym.local_fill_color = color,
                crate::app::LocalColorSlot::Line => sym.local_line_color = color,
                crate::app::LocalColorSlot::Pin => sym.local_pin_color = color,
            }
        }
        editor.local_color_picker = None;
        editor.dirty = true;
        editor.canvas_cache.clear();
        self.mark_active_symbol_tab_dirty();
        self.refresh_panel_ctx();
        true
    }

    /// SCH Library panel: select a placed graphic so the right-dock
    /// Properties panel renders its per-shape fields. Mirrors
    /// [`sym_editor_select_pin`].
    pub(super) fn sym_editor_select_graphic(&mut self, idx: usize) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        if idx >= editor.primitive().graphics.len() {
            return true;
        }
        editor.selected =
            Some(crate::library::editor::symbol::state::SymbolSelection::Graphic(idx));
        editor.canvas_cache.clear();
        self.refresh_panel_ctx();
        true
    }

    /// SCH Library panel: switch the editor's `active_part` to `part`.
    /// `0` is the special Part Zero (shared pins). Clamps `part` to
    /// `[0, max_part]` so a stale tree click can't park the editor
    /// outside the symbol's actual range.
    pub(super) fn sym_editor_select_part(&mut self, part: u8) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        let max = crate::library::editor::symbol::state::max_part_number(editor.primitive());
        let clamped = if part == 0 { 0 } else { part.min(max).max(1) };
        if editor.active_part == clamped {
            return true;
        }
        editor.active_part = clamped;
        editor.canvas_cache.clear();
        self.refresh_panel_ctx();
        true
    }

    pub(super) fn sym_editor_select_pin(&mut self, pin_idx: usize) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        if pin_idx >= editor.primitive().pins.len() {
            return true;
        }
        editor.selected = Some(crate::library::editor::symbol::state::SymbolSelection::Pin(
            pin_idx,
        ));
        editor.canvas_cache.clear();
        self.refresh_panel_ctx();
        true
    }

    pub(super) fn sym_editor_set_pin_electrical(
        &mut self,
        pin_idx: usize,
        value: signex_library::PinDirection,
    ) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        if let Some(pin) = editor.primitive_mut().pins.get_mut(pin_idx) {
            pin.electrical = value;
            editor.dirty = true;
            editor.canvas_cache.clear();
            self.mark_active_symbol_tab_dirty();
            self.refresh_panel_ctx();
        }
        true
    }

    pub(super) fn sym_editor_set_pin_orientation(
        &mut self,
        pin_idx: usize,
        value: signex_library::PinOrientation,
    ) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        if let Some(pin) = editor.primitive_mut().pins.get_mut(pin_idx) {
            pin.orientation = value;
            editor.dirty = true;
            editor.canvas_cache.clear();
            self.mark_active_symbol_tab_dirty();
            self.refresh_panel_ctx();
        }
        true
    }

    pub(super) fn sym_editor_set_pin_x(&mut self, pin_idx: usize, value: f64) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        if let Some(pin) = editor.primitive_mut().pins.get_mut(pin_idx) {
            pin.position[0] = value;
            editor.dirty = true;
            editor.canvas_cache.clear();
            self.mark_active_symbol_tab_dirty();
            self.refresh_panel_ctx();
        }
        true
    }

    pub(super) fn sym_editor_set_pin_y(&mut self, pin_idx: usize, value: f64) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        if let Some(pin) = editor.primitive_mut().pins.get_mut(pin_idx) {
            pin.position[1] = value;
            editor.dirty = true;
            editor.canvas_cache.clear();
            self.mark_active_symbol_tab_dirty();
            self.refresh_panel_ctx();
        }
        true
    }

    pub(super) fn sym_editor_set_pin_number(&mut self, pin_idx: usize, value: String) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        if let Some(pin) = editor.primitive_mut().pins.get_mut(pin_idx) {
            pin.number = value;
            editor.dirty = true;
            editor.canvas_cache.clear();
            self.mark_active_symbol_tab_dirty();
            self.refresh_panel_ctx();
        }
        true
    }

    pub(super) fn sym_editor_set_pin_name(&mut self, pin_idx: usize, value: String) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        if let Some(pin) = editor.primitive_mut().pins.get_mut(pin_idx) {
            pin.name = value;
            editor.dirty = true;
            editor.canvas_cache.clear();
            self.mark_active_symbol_tab_dirty();
            self.refresh_panel_ctx();
        }
        true
    }

    pub(super) fn sym_editor_set_pin_length(&mut self, pin_idx: usize, value: f64) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        if let Some(pin) = editor.primitive_mut().pins.get_mut(pin_idx) {
            // Clamp to a sane minimum so a user dragging through 0
            // doesn't produce a degenerate stub. 0.1 mm matches the
            // smallest grid step Altium allows for pins.
            pin.length = value.max(0.1);
            editor.dirty = true;
            editor.canvas_cache.clear();
            self.mark_active_symbol_tab_dirty();
            self.refresh_panel_ctx();
        }
        true
    }

    pub(super) fn sym_editor_set_symbol_name(&mut self, value: String) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        editor.primitive_mut().name = value;
        editor.dirty = true;
        editor.canvas_cache.clear();
        self.mark_active_symbol_tab_dirty();
        self.refresh_panel_ctx();
        true
    }

    fn mark_active_symbol_tab_dirty(&mut self) {
        let Some(path) = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|t| match &t.kind {
                crate::app::TabKind::SymbolEditor(p) => Some(p.clone()),
                _ => None,
            })
        else {
            return;
        };
        self.document_state.dirty_paths.insert(path.clone());
        if let Some(tab) = self.document_state.tabs.iter_mut().find(|t| t.path == path) {
            tab.dirty = true;
        }
    }

    /// Reset per-symbol viewport state when the active symbol changes.
    ///
    /// Without this, a newly selected/created symbol can inherit stale
    /// pan/zoom from the previous symbol and open at an unexpected scale.
    fn reset_symbol_viewport(editor: &mut crate::app::SymbolEditorState) {
        editor.reset_camera_origin_center();
        editor.cursor_mm = None;
        editor.canvas_cache.clear();
    }

    pub(super) fn sch_library_select_symbol(&mut self, idx: usize) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            tracing::warn!(
                target: "signex::library",
                idx,
                "SCH Library: select fired without an active Symbol editor"
            );
            return true;
        };
        if idx >= editor.file.symbols.len() {
            tracing::warn!(
                target: "signex::library",
                idx,
                len = editor.file.symbols.len(),
                "SCH Library: select index out of range"
            );
            return true;
        }
        if editor.active_idx == idx {
            return true;
        }
        editor.active_idx = idx;
        editor.selected = None;
        // Active part is per-editor but only meaningful for the
        // currently-active symbol; switching symbols resets to part 1
        // so the new symbol's pin filter starts in a sane state.
        editor.active_part = 1;
        Self::reset_symbol_viewport(editor);
        self.refresh_panel_ctx();
        true
    }

    pub(super) fn sch_library_add_symbol(&mut self) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            tracing::warn!(
                target: "signex::library",
                "SCH Library: add fired without an active Symbol editor"
            );
            return true;
        };
        // Pick a fresh name that doesn't collide with any existing
        // symbol in the file. `NewSymbol`, then `NewSymbol-2`, etc.
        let used: std::collections::HashSet<&str> = editor
            .file
            .symbols
            .iter()
            .map(|s| s.name.as_str())
            .collect();
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
        editor.active_part = 1;
        editor.selected = None;
        Self::reset_symbol_viewport(editor);
        editor.dirty = true;
        let path = editor.path.clone();
        self.document_state.dirty_paths.insert(path.clone());
        if let Some(tab) = self.document_state.tabs.iter_mut().find(|t| t.path == path) {
            tab.dirty = true;
        }
        self.refresh_panel_ctx();
        true
    }

    pub(super) fn sch_library_delete_symbol(&mut self, idx: usize) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            tracing::warn!(
                target: "signex::library",
                idx,
                "SCH Library: delete fired without an active Symbol editor"
            );
            return true;
        };
        if editor.file.symbols.len() <= 1 {
            tracing::warn!(
                target: "signex::library",
                "SCH Library: refusing to delete the last symbol in the file"
            );
            return true;
        }
        if idx >= editor.file.symbols.len() {
            tracing::warn!(
                target: "signex::library",
                idx,
                len = editor.file.symbols.len(),
                "SCH Library: delete index out of range"
            );
            return true;
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
        editor.active_part = 1;
        editor.selected = None;
        Self::reset_symbol_viewport(editor);
        editor.dirty = true;
        let path = editor.path.clone();
        self.document_state.dirty_paths.insert(path.clone());
        if let Some(tab) = self.document_state.tabs.iter_mut().find(|t| t.path == path) {
            tab.dirty = true;
        }
        self.refresh_panel_ctx();
        true
    }

    /// Borrow-mut the active tab's `SymbolEditorState`, if the
    /// active tab is a Symbol editor. Returns `None` for any other
    /// tab kind so the SCH Library handlers can exit fast.
    fn active_symbol_editor_mut(&mut self) -> Option<&mut crate::app::SymbolEditorState> {
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
