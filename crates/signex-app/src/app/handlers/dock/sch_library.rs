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
            crate::panels::PanelMsg::SymEditorSetPinNumber { pin_idx, value } => {
                self.sym_editor_set_pin_number(*pin_idx, value.clone());
                true
            }
            crate::panels::PanelMsg::SymEditorSetPinName { pin_idx, value } => {
                self.sym_editor_set_pin_name(*pin_idx, value.clone());
                true
            }
            crate::panels::PanelMsg::SymEditorSetPinLength { pin_idx, value } => {
                self.sym_editor_set_pin_length(*pin_idx, *value);
                true
            }
            crate::panels::PanelMsg::SymEditorSetSymbolName(value) => {
                self.sym_editor_set_symbol_name(value.clone());
                true
            }
            crate::panels::PanelMsg::SymEditorSetPinElectrical { pin_idx, value } => {
                self.sym_editor_set_pin_electrical(*pin_idx, *value);
                true
            }
            crate::panels::PanelMsg::SymEditorSetPinOrientation { pin_idx, value } => {
                self.sym_editor_set_pin_orientation(*pin_idx, *value);
                true
            }
            crate::panels::PanelMsg::SymEditorSetPinX { pin_idx, value } => {
                self.sym_editor_set_pin_x(*pin_idx, *value);
                true
            }
            crate::panels::PanelMsg::SymEditorSetPinY { pin_idx, value } => {
                self.sym_editor_set_pin_y(*pin_idx, *value);
                true
            }
            crate::panels::PanelMsg::SymEditorSelectPin(idx) => {
                self.sym_editor_select_pin(*idx);
                true
            }
            crate::panels::PanelMsg::SymEditorSetPinDescription { pin_idx, value } => {
                self.sym_editor_mutate_pin(*pin_idx, |pin| pin.description = value.clone());
                true
            }
            crate::panels::PanelMsg::SymEditorSetPinFunctionCsv { pin_idx, value } => {
                let parsed: Vec<String> = value
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect();
                self.sym_editor_mutate_pin(*pin_idx, move |pin| {
                    pin.function = parsed.clone();
                });
                true
            }
            crate::panels::PanelMsg::SymEditorTogglePinDesignatorVisible(pin_idx) => {
                self.sym_editor_mutate_pin(*pin_idx, |pin| {
                    pin.designator_visible = !pin.designator_visible;
                });
                true
            }
            crate::panels::PanelMsg::SymEditorTogglePinNameVisible(pin_idx) => {
                self.sym_editor_mutate_pin(*pin_idx, |pin| {
                    pin.name_visible = !pin.name_visible;
                });
                true
            }
            crate::panels::PanelMsg::SymEditorTogglePinHidden(pin_idx) => {
                self.sym_editor_mutate_pin(*pin_idx, |pin| pin.hidden = !pin.hidden);
                true
            }
            crate::panels::PanelMsg::SymEditorTogglePinLocked(pin_idx) => {
                self.sym_editor_mutate_pin(*pin_idx, |pin| pin.locked = !pin.locked);
                true
            }
            crate::panels::PanelMsg::SymEditorSetPinSymbol {
                pin_idx,
                slot,
                value,
            } => {
                let slot = *slot;
                let value = *value;
                self.sym_editor_mutate_pin(*pin_idx, move |pin| match slot {
                    0 => pin.inside_symbol = value,
                    1 => pin.inside_edge_symbol = value,
                    2 => pin.outside_edge_symbol = value,
                    3 => pin.outside_symbol = value,
                    _ => {}
                });
                true
            }
            crate::panels::PanelMsg::SymEditorSetPinPartNumber { pin_idx, value } => {
                let value = *value;
                self.sym_editor_mutate_pin(*pin_idx, move |pin| pin.part_number = value);
                true
            }
            crate::panels::PanelMsg::SymEditorSelectGraphic(idx) => {
                self.sym_editor_select_graphic(*idx);
                true
            }
            crate::panels::PanelMsg::SymEditorSelectPart(part) => {
                self.sym_editor_select_part(*part);
                true
            }
            crate::panels::PanelMsg::SymEditorSetGraphicField { idx, field, value } => {
                let field = *field;
                let value = *value;
                self.sym_editor_mutate_graphic(*idx, move |g| {
                    apply_graphic_field(g, field, value);
                });
                true
            }
            crate::panels::PanelMsg::SymEditorSetGraphicText { idx, value } => {
                let value = value.clone();
                self.sym_editor_mutate_graphic(*idx, move |g| {
                    if let signex_library::SymbolGraphicKind::Text { content, .. } = &mut g.kind {
                        *content = value.clone();
                    }
                });
                true
            }
            crate::panels::PanelMsg::SymEditorSetSymbolDesignator(value) => {
                let v = value.clone();
                self.sym_editor_mutate_symbol(move |s| s.designator = v);
                true
            }
            crate::panels::PanelMsg::SymEditorSetSymbolComment(value) => {
                let v = value.clone();
                self.sym_editor_mutate_symbol(move |s| s.comment = v);
                true
            }
            crate::panels::PanelMsg::SymEditorSetSymbolDescription(value) => {
                let v = value.clone();
                self.sym_editor_mutate_symbol(move |s| s.description = v);
                true
            }
            crate::panels::PanelMsg::SymEditorSetSymbolType(value) => {
                let v = *value;
                self.sym_editor_mutate_symbol(move |s| s.component_type = v);
                true
            }
            crate::panels::PanelMsg::SymEditorToggleSymbolMirrored => {
                self.sym_editor_mutate_symbol(|s| s.mirrored = !s.mirrored);
                true
            }
            crate::panels::PanelMsg::SymEditorCycleLocalFillColor => {
                self.sym_editor_mutate_symbol(|s| {
                    s.local_fill_color = cycle_local_color(s.local_fill_color);
                });
                true
            }
            crate::panels::PanelMsg::SymEditorCycleLocalLineColor => {
                self.sym_editor_mutate_symbol(|s| {
                    s.local_line_color = cycle_local_color(s.local_line_color);
                });
                true
            }
            crate::panels::PanelMsg::SymEditorCycleLocalPinColor => {
                self.sym_editor_mutate_symbol(|s| {
                    s.local_pin_color = cycle_local_color(s.local_pin_color);
                });
                true
            }
            crate::panels::PanelMsg::SymEditorSetDisplaySheetColor(color) => {
                let color = *color;
                self.sym_editor_mutate_display(|d| d.sheet_color = color);
                true
            }
            crate::panels::PanelMsg::SymEditorToggleDisplayGrid => {
                self.sym_editor_mutate_display(|d| d.grid_visible = !d.grid_visible);
                true
            }
            crate::panels::PanelMsg::SymEditorCycleDisplayGridSize => {
                self.sym_editor_mutate_display(|d| {
                    let sizes = crate::canvas::grid::GRID_SIZES_MM;
                    let i = sizes
                        .iter()
                        .position(|s| (s - d.grid_size_mm).abs() < f32::EPSILON)
                        .unwrap_or(2);
                    d.grid_size_mm = sizes[(i + 1) % sizes.len()];
                });
                true
            }
            crate::panels::PanelMsg::SymEditorCycleDisplayUnit => {
                self.sym_editor_mutate_display(|d| {
                    use signex_types::coord::Unit;
                    d.unit = match d.unit {
                        Unit::Mm => Unit::Mil,
                        Unit::Mil => Unit::Inch,
                        Unit::Inch => Unit::Micrometer,
                        Unit::Micrometer => Unit::Mm,
                    };
                });
                true
            }
            _ => false,
        }
    }

    /// Resolve the active `.snxsym` tab → its containing `.snxlib`,
    /// run `mutator` on the library's display settings, then clear
    /// the active editor's canvas cache so the change paints
    /// immediately. Silently no-ops on lone-file edits or when
    /// no Symbol editor is active.
    fn sym_editor_mutate_display<F>(&mut self, mutator: F)
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
            return;
        };
        if let Some(lib) = self.library.containing_library_mut(&path) {
            mutator(&mut lib.display);
        }
        if let Some(editor) = self.document_state.symbol_editors.get_mut(&path) {
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
    }

    /// Helper — apply a closure to the pin at `pin_idx` on the active
    /// Symbol editor and run the standard dirty/refresh cycle. Returns
    /// silently when no Symbol editor is active or the index is out of
    /// range so callers don't have to gate the call with their own
    /// match.
    fn sym_editor_mutate_pin<F>(&mut self, pin_idx: usize, mutator: F)
    where
        F: FnOnce(&mut signex_library::SymbolPin),
    {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return;
        };
        let Some(pin) = editor.primitive_mut().pins.get_mut(pin_idx) else {
            return;
        };
        mutator(pin);
        editor.dirty = true;
        editor.canvas_cache.clear();
        self.mark_active_symbol_tab_dirty();
        self.refresh_panel_ctx();
    }

    /// Helper — apply a closure to the active symbol (`Symbol`) on
    /// the active Symbol editor. Used by Properties Component
    /// section edits (designator / comment / description / type /
    /// mirrored). Runs the standard dirty/refresh cycle. No-op when
    /// no Symbol editor is the active tab.
    fn sym_editor_mutate_symbol<F>(&mut self, mutator: F)
    where
        F: FnOnce(&mut signex_library::Symbol),
    {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return;
        };
        mutator(editor.primitive_mut());
        editor.dirty = true;
        editor.canvas_cache.clear();
        self.mark_active_symbol_tab_dirty();
        self.refresh_panel_ctx();
    }

    /// Helper — apply a closure to the graphic at `idx` on the active
    /// Symbol editor. Sibling of [`sym_editor_mutate_pin`] for
    /// per-shape Properties edits. Silently returns when no Symbol
    /// editor is active or the index is out of range.
    fn sym_editor_mutate_graphic<F>(&mut self, idx: usize, mutator: F)
    where
        F: FnOnce(&mut signex_library::SymbolGraphic),
    {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return;
        };
        let Some(g) = editor.primitive_mut().graphics.get_mut(idx) else {
            return;
        };
        mutator(g);
        editor.dirty = true;
        editor.canvas_cache.clear();
        self.mark_active_symbol_tab_dirty();
        self.refresh_panel_ctx();
    }

    /// SCH Library panel: select a placed graphic so the right-dock
    /// Properties panel renders its per-shape fields. Mirrors
    /// [`sym_editor_select_pin`].
    fn sym_editor_select_graphic(&mut self, idx: usize) {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return;
        };
        if idx >= editor.primitive().graphics.len() {
            return;
        }
        editor.selected =
            Some(crate::library::editor::symbol::state::SymbolSelection::Graphic(idx));
        editor.canvas_cache.clear();
        self.refresh_panel_ctx();
    }

    /// SCH Library panel: switch the editor's `active_part` to `part`.
    /// `0` is the special Part Zero (shared pins). Clamps `part` to
    /// `[0, max_part]` so a stale tree click can't park the editor
    /// outside the symbol's actual range.
    fn sym_editor_select_part(&mut self, part: u8) {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return;
        };
        let max = crate::library::editor::symbol::state::max_part_number(editor.primitive());
        let clamped = if part == 0 { 0 } else { part.min(max).max(1) };
        if editor.active_part == clamped {
            return;
        }
        editor.active_part = clamped;
        editor.canvas_cache.clear();
        self.refresh_panel_ctx();
    }

    fn sym_editor_select_pin(&mut self, pin_idx: usize) {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return;
        };
        if pin_idx >= editor.primitive().pins.len() {
            return;
        }
        editor.selected = Some(crate::library::editor::symbol::state::SymbolSelection::Pin(
            pin_idx,
        ));
        editor.canvas_cache.clear();
        self.refresh_panel_ctx();
    }

    fn sym_editor_set_pin_electrical(
        &mut self,
        pin_idx: usize,
        value: signex_library::PinElectricalType,
    ) {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return;
        };
        if let Some(pin) = editor.primitive_mut().pins.get_mut(pin_idx) {
            pin.electrical = value;
            editor.dirty = true;
            editor.canvas_cache.clear();
            self.mark_active_symbol_tab_dirty();
            self.refresh_panel_ctx();
        }
    }

    fn sym_editor_set_pin_orientation(
        &mut self,
        pin_idx: usize,
        value: signex_library::PinOrientation,
    ) {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return;
        };
        if let Some(pin) = editor.primitive_mut().pins.get_mut(pin_idx) {
            pin.orientation = value;
            editor.dirty = true;
            editor.canvas_cache.clear();
            self.mark_active_symbol_tab_dirty();
            self.refresh_panel_ctx();
        }
    }

    fn sym_editor_set_pin_x(&mut self, pin_idx: usize, value: f64) {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return;
        };
        if let Some(pin) = editor.primitive_mut().pins.get_mut(pin_idx) {
            pin.position[0] = value;
            editor.dirty = true;
            editor.canvas_cache.clear();
            self.mark_active_symbol_tab_dirty();
            self.refresh_panel_ctx();
        }
    }

    fn sym_editor_set_pin_y(&mut self, pin_idx: usize, value: f64) {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return;
        };
        if let Some(pin) = editor.primitive_mut().pins.get_mut(pin_idx) {
            pin.position[1] = value;
            editor.dirty = true;
            editor.canvas_cache.clear();
            self.mark_active_symbol_tab_dirty();
            self.refresh_panel_ctx();
        }
    }

    fn sym_editor_set_pin_number(&mut self, pin_idx: usize, value: String) {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return;
        };
        if let Some(pin) = editor.primitive_mut().pins.get_mut(pin_idx) {
            pin.number = value;
            editor.dirty = true;
            editor.canvas_cache.clear();
            self.mark_active_symbol_tab_dirty();
            self.refresh_panel_ctx();
        }
    }

    fn sym_editor_set_pin_name(&mut self, pin_idx: usize, value: String) {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return;
        };
        if let Some(pin) = editor.primitive_mut().pins.get_mut(pin_idx) {
            pin.name = value;
            editor.dirty = true;
            editor.canvas_cache.clear();
            self.mark_active_symbol_tab_dirty();
            self.refresh_panel_ctx();
        }
    }

    fn sym_editor_set_pin_length(&mut self, pin_idx: usize, value: f64) {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return;
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
    }

    fn sym_editor_set_symbol_name(&mut self, value: String) {
        let Some(editor) = self.active_symbol_editor_mut() else {
            return;
        };
        editor.primitive_mut().name = value;
        editor.dirty = true;
        editor.canvas_cache.clear();
        self.mark_active_symbol_tab_dirty();
        self.refresh_panel_ctx();
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
        // Active part is per-editor but only meaningful for the
        // currently-active symbol; switching symbols resets to part 1
        // so the new symbol's pin filter starts in a sane state.
        editor.active_part = 1;
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

/// Apply one numeric Properties-pane edit to a graphic. (idx, field)
/// pairs whose field doesn't apply to the graphic's variant silently
/// no-op so a stale Properties pane can't mutate the wrong slot.
/// Click-to-cycle the symbol's local color override through a small
/// preset palette and back to `None` (= inherit). 5 steps total:
/// None → red → green → blue → yellow → back to None.
fn cycle_local_color(current: Option<[u8; 4]>) -> Option<[u8; 4]> {
    const PALETTE: &[[u8; 4]] = &[
        [220, 60, 60, 255],  // red
        [60, 180, 80, 255],  // green
        [60, 110, 220, 255], // blue
        [240, 200, 80, 255], // yellow
    ];
    match current {
        None => Some(PALETTE[0]),
        Some(c) => match PALETTE.iter().position(|p| *p == c) {
            Some(i) if i + 1 < PALETTE.len() => Some(PALETTE[i + 1]),
            _ => None,
        },
    }
}

fn apply_graphic_field(
    g: &mut signex_library::SymbolGraphic,
    field: crate::panels::GraphicFieldId,
    value: f64,
) {
    use crate::panels::GraphicFieldId;
    use signex_library::SymbolGraphicKind;
    if matches!(field, GraphicFieldId::StrokeWidth) {
        g.stroke_width = value.max(0.0);
        return;
    }
    match (&mut g.kind, field) {
        (
            SymbolGraphicKind::Rectangle { from, .. } | SymbolGraphicKind::Line { from, .. },
            GraphicFieldId::FromX,
        ) => from[0] = value,
        (
            SymbolGraphicKind::Rectangle { from, .. } | SymbolGraphicKind::Line { from, .. },
            GraphicFieldId::FromY,
        ) => from[1] = value,
        (
            SymbolGraphicKind::Rectangle { to, .. } | SymbolGraphicKind::Line { to, .. },
            GraphicFieldId::ToX,
        ) => to[0] = value,
        (
            SymbolGraphicKind::Rectangle { to, .. } | SymbolGraphicKind::Line { to, .. },
            GraphicFieldId::ToY,
        ) => to[1] = value,
        (
            SymbolGraphicKind::Circle { center, .. } | SymbolGraphicKind::Arc { center, .. },
            GraphicFieldId::CenterX,
        ) => center[0] = value,
        (
            SymbolGraphicKind::Circle { center, .. } | SymbolGraphicKind::Arc { center, .. },
            GraphicFieldId::CenterY,
        ) => center[1] = value,
        (
            SymbolGraphicKind::Circle { radius, .. } | SymbolGraphicKind::Arc { radius, .. },
            GraphicFieldId::Radius,
        ) => *radius = value.max(0.1),
        (SymbolGraphicKind::Arc { start_deg, .. }, GraphicFieldId::StartDeg) => *start_deg = value,
        (SymbolGraphicKind::Arc { end_deg, .. }, GraphicFieldId::EndDeg) => *end_deg = value,
        (SymbolGraphicKind::Text { position, .. }, GraphicFieldId::PositionX) => {
            position[0] = value
        }
        (SymbolGraphicKind::Text { position, .. }, GraphicFieldId::PositionY) => {
            position[1] = value
        }
        (SymbolGraphicKind::Text { size, .. }, GraphicFieldId::TextSize) => *size = value.max(0.1),
        _ => {}
    }
}
