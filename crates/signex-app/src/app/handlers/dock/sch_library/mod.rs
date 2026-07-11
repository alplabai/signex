//! Dock library/editor panel message dispatcher. Routes the
//! `DockMessage::Panel(PanelMsg::…)` variants that mutate the active
//! `.snxsym` / `.snxfpt` container — SCH Library, Symbol editor,
//! Footprint Library, and Footprint editor panels.
//!
//! `handle_dock_sch_library_message` is the fat match; every arm with
//! real logic delegates to a helper method living in the matching
//! concern module:
//!   - [`symbol`] — `SchLibrary*` / `SymEditor*` mutators.
//!   - [`footprint_pad`] — active-editor accessors + pad-defaults setters.
//!   - [`footprint_shape`] — pour / keepout / cutout / snap / array / silk.
//!
//! Mutations mark the tab dirty and clear the canvas cache; the actual
//! save to disk happens through the existing Save flow
//! (`save_primitive_tab_at`) so the panel never writes the file
//! directly — keeps the dirty / save semantics consistent with every
//! other in-tab mutation.
//!
//! Split out of the former `sch_library.rs` god-file (ADR-0001 #163);
//! pure code motion, zero behaviour change.

use iced::Task;

use super::super::super::*;

mod footprint_pad;
mod footprint_shape;
mod symbol;

use footprint_shape::{SilkLineEndpoint, SilkTextField};

