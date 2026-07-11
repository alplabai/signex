//! Footprint-editor active-tab accessors and pad-property setters —
//! the helper methods behind the `FpEditor*` pad-defaults / selected-
//! pad / sketch-pad dock-panel messages. They mutate `next_pad_defaults`
//! or the selected pad on the active `.snxfpt` editor and re-bake as
//! needed; the dispatcher in `mod.rs` routes the panel messages here.
//!
//! Pure code motion out of the former `sch_library.rs` god-file
//! (ADR-0001 #163); zero behaviour change.

use super::super::*;

impl Signex {
    /// v0.16.3 — sibling of [`Self::active_symbol_editor_mut`] for
    /// `.snxfpt` editor tabs. Drives the Properties-panel pad-defaults
    /// form so it can mutate `next_pad_defaults` without round-
    /// tripping through `LibraryMessage::PrimitiveEditorEvent`.
    /// Read-only sibling of [`active_footprint_editor_mut`].
    /// v0.18.11 — used by the Grid Properties modal open handler
    /// to seed the dialog buffers from the live snap step.
    pub(crate) fn active_footprint_editor(&self) -> Option<&crate::app::FootprintEditorState> {
        let path = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|t| match &t.kind {
                crate::app::TabKind::FootprintEditor(p) => Some(p.clone()),
                _ => None,
            })?;
        self.document_state.footprint_editors.get(&path)
    }

    pub(crate) fn active_footprint_editor_mut(
        &mut self,
    ) -> Option<&mut crate::app::FootprintEditorState> {
        let path = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
            .and_then(|t| match &t.kind {
                crate::app::TabKind::FootprintEditor(p) => Some(p.clone()),
                _ => None,
            })?;
        self.document_state.footprint_editors.get_mut(&path)
    }

    pub(crate) fn fp_editor_set_next_pad_designator(&mut self, value: String) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.designator_override =
                if value.is_empty() { None } else { Some(value) };
        }
        self.refresh_panel_ctx();
        true
    }

    pub(crate) fn fp_editor_set_next_pad_size_x(&mut self, value: String) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Ok(parsed) = value.trim().parse::<f64>() {
                if parsed > 0.0 {
                    editor.state.next_pad_defaults.size_x_mm = parsed;
                }
            }
        }
        self.refresh_panel_ctx();
        true
    }

    pub(crate) fn fp_editor_set_next_pad_size_y(&mut self, value: String) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Ok(parsed) = value.trim().parse::<f64>() {
                if parsed > 0.0 {
                    editor.state.next_pad_defaults.size_y_mm = parsed;
                }
            }
        }
        self.refresh_panel_ctx();
        true
    }

    pub(crate) fn fp_editor_set_next_pad_side(
        &mut self,
        side: crate::library::editor::footprint::state::PadSide,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.side = side;
        }
        self.refresh_panel_ctx();
        true
    }

    pub(crate) fn fp_editor_set_next_pad_rotation(&mut self, value: String) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Ok(parsed) = value.trim().parse::<f64>() {
                editor.state.next_pad_defaults.rotation_deg = parsed;
            }
        }
        self.refresh_panel_ctx();
        true
    }

    pub(crate) fn fp_editor_set_selected_pad_rotation(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Ok(parsed) = value.trim().parse::<f64>() {
                if let Some(pad) = editor.state.pads.get_mut(idx) {
                    pad.rotation_deg = parsed;
                    editor.with_parts(|state, primitive| {
                        crate::library::editor::footprint::state::FootprintEditorState::sync_pads_to_primitive(state, primitive);
                    });
                    editor.dirty = true;
                    editor.canvas_cache.clear();
                }
            }
        }
        self.refresh_panel_ctx();
        true
    }

    // v0.20 — Altium-parity Pad Properties / Pad Stack / Pad Features
    // form handlers. Each method mutates a slice of
    // `editor.state.next_pad_defaults` so the next `add_pad_at` mints
    // a pad with the user-selected stack / feature / testpoint
    // configuration. None of these are dirty-marking on their own —
    // they're "pre-placement defaults" — but the panel `refresh` runs
    // so the form re-reads the new value.
    pub(crate) fn fp_editor_set_next_pad_shape(&mut self, shape: signex_library::PadShape) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.shape = shape;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_next_pad_kind(&mut self, kind: signex_library::PadKind) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.kind = kind;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_next_pad_drill_diameter(&mut self, value: String) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.drill_diameter_mm = fp_parse_optional_mm(&value);
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_next_pad_drill_slot_length(&mut self, value: String) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.drill_slot_length_mm = fp_parse_optional_mm(&value);
        }
        self.refresh_panel_ctx();
        true
    }
    /// v0.20 — Hole shape pick_list. Round / Slot. The picker is a
    /// shortcut: Round clears slot_length; Slot defaults it to
    /// 1.5× drill diameter (or 1mm if no drill yet).
    pub(crate) fn fp_editor_set_next_pad_hole_shape_round(&mut self) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.drill_slot_length_mm = None;
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_set_next_pad_hole_shape_slot(&mut self) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            let default_slot = editor
                .state
                .next_pad_defaults
                .drill_diameter_mm
                .map(|d| d * 1.5)
                .unwrap_or(1.0);
            editor.state.next_pad_defaults.drill_slot_length_mm = Some(default_slot);
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_set_next_pad_corner_radius_pct(&mut self, value: String) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            let parsed = value.trim().parse::<f64>().ok();
            editor.state.next_pad_defaults.stack.corner_radius_pct =
                parsed.filter(|v| (0.0..=50.0).contains(v));
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_next_pad_template(&mut self, value: String) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.template = value;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_next_pad_template_library(&mut self, value: String) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.template_library = value;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_next_pad_paste_margin_top(&mut self, value: String) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.paste_margin_top = fp_parse_optional_mm(&value);
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_next_pad_paste_margin_bottom(&mut self, value: String) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.paste_margin_bottom = fp_parse_optional_mm(&value);
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_toggle_next_pad_paste_enabled_top(&mut self, on: bool) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.paste_enabled_top = on;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_toggle_next_pad_paste_enabled_bottom(&mut self, on: bool) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.paste_enabled_bottom = on;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_next_pad_mask_margin_top(&mut self, value: String) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.mask_margin_top = fp_parse_optional_mm(&value);
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_next_pad_mask_margin_bottom(&mut self, value: String) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.mask_margin_bottom = fp_parse_optional_mm(&value);
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_toggle_next_pad_mask_tented_top(&mut self, on: bool) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.mask_tented_top = on;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_toggle_next_pad_mask_tented_bottom(&mut self, on: bool) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.mask_tented_bottom = on;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_toggle_next_pad_thermal_relief(&mut self, on: bool) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.thermal_relief = on;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_next_pad_feature_top(
        &mut self,
        f: signex_sketch::attr::PadFeature,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.feature_top = f;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_next_pad_feature_bottom(
        &mut self,
        f: signex_sketch::attr::PadFeature,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.feature_bottom = f;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_toggle_next_pad_testpoint_top_assembly(&mut self, on: bool) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.testpoint.top_assembly = on;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_toggle_next_pad_testpoint_top_fab(&mut self, on: bool) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.testpoint.top_fab = on;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_toggle_next_pad_testpoint_bottom_assembly(&mut self, on: bool) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.testpoint.bottom_assembly = on;
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_toggle_next_pad_testpoint_bottom_fab(&mut self, on: bool) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.testpoint.bottom_fab = on;
        }
        self.refresh_panel_ctx();
        true
    }

    // v0.20 — Selected-pad editing handlers. Each one mutates a slice
    // of `state.pads[idx]` and dirty-marks the editor + clears the
    // canvas cache so the new value renders. The `with_parts` block
    // syncs the pad list back onto the underlying primitive so the
    // saved file picks up the change.
    pub(in crate::app::handlers::dock::sch_library) fn with_selected_pad<F>(&mut self, idx: usize, f: F) -> bool
    where
        F: FnOnce(&mut crate::library::editor::footprint::state::EditorPad),
    {
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(pad) = editor.state.pads.get_mut(idx) {
                f(pad);
                editor.with_parts(|state, primitive| {
                    crate::library::editor::footprint::state::FootprintEditorState::sync_pads_to_primitive(state, primitive);
                });
                editor.dirty = true;
                editor.canvas_cache.clear();
            }
        }
        self.refresh_panel_ctx();
        true
    }

    /// v0.21 — sketch-mode counterpart of `with_selected_pad`. Looks
    /// up the sketch entity by id, runs the closure on its `PadAttr`
    /// (creating one only if it already exists; non-pad entities are
    /// silently skipped), then dirty-marks the editor + clears the
    /// canvas cache. Solve+bake is queued on the next mutation cycle.
    pub(in crate::app::handlers::dock::sch_library) fn with_selected_sketch_pad<F>(
        &mut self,
        id: signex_sketch::id::SketchEntityId,
        f: F,
    ) -> bool
    where
        F: FnOnce(&mut signex_sketch::attr::PadAttr),
    {
        if let Some(editor) = self.active_footprint_editor_mut() {
            let sketch = editor.primitive_mut().sketch.as_mut();
            if let Some(sketch) = sketch {
                if let Some(entity) = sketch.entities.iter_mut().find(|e| e.id == id) {
                    if let Some(attr) = entity.pad.as_mut() {
                        f(attr);
                        editor.dirty = true;
                        editor.canvas_cache.clear();
                    }
                }
            }
        }
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_designator(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.number = value);
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_side(
        &mut self,
        idx: usize,
        side: crate::library::editor::footprint::state::PadSide,
    ) -> bool {
        use crate::library::editor::footprint::state::PadSide;
        use signex_library::LayerId;
        let layers = match side {
            PadSide::Top => vec![
                LayerId::new("F.Cu"),
                LayerId::new("F.Mask"),
                LayerId::new("F.Paste"),
            ],
            PadSide::Bottom => vec![
                LayerId::new("B.Cu"),
                LayerId::new("B.Mask"),
                LayerId::new("B.Paste"),
            ],
            PadSide::All => vec![
                LayerId::new("*.Cu"),
                LayerId::new("F.Mask"),
                LayerId::new("B.Mask"),
            ],
        };
        self.with_selected_pad(idx, |pad| pad.layers = layers);
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_shape(
        &mut self,
        idx: usize,
        shape: signex_library::PadShape,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.shape = shape);
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_kind(
        &mut self,
        idx: usize,
        kind: signex_library::PadKind,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.kind = kind);
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_size_x(&mut self, idx: usize, value: String) -> bool {
        if let Ok(parsed) = value.trim().parse::<f64>() {
            if parsed > 0.0 {
                self.with_selected_pad(idx, |pad| pad.size_mm.0 = parsed);
            }
        }
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_size_y(&mut self, idx: usize, value: String) -> bool {
        if let Ok(parsed) = value.trim().parse::<f64>() {
            if parsed > 0.0 {
                self.with_selected_pad(idx, |pad| pad.size_mm.1 = parsed);
            }
        }
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_drill_diameter(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        let parsed = fp_parse_optional_mm(&value);
        self.with_selected_pad(idx, |pad| pad.drill_diameter_mm = parsed);
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_drill_slot_length(
        &mut self,
        idx: usize,
        _value: String,
    ) -> bool {
        // v0.20 placeholder — slot length not yet on EditorPad. Wired
        // when `EditorPad` gains a separate `drill_slot_length_mm`.
        let _ = idx;
        self.refresh_panel_ctx();
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_corner_radius_pct(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        let parsed = value
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|v| (0.0..=50.0).contains(v));
        self.with_selected_pad(idx, |pad| pad.stack.corner_radius_pct = parsed);
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_template(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.template = value);
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_template_library(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.template_library = value);
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_paste_margin_top(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        let parsed = fp_parse_optional_mm(&value);
        self.with_selected_pad(idx, |pad| pad.stack.paste_margin_top = parsed);
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_paste_margin_bottom(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        let parsed = fp_parse_optional_mm(&value);
        self.with_selected_pad(idx, |pad| pad.stack.paste_margin_bottom = parsed);
        true
    }
    pub(crate) fn fp_editor_toggle_selected_pad_paste_enabled_top(
        &mut self,
        idx: usize,
        on: bool,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.stack.paste_enabled_top = on);
        true
    }
    pub(crate) fn fp_editor_toggle_selected_pad_paste_enabled_bottom(
        &mut self,
        idx: usize,
        on: bool,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.stack.paste_enabled_bottom = on);
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_mask_margin_top(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        let parsed = fp_parse_optional_mm(&value);
        self.with_selected_pad(idx, |pad| pad.stack.mask_margin_top = parsed);
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_mask_margin_bottom(
        &mut self,
        idx: usize,
        value: String,
    ) -> bool {
        let parsed = fp_parse_optional_mm(&value);
        self.with_selected_pad(idx, |pad| pad.stack.mask_margin_bottom = parsed);
        true
    }
    pub(crate) fn fp_editor_toggle_selected_pad_mask_tented_top(
        &mut self,
        idx: usize,
        on: bool,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.stack.mask_tented_top = on);
        true
    }
    pub(crate) fn fp_editor_toggle_selected_pad_mask_tented_bottom(
        &mut self,
        idx: usize,
        on: bool,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.stack.mask_tented_bottom = on);
        true
    }
    pub(crate) fn fp_editor_toggle_selected_pad_thermal_relief(
        &mut self,
        idx: usize,
        on: bool,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.stack.thermal_relief = on);
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_feature_top(
        &mut self,
        idx: usize,
        value: signex_sketch::attr::PadFeature,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.feature_top = value);
        true
    }
    pub(crate) fn fp_editor_set_selected_pad_feature_bottom(
        &mut self,
        idx: usize,
        value: signex_sketch::attr::PadFeature,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.feature_bottom = value);
        true
    }
    pub(crate) fn fp_editor_toggle_selected_pad_testpoint_top_assembly(
        &mut self,
        idx: usize,
        on: bool,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.testpoint.top_assembly = on);
        true
    }
    pub(crate) fn fp_editor_toggle_selected_pad_testpoint_top_fab(
        &mut self,
        idx: usize,
        on: bool,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.testpoint.top_fab = on);
        true
    }
    pub(crate) fn fp_editor_toggle_selected_pad_testpoint_bottom_assembly(
        &mut self,
        idx: usize,
        on: bool,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.testpoint.bottom_assembly = on);
        true
    }
    pub(crate) fn fp_editor_toggle_selected_pad_testpoint_bottom_fab(
        &mut self,
        idx: usize,
        on: bool,
    ) -> bool {
        self.with_selected_pad(idx, |pad| pad.testpoint.bottom_fab = on);
        true
    }

    // v0.20/v0.21 — Pad Stack tab + placement-default Net / Locked /
    // Electrical Type / Hole detail / Plated toggles. Each mutates a
    // slice of `editor.state.next_pad_defaults` (or `pad_stack_tab`)
    // so the next placement click picks up the new value; the panel
    // refresh re-reads the form.
    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_pad_stack_tab(
        &mut self,
        tab: &crate::library::editor::footprint::state::PadStackTab,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.pad_stack_tab = *tab;
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_next_pad_electrical_type(
        &mut self,
        v: &signex_sketch::attr::ElectricalType,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.electrical_type = *v;
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_next_pad_net(&mut self, v: &str) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.net = v.to_string();
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_toggle_next_pad_locked(&mut self, on: &bool) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.locked = *on;
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_next_pad_hole_tolerance_plus(&mut self, v: &str) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.hole_tolerance_plus_mm = fp_parse_optional_mm(v);
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_next_pad_hole_tolerance_minus(&mut self, v: &str) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.hole_tolerance_minus_mm = fp_parse_optional_mm(v);
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_next_pad_hole_rotation(&mut self, v: &str) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.hole_rotation_deg = v.trim().parse::<f64>().ok();
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_next_pad_copper_offset_x(&mut self, v: &str) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.copper_offset_x_mm = fp_parse_optional_mm(v);
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_set_next_pad_copper_offset_y(&mut self, v: &str) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.copper_offset_y_mm = fp_parse_optional_mm(v);
        }
        self.refresh_panel_ctx();
        true
    }

    pub(in crate::app::handlers::dock::sch_library) fn handle_fp_editor_toggle_next_pad_plated(&mut self, plated: &bool) -> bool {
        use signex_library::PadKind as Pk;
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.kind = if *plated { Pk::Tht } else { Pk::NptHole };
        }
        self.refresh_panel_ctx();
        true
    }
}
