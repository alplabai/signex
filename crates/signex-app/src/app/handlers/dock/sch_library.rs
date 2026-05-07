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
    /// v0.18.8 — convenience: resolve the active tab's `.snxfpt`
    /// path, if any. The Footprint Library panel handlers below all
    /// need this; centralising it keeps the dispatch arms tight.
    fn active_footprint_editor_path(&self) -> Option<std::path::PathBuf> {
        let active_tab = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)?;
        active_tab.kind.as_footprint_editor().cloned()
    }

    pub(super) fn handle_dock_sch_library_message(
        &mut self,
        panel_msg: &crate::panels::PanelMsg,
    ) -> bool {
        match panel_msg {
            crate::panels::PanelMsg::FpLibraryOpenSibling(sibling_path) => {
                // v0.14.2 — open the sibling .snxfpt as a new tab
                // (or activate an existing tab if it's already open)
                // via the existing primitive-open flow.
                let _ = self.handle_open_primitive(sibling_path.clone());
                self.refresh_panel_ctx();
                true
            }
            // v0.18.8 — Footprint Library panel internal-row select.
            // Stores `panel_selected_idx` on the active footprint
            // editor so the row tints + button row gates correctly.
            // Independent of `active_idx`: only the Edit button (or
            // a double-click hook later) promotes selection to active.
            crate::panels::PanelMsg::FpLibrarySelectInternal(idx) => {
                if let Some(path) = self.active_footprint_editor_path() {
                    if let Some(editor) = self.document_state.footprint_editors.get_mut(&path) {
                        let last = editor.file.footprints.len().saturating_sub(1);
                        editor.panel_selected_idx = Some((*idx).min(last));
                    }
                    self.refresh_panel_ctx();
                }
                true
            }
            // v0.18.8 — `+ Add` button. Routes through the existing
            // `FootprintAddNewSibling` dispatcher which appends an
            // empty Footprint and switches `active_idx` onto it.
            crate::panels::PanelMsg::FpLibraryAddInternal => {
                if let Some(path) = self.active_footprint_editor_path() {
                    let _ = self.update(Message::Library(
                        crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                            path: path.clone(),
                            msg: crate::library::messages::PrimitiveEditorMsg::FootprintAddNewSibling,
                        },
                    ));
                    if let Some(editor) = self.document_state.footprint_editors.get_mut(&path) {
                        // Mirror the panel selection onto the just-
                        // added sibling so Delete/Edit immediately
                        // operate on it.
                        editor.panel_selected_idx = Some(editor.active_idx);
                    }
                    self.refresh_panel_ctx();
                }
                true
            }
            // v0.18.8 — `Delete` button. Removes the selected
            // footprint from the envelope. Refuses to remove the
            // last remaining footprint (an empty FootprintFile would
            // fail to load on next open).
            crate::panels::PanelMsg::FpLibraryDeleteInternal(idx) => {
                if let Some(path) = self.active_footprint_editor_path() {
                    if let Some(editor) = self.document_state.footprint_editors.get_mut(&path) {
                        let last = editor.file.footprints.len();
                        if last > 1 && *idx < last {
                            editor.file.footprints.remove(*idx);
                            // Clamp `active_idx` so the canvas keeps
                            // pointing at a valid sibling.
                            if editor.active_idx >= editor.file.footprints.len() {
                                editor.active_idx = editor.file.footprints.len().saturating_sub(1);
                            }
                            // Re-derive canvas-side state from the
                            // (possibly different) active primitive.
                            editor.state =
                                crate::library::editor::footprint::state::FootprintEditorState::from_footprint(
                                    editor.primitive(),
                                );
                            editor.panel_selected_idx = None;
                            editor.canvas_cache.clear();
                            editor.dirty = true;
                            self.document_state.dirty_paths.insert(path.clone());
                        } else if last == 1 {
                            tracing::warn!(
                                target: "signex::library",
                                path = %path.display(),
                                "Footprint Library: refused to delete the last footprint in the envelope",
                            );
                        }
                    }
                    self.refresh_panel_ctx();
                }
                true
            }
            // v0.18.8 — `Edit` button. Promotes the panel selection
            // to `active_idx` via the existing
            // `FootprintSelectActiveIdx` dispatcher.
            crate::panels::PanelMsg::FpLibraryEditInternal(idx) => {
                if let Some(path) = self.active_footprint_editor_path() {
                    let _ = self.update(Message::Library(
                        crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                            path,
                            msg: crate::library::messages::PrimitiveEditorMsg::FootprintSelectActiveIdx(*idx),
                        },
                    ));
                    self.refresh_panel_ctx();
                }
                true
            }
            // v0.18.8 — `Place` button. PCB Component placement is
            // not wired yet; log a warn so the action is observable
            // without a visible no-op.
            crate::panels::PanelMsg::FpLibraryPlaceInternal(idx) => {
                tracing::warn!(
                    target: "signex::library",
                    idx = idx,
                    "Footprint Library: Place is not yet wired (PCB integration pending)",
                );
                true
            }
            crate::panels::PanelMsg::FpEditorToggleAutoFitCourtyard => {
                // v0.14.2 — resolve the active footprint editor's
                // path and route through the existing
                // `FootprintToggleAutoFit` dispatch so the toggle
                // shares its dirty / panel-refresh behaviour with
                // the active-bar button.
                if let Some(active_tab) =
                    self.document_state.tabs.get(self.document_state.active_tab)
                {
                    if let Some(path) = active_tab.kind.as_footprint_editor() {
                        let path = path.clone();
                        let _ = self.update(Message::Library(
                            crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                                path,
                                msg: crate::library::messages::PrimitiveEditorMsg::FootprintToggleAutoFit,
                            },
                        ));
                        // v0.16.x — rebuild the panel context so the
                        // pill's pressed-state style reflects the new
                        // `auto_fit_courtyard` bool. Without this the
                        // button click looked like a no-op because
                        // `PanelContext.footprint_editor.auto_fit_courtyard`
                        // was stale until the next unrelated panel
                        // refresh.
                        self.refresh_panel_ctx();
                    }
                }
                true
            }
            crate::panels::PanelMsg::FpEditorSetRole { id, role } => {
                // v0.16.2 — Properties-panel Role pick_list. Resolve
                // the active footprint editor tab and forward through
                // the standard PrimitiveEditorEvent path so the role
                // mutation goes through `apply_sketch_role_with_warnings`
                // (clears all attrs, sets matching one, runs solve+bake).
                if let Some(active_tab) =
                    self.document_state.tabs.get(self.document_state.active_tab)
                {
                    if let Some(path) = active_tab.kind.as_footprint_editor() {
                        let path = path.clone();
                        let _ = self.update(Message::Library(
                            crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                                path,
                                msg: crate::library::messages::PrimitiveEditorMsg::FootprintSketchSetRole {
                                    id: *id,
                                    role: *role,
                                },
                            },
                        ));
                        self.refresh_panel_ctx();
                    }
                }
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadDesignator(value) => {
                self.fp_editor_set_next_pad_designator(value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadSizeX(value) => {
                self.fp_editor_set_next_pad_size_x(value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadSizeY(value) => {
                self.fp_editor_set_next_pad_size_y(value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadSide(side) => {
                self.fp_editor_set_next_pad_side(*side);
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadRotation(value) => {
                self.fp_editor_set_next_pad_rotation(value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadRotation { idx, value } => {
                self.fp_editor_set_selected_pad_rotation(*idx, value.clone());
                true
            }
            // v0.20 — Altium-parity Pad Properties / Pad Stack / Pad
            // Features form for the next placed pad. Each handler
            // mutates the matching slice of `editor.state.next_pad_defaults`
            // (and its `stack` sub-struct) so the next placement
            // click picks up the new value.
            crate::panels::PanelMsg::FpEditorSetNextPadShape(shape) => {
                self.fp_editor_set_next_pad_shape(shape.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadKind(kind) => {
                self.fp_editor_set_next_pad_kind(*kind);
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadDrillDiameter(v) => {
                self.fp_editor_set_next_pad_drill_diameter(v.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadDrillSlotLength(v) => {
                self.fp_editor_set_next_pad_drill_slot_length(v.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadCornerRadiusPct(v) => {
                self.fp_editor_set_next_pad_corner_radius_pct(v.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadTemplate(v) => {
                self.fp_editor_set_next_pad_template(v.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadTemplateLibrary(v) => {
                self.fp_editor_set_next_pad_template_library(v.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadPasteMarginTop(v) => {
                self.fp_editor_set_next_pad_paste_margin_top(v.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadPasteMarginBottom(v) => {
                self.fp_editor_set_next_pad_paste_margin_bottom(v.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorToggleNextPadPasteEnabledTop(on) => {
                self.fp_editor_toggle_next_pad_paste_enabled_top(*on);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleNextPadPasteEnabledBottom(on) => {
                self.fp_editor_toggle_next_pad_paste_enabled_bottom(*on);
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadMaskMarginTop(v) => {
                self.fp_editor_set_next_pad_mask_margin_top(v.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadMaskMarginBottom(v) => {
                self.fp_editor_set_next_pad_mask_margin_bottom(v.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorToggleNextPadMaskTentedTop(on) => {
                self.fp_editor_toggle_next_pad_mask_tented_top(*on);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleNextPadMaskTentedBottom(on) => {
                self.fp_editor_toggle_next_pad_mask_tented_bottom(*on);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleNextPadThermalRelief(on) => {
                self.fp_editor_toggle_next_pad_thermal_relief(*on);
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadFeatureTop(f) => {
                self.fp_editor_set_next_pad_feature_top(*f);
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadFeatureBottom(f) => {
                self.fp_editor_set_next_pad_feature_bottom(*f);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleNextPadTestpointTopAssembly(on) => {
                self.fp_editor_toggle_next_pad_testpoint_top_assembly(*on);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleNextPadTestpointTopFab(on) => {
                self.fp_editor_toggle_next_pad_testpoint_top_fab(*on);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleNextPadTestpointBottomAssembly(on) => {
                self.fp_editor_toggle_next_pad_testpoint_bottom_assembly(*on);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleNextPadTestpointBottomFab(on) => {
                self.fp_editor_toggle_next_pad_testpoint_bottom_fab(*on);
                true
            }
            // v0.20 — selected-pad editing routes. Each handler mutates
            // `state.pads[idx]` and triggers a dirty + canvas-cache
            // invalidate so the panel re-renders with the new value.
            crate::panels::PanelMsg::FpEditorSetSelectedPadDesignator { idx, value } => {
                self.fp_editor_set_selected_pad_designator(*idx, value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadSide { idx, side } => {
                self.fp_editor_set_selected_pad_side(*idx, *side);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadShape { idx, shape } => {
                self.fp_editor_set_selected_pad_shape(*idx, shape.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadKind { idx, kind } => {
                self.fp_editor_set_selected_pad_kind(*idx, *kind);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadSizeX { idx, value } => {
                self.fp_editor_set_selected_pad_size_x(*idx, value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadSizeY { idx, value } => {
                self.fp_editor_set_selected_pad_size_y(*idx, value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadDrillDiameter { idx, value } => {
                self.fp_editor_set_selected_pad_drill_diameter(*idx, value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadDrillSlotLength { idx, value } => {
                self.fp_editor_set_selected_pad_drill_slot_length(*idx, value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadCornerRadiusPct { idx, value } => {
                self.fp_editor_set_selected_pad_corner_radius_pct(*idx, value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadTemplate { idx, value } => {
                self.fp_editor_set_selected_pad_template(*idx, value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadTemplateLibrary { idx, value } => {
                self.fp_editor_set_selected_pad_template_library(*idx, value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadPasteMarginTop { idx, value } => {
                self.fp_editor_set_selected_pad_paste_margin_top(*idx, value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadPasteMarginBottom { idx, value } => {
                self.fp_editor_set_selected_pad_paste_margin_bottom(*idx, value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSelectedPadPasteEnabledTop { idx, value } => {
                self.fp_editor_toggle_selected_pad_paste_enabled_top(*idx, *value);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSelectedPadPasteEnabledBottom { idx, value } => {
                self.fp_editor_toggle_selected_pad_paste_enabled_bottom(*idx, *value);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadMaskMarginTop { idx, value } => {
                self.fp_editor_set_selected_pad_mask_margin_top(*idx, value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadMaskMarginBottom { idx, value } => {
                self.fp_editor_set_selected_pad_mask_margin_bottom(*idx, value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSelectedPadMaskTentedTop { idx, value } => {
                self.fp_editor_toggle_selected_pad_mask_tented_top(*idx, *value);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSelectedPadMaskTentedBottom { idx, value } => {
                self.fp_editor_toggle_selected_pad_mask_tented_bottom(*idx, *value);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSelectedPadThermalRelief { idx, value } => {
                self.fp_editor_toggle_selected_pad_thermal_relief(*idx, *value);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadFeatureTop { idx, value } => {
                self.fp_editor_set_selected_pad_feature_top(*idx, *value);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadFeatureBottom { idx, value } => {
                self.fp_editor_set_selected_pad_feature_bottom(*idx, *value);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSelectedPadTestpointTopAssembly { idx, value } => {
                self.fp_editor_toggle_selected_pad_testpoint_top_assembly(*idx, *value);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSelectedPadTestpointTopFab { idx, value } => {
                self.fp_editor_toggle_selected_pad_testpoint_top_fab(*idx, *value);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSelectedPadTestpointBottomAssembly { idx, value } => {
                self.fp_editor_toggle_selected_pad_testpoint_bottom_assembly(*idx, *value);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSelectedPadTestpointBottomFab { idx, value } => {
                self.fp_editor_toggle_selected_pad_testpoint_bottom_fab(*idx, *value);
                true
            }
            crate::panels::PanelMsg::FpEditorSetPadStackTab(tab) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.state.pad_stack_tab = *tab;
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadElectricalType(v) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.state.next_pad_defaults.electrical_type = *v;
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadNet(v) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.state.next_pad_defaults.net = v.clone();
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorToggleNextPadLocked(on) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.state.next_pad_defaults.locked = *on;
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadElectricalType { idx, value } => {
                let v = *value;
                self.with_selected_pad(*idx, |pad| pad.electrical_type = v);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadNet { idx, value } => {
                let v = value.clone();
                self.with_selected_pad(*idx, |pad| pad.net = v);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSelectedPadLocked { idx, value } => {
                let v = *value;
                self.with_selected_pad(*idx, |pad| pad.locked = v);
                true
            }
            crate::panels::PanelMsg::FpEditorSetFootprintDescription(v) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.primitive_mut().description = v.clone();
                    editor.dirty = true;
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorSetFootprintDefaultDesignator(v) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.primitive_mut().default_designator = v.clone();
                    editor.dirty = true;
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorSetFootprintComponentType(t) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.primitive_mut().component_type = *t;
                    editor.dirty = true;
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorSetFootprintHeight(v) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.primitive_mut().height_mm = fp_parse_optional_mm(v);
                    editor.dirty = true;
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorSetSilkLineFromX(v) => {
                self.fp_editor_set_silk_line_endpoint(SilkLineEndpoint::FromX, v.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetSilkLineFromY(v) => {
                self.fp_editor_set_silk_line_endpoint(SilkLineEndpoint::FromY, v.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetSilkLineToX(v) => {
                self.fp_editor_set_silk_line_endpoint(SilkLineEndpoint::ToX, v.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetSilkLineToY(v) => {
                self.fp_editor_set_silk_line_endpoint(SilkLineEndpoint::ToY, v.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetSilkTextPositionX(v) => {
                self.fp_editor_set_silk_text_field(SilkTextField::PositionX, v.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetSilkTextPositionY(v) => {
                self.fp_editor_set_silk_text_field(SilkTextField::PositionY, v.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetSilkTextSize(v) => {
                self.fp_editor_set_silk_text_field(SilkTextField::Size, v.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetSilkStrokeWidth(v) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    let parsed = v.trim().parse::<f64>().ok();
                    if let Some(idx) = editor.state.selected_silk_f {
                        if let Some(g) = editor.primitive_mut().silk_f.get_mut(idx) {
                            if let Some(w) = parsed {
                                if w >= 0.0 {
                                    g.stroke_width = w;
                                    editor.dirty = true;
                                    editor.canvas_cache.clear();
                                }
                            }
                        }
                    }
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadHoleTolerancePlus(v) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.state.next_pad_defaults.hole_tolerance_plus_mm =
                        fp_parse_optional_mm(v);
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadHoleToleranceMinus(v) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.state.next_pad_defaults.hole_tolerance_minus_mm =
                        fp_parse_optional_mm(v);
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadHoleRotation(v) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.state.next_pad_defaults.hole_rotation_deg =
                        v.trim().parse::<f64>().ok();
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadCopperOffsetX(v) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.state.next_pad_defaults.copper_offset_x_mm =
                        fp_parse_optional_mm(v);
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadCopperOffsetY(v) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.state.next_pad_defaults.copper_offset_y_mm =
                        fp_parse_optional_mm(v);
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorToggleNextPadPlated(plated) => {
                use signex_library::PadKind as Pk;
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.state.next_pad_defaults.kind = if *plated { Pk::Tht } else { Pk::NptHole };
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadHoleTolerancePlus { idx, value } => {
                let v = fp_parse_optional_mm(value);
                self.with_selected_pad(*idx, |pad| pad.hole_tolerance_plus_mm = v);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadHoleToleranceMinus { idx, value } => {
                let v = fp_parse_optional_mm(value);
                self.with_selected_pad(*idx, |pad| pad.hole_tolerance_minus_mm = v);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadHoleRotation { idx, value } => {
                let v = value.trim().parse::<f64>().ok();
                self.with_selected_pad(*idx, |pad| pad.hole_rotation_deg = v);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadCopperOffsetX { idx, value } => {
                let v = fp_parse_optional_mm(value);
                self.with_selected_pad(*idx, |pad| pad.copper_offset_x_mm = v);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSelectedPadCopperOffsetY { idx, value } => {
                let v = fp_parse_optional_mm(value);
                self.with_selected_pad(*idx, |pad| pad.copper_offset_y_mm = v);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSelectedPadPlated { idx, value } => {
                use signex_library::PadKind as Pk;
                let plated = *value;
                self.with_selected_pad(*idx, |pad| {
                    pad.kind = if plated { Pk::Tht } else { Pk::NptHole };
                });
                true
            }
            // v0.21 — sketch-pad attribute mutations. Each routes
            // through `with_selected_sketch_pad` which mutates the
            // entity's `PadAttr` then triggers solve+bake so geometry
            // re-derives if the change affected dependent expressions.
            crate::panels::PanelMsg::FpEditorSetSketchPadElectricalType { id, value } => {
                let v = *value;
                self.with_selected_sketch_pad(*id, |attr| attr.electrical_type = v);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSketchPadNet { id, value } => {
                let v = value.clone();
                self.with_selected_sketch_pad(*id, |attr| attr.net = v);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSketchPadLocked { id, value } => {
                let v = *value;
                self.with_selected_sketch_pad(*id, |attr| attr.locked = v);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSketchPadTemplate { id, value } => {
                let v = value.clone();
                self.with_selected_sketch_pad(*id, |attr| attr.template = v);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSketchPadTemplateLibrary { id, value } => {
                let v = value.clone();
                self.with_selected_sketch_pad(*id, |attr| attr.library = v);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSketchPadFeatureTop { id, value } => {
                let v = *value;
                self.with_selected_sketch_pad(*id, |attr| attr.feature_top = v);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSketchPadFeatureBottom { id, value } => {
                let v = *value;
                self.with_selected_sketch_pad(*id, |attr| attr.feature_bottom = v);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSketchPadTestpointTopAssembly { id, value } => {
                let v = *value;
                self.with_selected_sketch_pad(*id, |attr| attr.testpoint.top_assembly = v);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSketchPadTestpointTopFab { id, value } => {
                let v = *value;
                self.with_selected_sketch_pad(*id, |attr| attr.testpoint.top_fab = v);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSketchPadTestpointBottomAssembly { id, value } => {
                let v = *value;
                self.with_selected_sketch_pad(*id, |attr| attr.testpoint.bottom_assembly = v);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSketchPadTestpointBottomFab { id, value } => {
                let v = *value;
                self.with_selected_sketch_pad(*id, |attr| attr.testpoint.bottom_fab = v);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSketchPadThermalRelief { id, value } => {
                let v = *value;
                self.with_selected_sketch_pad(*id, |attr| attr.stack.thermal_relief = v);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSketchPadMaskTentedTop { id, value } => {
                let v = *value;
                self.with_selected_sketch_pad(*id, |attr| attr.stack.mask_top_tented = v);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSketchPadMaskTentedBottom { id, value } => {
                let v = *value;
                self.with_selected_sketch_pad(*id, |attr| attr.stack.mask_bottom_tented = v);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSketchPadPasteEnabledTop { id, value } => {
                let v = *value;
                self.with_selected_sketch_pad(*id, |attr| attr.stack.paste_top_enabled = v);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSketchPadPasteEnabledBottom { id, value } => {
                let v = *value;
                self.with_selected_sketch_pad(*id, |attr| attr.stack.paste_bottom_enabled = v);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSketchPadHoleTolerancePlus { id, value } => {
                let v = fp_parse_optional_mm(value);
                self.with_selected_sketch_pad(*id, |attr| attr.hole_tolerance_plus_mm = v);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSketchPadHoleToleranceMinus { id, value } => {
                let v = fp_parse_optional_mm(value);
                self.with_selected_sketch_pad(*id, |attr| attr.hole_tolerance_minus_mm = v);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSketchPadHoleRotation { id, value } => {
                let v = value.trim().parse::<f64>().ok();
                self.with_selected_sketch_pad(*id, |attr| attr.hole_rotation_deg = v);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSketchPadCopperOffsetX { id, value } => {
                let v = fp_parse_optional_mm(value);
                self.with_selected_sketch_pad(*id, |attr| attr.copper_offset_x_mm = v);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSketchPadCopperOffsetY { id, value } => {
                let v = fp_parse_optional_mm(value);
                self.with_selected_sketch_pad(*id, |attr| attr.copper_offset_y_mm = v);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSketchPadCornerRadiusPct { id, value } => {
                let v = value
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|v| (0.0..=50.0).contains(v));
                self.with_selected_sketch_pad(*id, |attr| attr.stack.corner_radius_pct = v);
                true
            }
            crate::panels::PanelMsg::FpEditorEditPadInSketch { pad_idx } => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    let entity_id = editor
                        .state
                        .pads
                        .get(*pad_idx)
                        .and_then(|p| p.sketch_entity_id);
                    editor.state.mode =
                        crate::library::editor::footprint::state::EditorMode::Sketch;
                    editor.state.selected_pad = None;
                    editor.state.selected_sketch = entity_id;
                    editor.canvas_cache.clear();
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorEditSketchPadInPads { id } => {
                // v0.22 Phase D6 — mirror of FpEditorEditPadInSketch:
                // resolve the EditorPad whose `sketch_entity_id` ==
                // `id`, switch to Pads mode, and select that pad.
                if let Some(editor) = self.active_footprint_editor_mut() {
                    let pad_idx = editor
                        .state
                        .pads
                        .iter()
                        .position(|p| p.sketch_entity_id == Some(*id));
                    editor.state.mode =
                        crate::library::editor::footprint::state::EditorMode::Normal;
                    editor.state.selected_sketch = None;
                    editor.state.selected_sketch_secondary = None;
                    editor.state.selected_pad = pad_idx;
                    editor.canvas_cache.clear();
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSilkFilled(on) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    if let Some(idx) = editor.state.selected_silk_f {
                        if let Some(g) = editor.primitive_mut().silk_f.get_mut(idx) {
                            g.filled = *on;
                            editor.dirty = true;
                            editor.canvas_cache.clear();
                        }
                    }
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorSetPourNet { id, value } => {
                self.fp_editor_set_pour_net(*id, value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetPourFillType { id, value } => {
                self.fp_editor_set_pour_fill_type(*id, *value);
                true
            }
            crate::panels::PanelMsg::FpEditorSetPourPriority { id, value } => {
                self.fp_editor_set_pour_priority(*id, value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetKeepoutKind { id, kind, value } => {
                self.fp_editor_set_keepout_kind(*id, *kind, *value);
                true
            }
            crate::panels::PanelMsg::FpEditorSetCutoutEdgeRadius { id, value } => {
                self.fp_editor_set_cutout_edge_radius(*id, value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetCutoutThrough { id, value } => {
                self.fp_editor_set_cutout_through(*id, *value);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSnapOption(flag) => {
                self.fp_editor_toggle_snap_option(*flag);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSnapGridStep(value) => {
                self.fp_editor_set_snap_grid_step(value);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSnapDistance(value) => {
                self.handle_fp_set_snap_distance(value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetAxisSnapRange(value) => {
                self.handle_fp_set_axis_snap_range(value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetFootprintName(name) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.primitive_mut().name = name.clone();
                    editor.dirty = true;
                    editor.canvas_cache.clear();
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSelectionFilter(kind) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.state.selection_filter.toggle(*kind);
                    editor.canvas_cache.clear();
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorOpenSelectionFilterCustom => {
                let _ = self.update(Message::OpenSelectionFilterCustom);
                true
            }
            crate::panels::PanelMsg::FpEditorSetSnapSubTab(tab) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.state.snap_subtab = *tab;
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorSetSnappingMode(mode) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.state.snapping_mode = *mode;
                    editor.canvas_cache.clear();
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorGridManagerAdd => {
                // v0.18.21 — append a fresh `GridDef` clone of the
                // active row (so the new grid inherits the user's last
                // step + display picks). The new row activates so the
                // user can immediately retune via Ctrl+G.
                //
                // v0.18.25.1 — fall back to the live `snap_options`
                // (not `GridDef::default()`) when `active_grid_idx`
                // is out of range, so a misindex doesn't drop the
                // user's current step/display pickers on the floor.
                if let Some(editor) = self.active_footprint_editor_mut() {
                    let seed = editor
                        .state
                        .grids
                        .get(editor.state.active_grid_idx)
                        .cloned()
                        .unwrap_or_else(|| {
                            crate::library::editor::footprint::state::GridDef::from_snap_options(
                                &editor.state.snap_options,
                            )
                        });
                    let mut next = seed;
                    next.name = format!("Grid {}", editor.state.grids.len() + 1);
                    editor.state.grids.push(next);
                    let new_idx = editor.state.grids.len() - 1;
                    editor.state.active_grid_idx = new_idx;
                    // Mirror onto SnapOptions so the canvas picks up
                    // the new active row immediately.
                    let row = &editor.state.grids[new_idx];
                    editor.state.snap_options.grid_step_mm = row.step_mm;
                    editor.state.snap_options.fine_grid_display = row.fine_display;
                    editor.state.snap_options.coarse_grid_display = row.coarse_display;
                    editor.state.snap_options.coarse_multiplier = row.coarse_multiplier;
                    editor.canvas_cache.clear();
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorGridManagerProperties => {
                // Reuses the Ctrl+G modal so the user can edit the
                // active grid via the same dialog. The modal open
                // handler reads `snap_options.grid_step_mm` and seeds
                // the buffers; the commit path mirrors back to
                // `grids[active_grid_idx]` (see GridPropertiesCommit).
                let _ = self.update(Message::GridPropertiesOpen);
                true
            }
            crate::panels::PanelMsg::FpEditorGridManagerDelete => {
                // v0.18.21 — remove the active row. Always keep at
                // least one grid (UI gates the button when only one
                // remains, so this branch should normally only fire
                // when len > 1).
                if let Some(editor) = self.active_footprint_editor_mut() {
                    if editor.state.grids.len() > 1 {
                        let idx = editor.state.active_grid_idx;
                        editor.state.grids.remove(idx);
                        if editor.state.active_grid_idx >= editor.state.grids.len() {
                            editor.state.active_grid_idx = editor.state.grids.len() - 1;
                        }
                        // Mirror new active onto SnapOptions.
                        let row = &editor.state.grids[editor.state.active_grid_idx];
                        editor.state.snap_options.grid_step_mm = row.step_mm;
                        editor.state.snap_options.fine_grid_display = row.fine_display;
                        editor.state.snap_options.coarse_grid_display = row.coarse_display;
                        editor.state.snap_options.coarse_multiplier = row.coarse_multiplier;
                        editor.canvas_cache.clear();
                    }
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorGridSetActive(idx) => {
                let idx = *idx;
                if let Some(editor) = self.active_footprint_editor_mut() {
                    if idx < editor.state.grids.len() {
                        editor.state.active_grid_idx = idx;
                        let row = &editor.state.grids[idx];
                        editor.state.snap_options.grid_step_mm = row.step_mm;
                        editor.state.snap_options.fine_grid_display = row.fine_display;
                        editor.state.snap_options.coarse_grid_display = row.coarse_display;
                        editor.state.snap_options.coarse_multiplier = row.coarse_multiplier;
                        editor.canvas_cache.clear();
                    }
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorGuideManagerAdd => {
                // v0.18.20 — bare "Add" button defaults to a vertical
                // guide at world X = 0; users can flip via the row's
                // axis label and edit the position field afterwards.
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor
                        .state
                        .guides
                        .push(crate::library::editor::footprint::state::Guide {
                            axis: crate::library::editor::footprint::state::GuideAxis::Vertical,
                            position_mm: 0.0,
                            enabled: true,
                        });
                    editor.canvas_cache.clear();
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorGuideAddVertical => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor
                        .state
                        .guides
                        .push(crate::library::editor::footprint::state::Guide {
                            axis: crate::library::editor::footprint::state::GuideAxis::Vertical,
                            position_mm: 0.0,
                            enabled: true,
                        });
                    editor.canvas_cache.clear();
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorGuideAddHorizontal => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor
                        .state
                        .guides
                        .push(crate::library::editor::footprint::state::Guide {
                            axis: crate::library::editor::footprint::state::GuideAxis::Horizontal,
                            position_mm: 0.0,
                            enabled: true,
                        });
                    editor.canvas_cache.clear();
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorGuideDelete(idx) => {
                let idx = *idx;
                if let Some(editor) = self.active_footprint_editor_mut() {
                    if idx < editor.state.guides.len() {
                        editor.state.guides.remove(idx);
                        editor.canvas_cache.clear();
                    }
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorGuideToggle(idx) => {
                let idx = *idx;
                if let Some(editor) = self.active_footprint_editor_mut() {
                    if let Some(g) = editor.state.guides.get_mut(idx) {
                        g.enabled = !g.enabled;
                        editor.canvas_cache.clear();
                    }
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorGuideSetPosition(idx, raw) => {
                let idx = *idx;
                if let Ok(parsed) = raw.trim().parse::<f64>() {
                    if let Some(editor) = self.active_footprint_editor_mut() {
                        if let Some(g) = editor.state.guides.get_mut(idx) {
                            g.position_mm = parsed;
                            editor.canvas_cache.clear();
                        }
                    }
                    self.refresh_panel_ctx();
                }
                // Invalid float (e.g. user typing "-") — silently drop
                // so the input keeps capturing keystrokes.
                true
            }
            crate::panels::PanelMsg::FpEditorSetSilkText(value) => {
                // v0.18.24 — edit the selected silk-front graphic's
                // Text content. No-op when the selection isn't a
                // Text or no silk graphic is selected.
                let value = value.clone();
                if let Some(editor) = self.active_footprint_editor_mut() {
                    if let Some(idx) = editor.state.selected_silk_f {
                        editor.with_parts(|_state, primitive| {
                            use signex_library::primitive::footprint::FpGraphicKind;
                            if let Some(g) = primitive.silk_f.get_mut(idx) {
                                if let FpGraphicKind::Text { content, .. } = &mut g.kind {
                                    *content = value;
                                }
                            }
                        });
                        editor.dirty = true;
                        editor.canvas_cache.clear();
                    }
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorDeleteSelectedSilk => {
                // v0.18.24 — delete the currently-selected silk-front
                // graphic and clear the selection.
                if let Some(editor) = self.active_footprint_editor_mut() {
                    if let Some(idx) = editor.state.selected_silk_f {
                        editor.with_parts(|state, primitive| {
                            if idx < primitive.silk_f.len() {
                                primitive.silk_f.remove(idx);
                                // HI-25: keep the selection cursor consistent
                                // with the new vec length / shifted indices.
                                state.selected_silk_f =
                                    crate::library::editor::footprint::state::adjust_selection_after_remove(
                                        state.selected_silk_f,
                                        idx,
                                    );
                            } else {
                                state.selected_silk_f = None;
                            }
                        });
                        editor.dirty = true;
                        editor.canvas_cache.clear();
                    }
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorEditParameter { name, expr } => {
                // v0.16.2 — Properties-panel parameter row edit.
                // Forwards to `FootprintSketchEditParameter` which
                // upserts the parameter and triggers a solve+bake.
                if let Some(active_tab) =
                    self.document_state.tabs.get(self.document_state.active_tab)
                {
                    if let Some(path) = active_tab.kind.as_footprint_editor() {
                        let path = path.clone();
                        let _ = self.update(Message::Library(
                            crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                                path,
                                msg: crate::library::messages::PrimitiveEditorMsg::FootprintSketchEditParameter {
                                    name: name.clone(),
                                    expr: expr.clone(),
                                },
                            },
                        ));
                        self.refresh_panel_ctx();
                    }
                }
                true
            }
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
        value: signex_library::PinDirection,
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

    pub(crate) fn fp_editor_set_next_pad_designator(&mut self, value: String) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.designator_override =
                if value.is_empty() { None } else { Some(value) };
        }
        self.refresh_panel_ctx();
    }

    pub(crate) fn fp_editor_set_next_pad_size_x(&mut self, value: String) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Ok(parsed) = value.trim().parse::<f64>() {
                if parsed > 0.0 {
                    editor.state.next_pad_defaults.size_x_mm = parsed;
                }
            }
        }
        self.refresh_panel_ctx();
    }

    pub(crate) fn fp_editor_set_next_pad_size_y(&mut self, value: String) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Ok(parsed) = value.trim().parse::<f64>() {
                if parsed > 0.0 {
                    editor.state.next_pad_defaults.size_y_mm = parsed;
                }
            }
        }
        self.refresh_panel_ctx();
    }

    pub(crate) fn fp_editor_set_next_pad_side(
        &mut self,
        side: crate::library::editor::footprint::state::PadSide,
    ) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.side = side;
        }
        self.refresh_panel_ctx();
    }

    pub(crate) fn fp_editor_set_next_pad_rotation(&mut self, value: String) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Ok(parsed) = value.trim().parse::<f64>() {
                editor.state.next_pad_defaults.rotation_deg = parsed;
            }
        }
        self.refresh_panel_ctx();
    }

    pub(crate) fn fp_editor_set_selected_pad_rotation(&mut self, idx: usize, value: String) {
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
    }

    // v0.20 — Altium-parity Pad Properties / Pad Stack / Pad Features
    // form handlers. Each method mutates a slice of
    // `editor.state.next_pad_defaults` so the next `add_pad_at` mints
    // a pad with the user-selected stack / feature / testpoint
    // configuration. None of these are dirty-marking on their own —
    // they're "pre-placement defaults" — but the panel `refresh` runs
    // so the form re-reads the new value.
    pub(crate) fn fp_editor_set_next_pad_shape(
        &mut self,
        shape: signex_library::PadShape,
    ) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.shape = shape;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_set_next_pad_kind(
        &mut self,
        kind: signex_library::PadKind,
    ) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.kind = kind;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_set_next_pad_drill_diameter(&mut self, value: String) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.drill_diameter_mm = fp_parse_optional_mm(&value);
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_set_next_pad_drill_slot_length(&mut self, value: String) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.drill_slot_length_mm =
                fp_parse_optional_mm(&value);
        }
        self.refresh_panel_ctx();
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
    pub(crate) fn fp_editor_set_next_pad_corner_radius_pct(&mut self, value: String) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            let parsed = value.trim().parse::<f64>().ok();
            editor.state.next_pad_defaults.stack.corner_radius_pct =
                parsed.filter(|v| (0.0..=50.0).contains(v));
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_set_next_pad_template(&mut self, value: String) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.template = value;
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_set_next_pad_template_library(&mut self, value: String) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.template_library = value;
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_set_next_pad_paste_margin_top(&mut self, value: String) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.paste_margin_top =
                fp_parse_optional_mm(&value);
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_set_next_pad_paste_margin_bottom(&mut self, value: String) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.paste_margin_bottom =
                fp_parse_optional_mm(&value);
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_toggle_next_pad_paste_enabled_top(&mut self, on: bool) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.paste_enabled_top = on;
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_toggle_next_pad_paste_enabled_bottom(&mut self, on: bool) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.paste_enabled_bottom = on;
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_set_next_pad_mask_margin_top(&mut self, value: String) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.mask_margin_top =
                fp_parse_optional_mm(&value);
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_set_next_pad_mask_margin_bottom(&mut self, value: String) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.mask_margin_bottom =
                fp_parse_optional_mm(&value);
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_toggle_next_pad_mask_tented_top(&mut self, on: bool) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.mask_tented_top = on;
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_toggle_next_pad_mask_tented_bottom(&mut self, on: bool) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.mask_tented_bottom = on;
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_toggle_next_pad_thermal_relief(&mut self, on: bool) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.stack.thermal_relief = on;
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_set_next_pad_feature_top(
        &mut self,
        f: signex_sketch::attr::PadFeature,
    ) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.feature_top = f;
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_set_next_pad_feature_bottom(
        &mut self,
        f: signex_sketch::attr::PadFeature,
    ) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.feature_bottom = f;
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_toggle_next_pad_testpoint_top_assembly(&mut self, on: bool) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.testpoint.top_assembly = on;
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_toggle_next_pad_testpoint_top_fab(&mut self, on: bool) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.testpoint.top_fab = on;
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_toggle_next_pad_testpoint_bottom_assembly(&mut self, on: bool) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.testpoint.bottom_assembly = on;
        }
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_toggle_next_pad_testpoint_bottom_fab(&mut self, on: bool) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.next_pad_defaults.testpoint.bottom_fab = on;
        }
        self.refresh_panel_ctx();
    }

    // v0.20 — Selected-pad editing handlers. Each one mutates a slice
    // of `state.pads[idx]` and dirty-marks the editor + clears the
    // canvas cache so the new value renders. The `with_parts` block
    // syncs the pad list back onto the underlying primitive so the
    // saved file picks up the change.
    fn with_selected_pad<F>(&mut self, idx: usize, f: F)
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
    }

    /// v0.21 — sketch-mode counterpart of `with_selected_pad`. Looks
    /// up the sketch entity by id, runs the closure on its `PadAttr`
    /// (creating one only if it already exists; non-pad entities are
    /// silently skipped), then dirty-marks the editor + clears the
    /// canvas cache. Solve+bake is queued on the next mutation cycle.
    fn with_selected_sketch_pad<F>(
        &mut self,
        id: signex_sketch::id::SketchEntityId,
        f: F,
    ) where
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
    }
    pub(crate) fn fp_editor_set_selected_pad_designator(&mut self, idx: usize, value: String) {
        self.with_selected_pad(idx, |pad| pad.number = value);
    }
    pub(crate) fn fp_editor_set_selected_pad_side(
        &mut self,
        idx: usize,
        side: crate::library::editor::footprint::state::PadSide,
    ) {
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
    }
    pub(crate) fn fp_editor_set_selected_pad_shape(
        &mut self,
        idx: usize,
        shape: signex_library::PadShape,
    ) {
        self.with_selected_pad(idx, |pad| pad.shape = shape);
    }
    pub(crate) fn fp_editor_set_selected_pad_kind(
        &mut self,
        idx: usize,
        kind: signex_library::PadKind,
    ) {
        self.with_selected_pad(idx, |pad| pad.kind = kind);
    }
    pub(crate) fn fp_editor_set_selected_pad_size_x(&mut self, idx: usize, value: String) {
        if let Ok(parsed) = value.trim().parse::<f64>() {
            if parsed > 0.0 {
                self.with_selected_pad(idx, |pad| pad.size_mm.0 = parsed);
            }
        }
    }
    pub(crate) fn fp_editor_set_selected_pad_size_y(&mut self, idx: usize, value: String) {
        if let Ok(parsed) = value.trim().parse::<f64>() {
            if parsed > 0.0 {
                self.with_selected_pad(idx, |pad| pad.size_mm.1 = parsed);
            }
        }
    }
    pub(crate) fn fp_editor_set_selected_pad_drill_diameter(&mut self, idx: usize, value: String) {
        let parsed = fp_parse_optional_mm(&value);
        self.with_selected_pad(idx, |pad| pad.drill_diameter_mm = parsed);
    }
    pub(crate) fn fp_editor_set_selected_pad_drill_slot_length(&mut self, idx: usize, _value: String) {
        // v0.20 placeholder — slot length not yet on EditorPad. Wired
        // when `EditorPad` gains a separate `drill_slot_length_mm`.
        let _ = idx;
        self.refresh_panel_ctx();
    }
    pub(crate) fn fp_editor_set_selected_pad_corner_radius_pct(&mut self, idx: usize, value: String) {
        let parsed = value.trim().parse::<f64>().ok().filter(|v| (0.0..=50.0).contains(v));
        self.with_selected_pad(idx, |pad| pad.stack.corner_radius_pct = parsed);
    }
    pub(crate) fn fp_editor_set_selected_pad_template(&mut self, idx: usize, value: String) {
        self.with_selected_pad(idx, |pad| pad.template = value);
    }
    pub(crate) fn fp_editor_set_selected_pad_template_library(&mut self, idx: usize, value: String) {
        self.with_selected_pad(idx, |pad| pad.template_library = value);
    }
    pub(crate) fn fp_editor_set_selected_pad_paste_margin_top(&mut self, idx: usize, value: String) {
        let parsed = fp_parse_optional_mm(&value);
        self.with_selected_pad(idx, |pad| pad.stack.paste_margin_top = parsed);
    }
    pub(crate) fn fp_editor_set_selected_pad_paste_margin_bottom(&mut self, idx: usize, value: String) {
        let parsed = fp_parse_optional_mm(&value);
        self.with_selected_pad(idx, |pad| pad.stack.paste_margin_bottom = parsed);
    }
    pub(crate) fn fp_editor_toggle_selected_pad_paste_enabled_top(&mut self, idx: usize, on: bool) {
        self.with_selected_pad(idx, |pad| pad.stack.paste_enabled_top = on);
    }
    pub(crate) fn fp_editor_toggle_selected_pad_paste_enabled_bottom(&mut self, idx: usize, on: bool) {
        self.with_selected_pad(idx, |pad| pad.stack.paste_enabled_bottom = on);
    }
    pub(crate) fn fp_editor_set_selected_pad_mask_margin_top(&mut self, idx: usize, value: String) {
        let parsed = fp_parse_optional_mm(&value);
        self.with_selected_pad(idx, |pad| pad.stack.mask_margin_top = parsed);
    }
    pub(crate) fn fp_editor_set_selected_pad_mask_margin_bottom(&mut self, idx: usize, value: String) {
        let parsed = fp_parse_optional_mm(&value);
        self.with_selected_pad(idx, |pad| pad.stack.mask_margin_bottom = parsed);
    }
    pub(crate) fn fp_editor_toggle_selected_pad_mask_tented_top(&mut self, idx: usize, on: bool) {
        self.with_selected_pad(idx, |pad| pad.stack.mask_tented_top = on);
    }
    pub(crate) fn fp_editor_toggle_selected_pad_mask_tented_bottom(&mut self, idx: usize, on: bool) {
        self.with_selected_pad(idx, |pad| pad.stack.mask_tented_bottom = on);
    }
    pub(crate) fn fp_editor_toggle_selected_pad_thermal_relief(&mut self, idx: usize, on: bool) {
        self.with_selected_pad(idx, |pad| pad.stack.thermal_relief = on);
    }
    pub(crate) fn fp_editor_set_selected_pad_feature_top(
        &mut self,
        idx: usize,
        value: signex_sketch::attr::PadFeature,
    ) {
        self.with_selected_pad(idx, |pad| pad.feature_top = value);
    }
    pub(crate) fn fp_editor_set_selected_pad_feature_bottom(
        &mut self,
        idx: usize,
        value: signex_sketch::attr::PadFeature,
    ) {
        self.with_selected_pad(idx, |pad| pad.feature_bottom = value);
    }
    pub(crate) fn fp_editor_toggle_selected_pad_testpoint_top_assembly(&mut self, idx: usize, on: bool) {
        self.with_selected_pad(idx, |pad| pad.testpoint.top_assembly = on);
    }
    pub(crate) fn fp_editor_toggle_selected_pad_testpoint_top_fab(&mut self, idx: usize, on: bool) {
        self.with_selected_pad(idx, |pad| pad.testpoint.top_fab = on);
    }
    pub(crate) fn fp_editor_toggle_selected_pad_testpoint_bottom_assembly(&mut self, idx: usize, on: bool) {
        self.with_selected_pad(idx, |pad| pad.testpoint.bottom_assembly = on);
    }
    pub(crate) fn fp_editor_toggle_selected_pad_testpoint_bottom_fab(&mut self, idx: usize, on: bool) {
        self.with_selected_pad(idx, |pad| pad.testpoint.bottom_fab = on);
    }

    /// v0.16.4 — mutate the selected entity's pour `net` and re-bake.
    pub(crate) fn fp_editor_set_pour_net(
        &mut self,
        id: signex_sketch::id::SketchEntityId,
        value: String,
    ) {
        let net = if value.trim().is_empty() {
            None
        } else {
            Some(value)
        };
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                if let Some(e) = sketch.entities.iter_mut().find(|e| e.id == id) {
                    if let Some(p) = e.pour.as_mut() {
                        p.net = net;
                    }
                }
            }
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
    }

    pub(crate) fn fp_editor_set_pour_fill_type(
        &mut self,
        id: signex_sketch::id::SketchEntityId,
        value: signex_sketch::attr::PourFillType,
    ) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                if let Some(e) = sketch.entities.iter_mut().find(|e| e.id == id) {
                    if let Some(p) = e.pour.as_mut() {
                        p.fill_type = value;
                    }
                }
            }
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
    }

    pub(crate) fn fp_editor_set_pour_priority(
        &mut self,
        id: signex_sketch::id::SketchEntityId,
        value: String,
    ) {
        let parsed = value.trim().parse::<u32>().ok();
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                if let Some(e) = sketch.entities.iter_mut().find(|e| e.id == id) {
                    if let Some(p) = e.pour.as_mut() {
                        if let Some(n) = parsed {
                            p.priority = n;
                        }
                    }
                }
            }
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
    }

    pub(crate) fn fp_editor_set_keepout_kind(
        &mut self,
        id: signex_sketch::id::SketchEntityId,
        kind: crate::panels::KeepoutKindFlag,
        value: bool,
    ) {
        use crate::panels::KeepoutKindFlag;
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                if let Some(e) = sketch.entities.iter_mut().find(|e| e.id == id) {
                    if let Some(k) = e.keepout.as_mut() {
                        match kind {
                            KeepoutKindFlag::NoRouting => k.kinds.no_routing = value,
                            KeepoutKindFlag::NoComponents => k.kinds.no_components = value,
                            KeepoutKindFlag::NoCopper => k.kinds.no_copper = value,
                            KeepoutKindFlag::NoVias => k.kinds.no_vias = value,
                            KeepoutKindFlag::NoDrilling => k.kinds.no_drilling = value,
                            KeepoutKindFlag::NoPours => k.kinds.no_pours = value,
                        }
                    }
                }
            }
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
    }

    pub(crate) fn fp_editor_set_cutout_edge_radius(
        &mut self,
        id: signex_sketch::id::SketchEntityId,
        value: String,
    ) {
        let edge_radius = if value.trim().is_empty() {
            None
        } else {
            Some(value)
        };
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                if let Some(e) = sketch.entities.iter_mut().find(|e| e.id == id) {
                    if let Some(c) = e.board_cutout.as_mut() {
                        c.edge_radius_expr = edge_radius;
                    }
                }
            }
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
    }

    pub(crate) fn fp_editor_toggle_snap_option(&mut self, flag: crate::panels::SnapOptionFlag) {
        use crate::panels::SnapOptionFlag;
        if let Some(editor) = self.active_footprint_editor_mut() {
            let opts = &mut editor.state.snap_options;
            match flag {
                SnapOptionFlag::PointHit => opts.point_hit = !opts.point_hit,
                SnapOptionFlag::HorizontalVertical => {
                    opts.horizontal_vertical = !opts.horizontal_vertical
                }
                SnapOptionFlag::Angle => opts.angle = !opts.angle,
                SnapOptionFlag::Grid => opts.grid = !opts.grid,
                SnapOptionFlag::TrackVertices => {
                    opts.snap_track_vertices = !opts.snap_track_vertices
                }
                SnapOptionFlag::TrackLines => opts.snap_track_lines = !opts.snap_track_lines,
                SnapOptionFlag::ArcCenters => opts.snap_arc_centers = !opts.snap_arc_centers,
                SnapOptionFlag::Intersections => {
                    opts.snap_intersections = !opts.snap_intersections
                }
                SnapOptionFlag::PadCenters => opts.snap_pad_centers = !opts.snap_pad_centers,
                SnapOptionFlag::PadVertices => opts.snap_pad_vertices = !opts.snap_pad_vertices,
                SnapOptionFlag::PadEdges => opts.snap_pad_edges = !opts.snap_pad_edges,
                SnapOptionFlag::ViaCenters => opts.snap_via_centers = !opts.snap_via_centers,
                SnapOptionFlag::Texts => opts.snap_texts = !opts.snap_texts,
                SnapOptionFlag::Regions => opts.snap_regions = !opts.snap_regions,
                SnapOptionFlag::FootprintOrigins => {
                    opts.snap_footprint_origins = !opts.snap_footprint_origins
                }
                SnapOptionFlag::Body3dPoints => opts.snap_3d_body_points = !opts.snap_3d_body_points,
                SnapOptionFlag::SnapToGrids => opts.snap_to_grids = !opts.snap_to_grids,
                SnapOptionFlag::SnapToGuides => opts.snap_to_guides = !opts.snap_to_guides,
                SnapOptionFlag::SnapToAxes => opts.snap_to_axes = !opts.snap_to_axes,
            }
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
    }

    /// v0.13 — Altium "Snap Distance" setter.
    pub(crate) fn handle_fp_set_snap_distance(&mut self, raw: String) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Ok(v) = raw.trim().parse::<f64>() {
                editor.state.snap_options.snap_distance_mm = v.clamp(0.001, 100.0);
                editor.canvas_cache.clear();
            }
        }
        self.refresh_panel_ctx();
    }

    /// v0.13 — Altium "Axis Snap Range" setter.
    pub(crate) fn handle_fp_set_axis_snap_range(&mut self, raw: String) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Ok(v) = raw.trim().parse::<f64>() {
                editor.state.snap_options.axis_snap_range_mm = v.clamp(0.001, 100.0);
                editor.canvas_cache.clear();
            }
        }
        self.refresh_panel_ctx();
    }

    /// v0.18.9 — Properties-panel "Grid step" numeric input. Parses
    /// the user's text; on a clean positive parse writes
    /// `state.snap_options.grid_step_mm`. Invalid / empty / non-
    /// positive strings no-op so partial keystrokes don't snap to
    /// zero (which would crash the canvas's grid math).
    pub(crate) fn fp_editor_set_snap_grid_step(&mut self, value: &str) {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return;
        }
        let parsed: f64 = match trimmed.parse::<f64>() {
            Ok(v) if v > 0.0 && v.is_finite() => v,
            _ => return,
        };
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.state.snap_options.grid_step_mm = parsed;
            // v0.18.21 — mirror onto the active grid row so the
            // Manager view + the canvas stay aligned.
            let idx = editor.state.active_grid_idx;
            if let Some(row) = editor.state.grids.get_mut(idx) {
                row.step_mm = parsed;
            }
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
    }

    pub(crate) fn fp_editor_set_cutout_through(
        &mut self,
        id: signex_sketch::id::SketchEntityId,
        value: bool,
    ) {
        if let Some(editor) = self.active_footprint_editor_mut() {
            if let Some(sketch) = editor.primitive_mut().sketch.as_mut() {
                if let Some(e) = sketch.entities.iter_mut().find(|e| e.id == id) {
                    if let Some(c) = e.board_cutout.as_mut() {
                        c.through = value;
                    }
                }
            }
            use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
            use crate::library::editor::footprint::sketch_mode::SketchEdit;
            editor.with_parts(|state, primitive| {
                apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
            });
            editor.dirty = true;
            editor.canvas_cache.clear();
        }
        self.refresh_panel_ctx();
    }
}

/// Apply one numeric Properties-pane edit to a graphic. (idx, field)
/// pairs whose field doesn't apply to the graphic's variant silently
/// no-op so a stale Properties pane can't mutate the wrong slot.
/// Click-to-cycle the symbol's local color override through a small
/// preset palette and back to `None` (= inherit). 5 steps total:
/// None → red → green → blue → yellow → back to None.
/// v0.21 — Silk Line endpoint to mutate.
#[derive(Debug, Clone, Copy)]
pub(crate) enum SilkLineEndpoint {
    FromX,
    FromY,
    ToX,
    ToY,
}

/// v0.21 — Silk Text field to mutate.
#[derive(Debug, Clone, Copy)]
pub(crate) enum SilkTextField {
    PositionX,
    PositionY,
    Size,
}

impl Signex {
    pub(crate) fn fp_editor_set_silk_line_endpoint(
        &mut self,
        endpoint: SilkLineEndpoint,
        value: String,
    ) {
        let parsed = value.trim().parse::<f64>().ok();
        if let Some(parsed) = parsed {
            if let Some(editor) = self.active_footprint_editor_mut() {
                if let Some(idx) = editor.state.selected_silk_f {
                    if let Some(g) = editor.primitive_mut().silk_f.get_mut(idx) {
                        if let signex_library::primitive::footprint::FpGraphicKind::Line {
                            from,
                            to,
                        } = &mut g.kind
                        {
                            match endpoint {
                                SilkLineEndpoint::FromX => from[0] = parsed,
                                SilkLineEndpoint::FromY => from[1] = parsed,
                                SilkLineEndpoint::ToX => to[0] = parsed,
                                SilkLineEndpoint::ToY => to[1] = parsed,
                            }
                            editor.dirty = true;
                            editor.canvas_cache.clear();
                        }
                    }
                }
            }
        }
        self.refresh_panel_ctx();
    }

    pub(crate) fn fp_editor_set_silk_text_field(
        &mut self,
        field: SilkTextField,
        value: String,
    ) {
        let parsed = value.trim().parse::<f64>().ok();
        if let Some(parsed) = parsed {
            if let Some(editor) = self.active_footprint_editor_mut() {
                if let Some(idx) = editor.state.selected_silk_f {
                    if let Some(g) = editor.primitive_mut().silk_f.get_mut(idx) {
                        if let signex_library::primitive::footprint::FpGraphicKind::Text {
                            position,
                            size,
                            ..
                        } = &mut g.kind
                        {
                            match field {
                                SilkTextField::PositionX => position[0] = parsed,
                                SilkTextField::PositionY => position[1] = parsed,
                                SilkTextField::Size => {
                                    if parsed > 0.0 {
                                        *size = parsed;
                                    }
                                }
                            }
                            editor.dirty = true;
                            editor.canvas_cache.clear();
                        }
                    }
                }
            }
        }
        self.refresh_panel_ctx();
    }
}

/// v0.20 — parse a Properties-panel mm input as `Option<f64>`.
/// Empty / whitespace = `None` (means "use rule"); non-numeric =
/// `None` (the form re-displays the previous value, so the user
/// can keep typing). Used by every per-side mask / paste row.
fn fp_parse_optional_mm(value: &str) -> Option<f64> {
    let s = value.trim();
    if s.is_empty() {
        return None;
    }
    s.parse::<f64>().ok()
}

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