impl Signex {
    /// v0.18.8 — convenience: resolve the active tab's `.snxfpt`
    /// path, if any. The Footprint Library panel handlers below all
    /// need this; centralising it keeps the dispatch arms tight.
    pub(crate) fn active_footprint_editor_path(&self) -> Option<std::path::PathBuf> {
        let active_tab = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)?;
        active_tab.kind.as_footprint_editor().cloned()
    }

    /// Returns `None` when the message isn't an SCH-library message (so
    /// the caller falls through to the next dock handler), or `Some(task)`
    /// when handled — the task carries any follow-up work from a
    /// re-entrant `self.update(...)` so it isn't dropped.
    pub(super) fn handle_dock_sch_library_message(
        &mut self,
        panel_msg: &crate::panels::PanelMsg,
    ) -> Option<Task<Message>> {
        let mut follow = Task::none();
        let handled = match panel_msg {
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
                    follow = self.update(Message::Library(
                        crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                            path: path.clone(),
                            msg: crate::library::messages::PrimitiveEdit::Footprint(
                                crate::library::messages::FootprintEditorMsg::AddNewSibling,
                            ),
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
                    follow = self.update(Message::Library(
                        crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                            path,
                            msg: crate::library::messages::PrimitiveEdit::Footprint(
                                crate::library::messages::FootprintEditorMsg::SelectActiveIdx(*idx),
                            ),
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
                        follow = self.update(Message::Library(
                            crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                                path,
                                msg: crate::library::messages::PrimitiveEdit::Footprint(
                                    crate::library::messages::FootprintEditorMsg::ToggleAutoFit,
                                ),
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
                        follow = self.update(Message::Library(
                            crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                                path,
                                msg: crate::library::messages::PrimitiveEdit::Footprint(
                                    crate::library::messages::FootprintEditorMsg::SketchSetRole {
                                        id: *id,
                                        role: *role,
                                    },
                                ),
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
            crate::panels::PanelMsg::FpEditorToggleSelectedPadTestpointTopAssembly {
                idx,
                value,
            } => {
                self.fp_editor_toggle_selected_pad_testpoint_top_assembly(*idx, *value);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSelectedPadTestpointTopFab { idx, value } => {
                self.fp_editor_toggle_selected_pad_testpoint_top_fab(*idx, *value);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleSelectedPadTestpointBottomAssembly {
                idx,
                value,
            } => {
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
                    editor.state.next_pad_defaults.hole_tolerance_plus_mm = fp_parse_optional_mm(v);
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
                    editor.state.next_pad_defaults.hole_rotation_deg = v.trim().parse::<f64>().ok();
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadCopperOffsetX(v) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.state.next_pad_defaults.copper_offset_x_mm = fp_parse_optional_mm(v);
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorSetNextPadCopperOffsetY(v) => {
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.state.next_pad_defaults.copper_offset_y_mm = fp_parse_optional_mm(v);
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::FpEditorToggleNextPadPlated(plated) => {
                use signex_library::PadKind as Pk;
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.state.next_pad_defaults.kind =
                        if *plated { Pk::Tht } else { Pk::NptHole };
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
            crate::panels::PanelMsg::FpEditorToggleSketchPadTestpointBottomAssembly {
                id,
                value,
            } => {
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
            crate::panels::PanelMsg::FpEditorEditPadShapeParam {
                pad_idx,
                key,
                value,
            } => {
                // v0.24 Phase 3 (Track A2) — Properties-panel
                // parametric-handle edit. Resolve the bound sketch
                // parameter via `pad.shape_params[key]`, then forward
                // to `FootprintSketchEditParameter` which upserts the
                // expression and triggers a solve+rebake. Undo
                // snapshot is captured at the dispatcher level via
                // `mutates_footprint_state` (defaults to true for any
                // unrecognised FootprintEditorMsg variant — verified
                // for `FootprintSketchEditParameter` already).
                let parameter_name = self.active_footprint_editor_mut().and_then(|editor| {
                    editor
                        .state
                        .pads
                        .get(*pad_idx)
                        .and_then(|pad| pad.shape_params.get(key).cloned())
                });
                if let Some(name) = parameter_name {
                    if let Some(active_tab) =
                        self.document_state.tabs.get(self.document_state.active_tab)
                    {
                        if let Some(path) = active_tab.kind.as_footprint_editor() {
                            let path = path.clone();
                            follow = self.update(Message::Library(
                                crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                                    path,
                                    msg: crate::library::messages::PrimitiveEdit::Footprint(
                                        crate::library::messages::FootprintEditorMsg::SketchEditParameter {
                                            name,
                                            expr: value.clone(),
                                        },
                                    ),
                                },
                            ));
                            self.refresh_panel_ctx();
                        }
                    }
                } else {
                    tracing::warn!(
                        target: "signex::v024",
                        "FpEditorEditPadShapeParam: pad {pad_idx} has no shape_params[{key}] \
                         binding; ignoring edit"
                    );
                }
                true
            }
            crate::panels::PanelMsg::FpEditorUnlinkCornerRadius { arc_entity_id } => {
                // v0.24 Phase 3 (Track A3) — forward to the
                // `FootprintEditorMsg::SketchUnlinkCornerRadius`.
                // The dispatcher walks pads for the matching arc,
                // mints the per-corner parameter, and triggers a
                // solve+rebake. Undo snapshot captured at dispatcher
                // level via mutates_footprint_state.
                if let Some(active_tab) =
                    self.document_state.tabs.get(self.document_state.active_tab)
                {
                    if let Some(path) = active_tab.kind.as_footprint_editor() {
                        let path = path.clone();
                        follow = self.update(Message::Library(
                            crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                                path,
                                msg: crate::library::messages::PrimitiveEdit::Footprint(
                                    crate::library::messages::FootprintEditorMsg::SketchUnlinkCornerRadius {
                                        arc_entity_id: *arc_entity_id,
                                    },
                                ),
                            },
                        ));
                        self.refresh_panel_ctx();
                    }
                }
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
            crate::panels::PanelMsg::FpEditorSelectSketchEntity { id } => {
                // v0.22 Phase E3+E4 — Properties-panel "Conflicts"
                // row click → set the sketch entity as the primary
                // selection so the canvas re-renders with that
                // entity's constraint icons highlighted in red.
                if let Some(editor) = self.active_footprint_editor_mut() {
                    editor.state.selected_sketch = Some(*id);
                    editor.state.selected_sketch_secondary = None;
                    editor.canvas_cache.clear();
                }
                self.refresh_panel_ctx();
                true
            }
            crate::panels::PanelMsg::HistoryRestoreClicked { sha } => {
                // v0.22 Phase 8.5 — History panel "Restore this
                // version" button. Resolve the active tab's owning
                // project, open `LocalGitProjectAdapter`, and run
                // `restore_at(rel_path, oid)` to overwrite the
                // working-tree file with the historical blob. Mark
                // the file dirty so the next save commits the
                // restored content.
                self.handle_history_restore_clicked(sha);
                true
            }
            crate::panels::PanelMsg::FpEditorHoverOverConstraint { constraint } => {
                // v0.23 — per-row precision. `Some(id)` isolates a
                // single constraint at full red while every other
                // glyph (including other over-constraints) dims.
                // `None` clears back to the default rendering.
                if let Some(editor) = self.active_footprint_editor_mut() {
                    if editor.state.conflicts_row_hovered != *constraint {
                        editor.state.conflicts_row_hovered = *constraint;
                        editor.canvas_cache.clear();
                    }
                }
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
            crate::panels::PanelMsg::FpEditorEditArrayParam {
                array_id,
                field,
                value,
            } => {
                self.fp_editor_edit_array_param(*array_id, *field, value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetArrayNumberingScheme { array_id, scheme } => {
                self.fp_editor_set_array_numbering_scheme(*array_id, *scheme);
                true
            }
            crate::panels::PanelMsg::FpEditorSetBgaSkipLetters { array_id, value } => {
                self.fp_editor_set_bga_skip_letters(*array_id, *value);
                true
            }
            crate::panels::PanelMsg::FpEditorSetBgaStartRow { array_id, value } => {
                self.fp_editor_set_bga_start_row(*array_id, value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorSetBgaStartCol { array_id, value } => {
                self.fp_editor_set_bga_start_col(*array_id, value.clone());
                true
            }
            crate::panels::PanelMsg::FpEditorDeleteArray { array_id } => {
                self.fp_editor_delete_array(*array_id);
                true
            }
            crate::panels::PanelMsg::FpEditorBeginRepickPolarCenter { array_id } => {
                self.fp_editor_begin_repick_polar_center(*array_id);
                true
            }
            crate::panels::PanelMsg::FpEditorToggleArrayInstance {
                array_id,
                i,
                j,
                value,
            } => {
                self.fp_editor_toggle_array_instance(*array_id, *i, *j, *value);
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
                follow = self.update(Message::SelectionFilter(SelectionFilterMsg::OpenCustom));
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
                follow = self.update(Message::GridProperties(GridPropertiesMsg::Open));
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
                        follow = self.update(Message::Library(
                            crate::library::messages::LibraryMessage::PrimitiveEditorEvent {
                                path,
                                msg: crate::library::messages::PrimitiveEdit::Footprint(
                                    crate::library::messages::FootprintEditorMsg::SketchEditParameter {
                                        name: name.clone(),
                                        expr: expr.clone(),
                                    },
                                ),
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
        };
        if handled { Some(follow) } else { None }
    }
}

/// v0.20 — parse a Properties-panel mm input as `Option<f64>`.
/// Empty / whitespace = `None` (means "use rule"); non-numeric =
/// `None` (the form re-displays the previous value, so the user
/// can keep typing). Used by every per-side mask / paste row.
pub(super) fn fp_parse_optional_mm(value: &str) -> Option<f64> {
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
