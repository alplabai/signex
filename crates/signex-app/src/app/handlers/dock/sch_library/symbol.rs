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

    /// Run `edit` on the active Symbol editor as one recorded undo step
    /// (coalesced per `key`, see `SymbolEditorState::record_edit`), then
    /// run the standard dirty / refresh cycle. `edit` returns whether it
    /// changed anything. Silently no-ops when no Symbol editor is active,
    /// so callers need no match of their own.
    fn sym_editor_record_edit(
        &mut self,
        key: Option<crate::app::documents::FieldEditKey>,
        edit: impl FnOnce(&mut crate::app::SymbolEditorState) -> bool,
    ) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return true;
        };
        let applied = editor.record_edit(key, |editor| {
            if !edit(editor) {
                return false;
            }
            editor.dirty = true;
            editor.canvas_cache.clear();
            true
        });
        if applied {
            self.mark_active_symbol_tab_dirty();
        }
        self.refresh_panel_ctx();
        true
    }

    /// Helper — apply a closure to the pin at `pin_idx` on the active
    /// Symbol editor as one undo step. Returns silently when no Symbol
    /// editor is active or the index is out of range.
    pub(super) fn sym_editor_mutate_pin<F>(
        &mut self,
        pin_idx: usize,
        key: Option<&'static str>,
        mutator: F,
    ) -> bool
    where
        F: FnOnce(&mut signex_library::SymbolPin),
    {
        self.sym_editor_record_edit(key.map(|k| (k, pin_idx as u128)), |editor| {
            let Some(pin) = editor.primitive_mut().pins.get_mut(pin_idx) else {
                return false;
            };
            mutator(pin);
            true
        })
    }

    /// Helper — apply a closure to the active symbol (`Symbol`) as one
    /// undo step. Used by Properties Component section edits
    /// (designator / comment / description / type / mirrored).
    pub(super) fn sym_editor_mutate_symbol<F>(
        &mut self,
        key: Option<&'static str>,
        mutator: F,
    ) -> bool
    where
        F: FnOnce(&mut signex_library::Symbol),
    {
        self.sym_editor_record_edit(key.map(|k| (k, 0)), |editor| {
            mutator(editor.primitive_mut());
            true
        })
    }

    /// Helper — apply a closure to the graphic at `idx` on the active
    /// Symbol editor as one undo step. Sibling of
    /// [`sym_editor_mutate_pin`] for per-shape Properties edits. Silently
    /// returns when no Symbol editor is active or the index is out of
    /// range. `key` is the field edited; it is scoped to `idx` here.
    pub(super) fn sym_editor_mutate_graphic<F>(
        &mut self,
        idx: usize,
        key: Option<(&'static str, u128)>,
        mutator: F,
    ) -> bool
    where
        F: FnOnce(&mut signex_library::SymbolGraphic),
    {
        let key = key.map(|(name, field)| (name, ((idx as u128) << 8) | field));
        self.sym_editor_record_edit(key, |editor| {
            let Some(g) = editor.primitive_mut().graphics.get_mut(idx) else {
                return false;
            };
            mutator(g);
            true
        })
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
        self.sym_editor_record_edit(None, |editor| {
            editor.graphic_fill_picker = None;
            let Some(g) = editor.primitive_mut().graphics.get_mut(idx) else {
                return false;
            };
            g.fill = color;
            true
        })
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
        self.sym_editor_record_edit(None, |editor| {
            editor.local_color_picker = None;
            let sym = editor.primitive_mut();
            match slot {
                crate::app::LocalColorSlot::Fill => sym.local_fill_color = color,
                crate::app::LocalColorSlot::Line => sym.local_line_color = color,
                crate::app::LocalColorSlot::Pin => sym.local_pin_color = color,
            }
            true
        })
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
        self.sym_editor_mutate_pin(pin_idx, None, |pin| pin.electrical = value)
    }

    pub(super) fn sym_editor_set_pin_orientation(
        &mut self,
        pin_idx: usize,
        value: signex_library::PinOrientation,
    ) -> bool {
        self.sym_editor_mutate_pin(pin_idx, None, |pin| pin.orientation = value)
    }

    pub(super) fn sym_editor_set_pin_x(&mut self, pin_idx: usize, value: f64) -> bool {
        self.sym_editor_mutate_pin(pin_idx, Some("pin_x"), |pin| pin.position[0] = value)
    }

    pub(super) fn sym_editor_set_pin_y(&mut self, pin_idx: usize, value: f64) -> bool {
        self.sym_editor_mutate_pin(pin_idx, Some("pin_y"), |pin| pin.position[1] = value)
    }

    pub(super) fn sym_editor_set_pin_number(&mut self, pin_idx: usize, value: String) -> bool {
        self.sym_editor_mutate_pin(pin_idx, Some("pin_number"), |pin| pin.number = value)
    }

    pub(super) fn sym_editor_set_pin_name(&mut self, pin_idx: usize, value: String) -> bool {
        self.sym_editor_mutate_pin(pin_idx, Some("pin_name"), |pin| pin.name = value)
    }

    pub(super) fn sym_editor_set_pin_length(&mut self, pin_idx: usize, value: f64) -> bool {
        // Clamp to a sane minimum so a user dragging through 0
        // doesn't produce a degenerate stub. 0.1 mm matches the
        // smallest grid step Altium allows for pins.
        self.sym_editor_mutate_pin(pin_idx, Some("pin_length"), |pin| {
            pin.length = value.max(0.1)
        })
    }

    pub(super) fn sym_editor_set_symbol_name(&mut self, value: String) -> bool {
        self.sym_editor_mutate_symbol(Some("symbol_name"), |symbol| symbol.name = value)
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

    pub(super) fn sch_library_select_symbol(&mut self, idx: usize) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            crate::diagnostics::log_warning(
                "SCH Library: select fired without an active Symbol editor",
            );
            return true;
        };
        if idx >= editor.file.symbols.len() {
            crate::diagnostics::log_warning(format!(
                "{}: symbol select index {idx} is out of range ({} symbols)",
                editor.path.display(),
                editor.file.symbols.len()
            ));
            return true;
        }
        if editor.active_idx == idx {
            return true;
        }
        editor.active_idx = idx;
        // Switching symbols resets the unit, selection and viewport so
        // the new symbol does not inherit the previous one's.
        editor.reset_view_for_symbol_switch();
        self.refresh_panel_ctx();
        true
    }

    pub(super) fn sch_library_add_symbol(&mut self) -> bool {
        let Some(editor) = self.active_symbol_editor_mut() else {
            crate::diagnostics::log_warning(
                "SCH Library: add fired without an active Symbol editor",
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
        // The add is its own undo step (#760 review): snapshot the whole
        // container first so Undo drops only the new symbol and leaves
        // earlier edits to the other symbols alone.
        let snapshot = editor.snapshot();
        editor.commit_history(snapshot);
        let sym = signex_library::Symbol::empty(name);
        editor.file.symbols.push(sym);
        editor.file.updated = chrono::Utc::now();
        editor.active_idx = editor.file.symbols.len() - 1;
        editor.reset_view_for_symbol_switch();
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
            crate::diagnostics::log_warning(
                "SCH Library: delete fired without an active Symbol editor",
            );
            return true;
        };
        if editor.file.symbols.len() <= 1 {
            crate::diagnostics::log_warning(format!(
                "{}: refused to delete the last symbol in the file",
                editor.path.display()
            ));
            return true;
        }
        if idx >= editor.file.symbols.len() {
            crate::diagnostics::log_warning(format!(
                "{}: symbol delete index {idx} is out of range ({} symbols)",
                editor.path.display(),
                editor.file.symbols.len()
            ));
            return true;
        }
        // The delete is its own undo step (#760): snapshot the whole
        // container first so Undo restores the deleted symbol and leaves
        // the previous edit alone.
        let snapshot = editor.snapshot();
        editor.commit_history(snapshot);
        editor.file.symbols.remove(idx);
        editor.file.updated = chrono::Utc::now();
        // A sibling before the active one shifted it down a slot: follow
        // it. Deleting the active symbol lands on the one that took its
        // slot (or the last, when the tail went).
        let active_moved = idx <= editor.active_idx;
        if idx < editor.active_idx {
            editor.active_idx -= 1;
        } else if editor.active_idx >= editor.file.symbols.len() {
            editor.active_idx = editor.file.symbols.len() - 1;
        }
        if active_moved {
            editor.reset_view_for_symbol_switch();
        }
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
