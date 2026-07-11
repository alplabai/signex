//! Dock library/editor panel message dispatcher. Routes the
//! `DockMessage::Panel(PanelMsg::…)` variants that mutate the active
//! `.snxsym` / `.snxfpt` container — SCH Library, Symbol editor,
//! Footprint Library, and Footprint editor panels.
//!
//! `handle_dock_sch_library_message` is a thin router; every arm with
//! real logic delegates to a `handle_*` method living in the matching
//! concern module:
//!   - [`symbol`] — `SchLibrary*` / `SymEditor*` mutators.
//!   - [`footprint_pad`] — active-editor accessors + pad-defaults setters.
//!   - [`footprint_shape`] — pour / keepout / cutout / snap / array / silk.
//!   - [`footprint_library`] — Footprint Library panel (envelope CRUD).
//!   - [`footprint_props`] — footprint component-level properties.
//!   - [`footprint_grid`] — grid / guide managers + snap sub-tab / mode.
//!   - [`footprint_silk`] — selected silk-graphic edits.
//!   - [`footprint_sketch`] — sketch-entity jumps + parameter forwards.
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
use crate::panels::PanelMsg;

mod footprint_grid;
mod footprint_library;
mod footprint_pad;
mod footprint_props;
mod footprint_shape;
mod footprint_silk;
mod footprint_sketch;
mod symbol;

use footprint_silk::{SilkLineEndpoint, SilkTextField};

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
        panel_msg: &PanelMsg,
    ) -> Option<Task<Message>> {
        let mut follow = Task::none();
        let handled = match panel_msg {
            PanelMsg::FpLibraryOpenSibling(sibling_path) => {
                self.handle_fp_library_open_sibling(sibling_path)
            }
            PanelMsg::FpLibrarySelectInternal(idx) => self.handle_fp_library_select_internal(idx),
            PanelMsg::FpLibraryAddInternal => {
                follow = self.handle_fp_library_add_internal();
                true
            }
            PanelMsg::FpLibraryDeleteInternal(idx) => self.handle_fp_library_delete_internal(idx),
            PanelMsg::FpLibraryEditInternal(idx) => {
                follow = self.handle_fp_library_edit_internal(idx);
                true
            }
            // v0.18.8 — `Place` button. PCB Component placement is
            // not wired yet; log a warn so the action is observable
            // without a visible no-op.
            PanelMsg::FpLibraryPlaceInternal(idx) => {
                tracing::warn!(
                    target: "signex::library",
                    idx = idx,
                    "Footprint Library: Place is not yet wired (PCB integration pending)",
                );
                true
            }
            PanelMsg::FpEditorToggleAutoFitCourtyard => {
                follow = self.handle_fp_editor_toggle_auto_fit_courtyard();
                true
            }
            PanelMsg::FpEditorSetRole { id, role } => {
                follow = self.handle_fp_editor_set_role(id, role);
                true
            }
            PanelMsg::FpEditorSetNextPadDesignator(value) => {
                self.fp_editor_set_next_pad_designator(value.clone())
            }
            PanelMsg::FpEditorSetNextPadSizeX(value) => {
                self.fp_editor_set_next_pad_size_x(value.clone())
            }
            PanelMsg::FpEditorSetNextPadSizeY(value) => {
                self.fp_editor_set_next_pad_size_y(value.clone())
            }
            PanelMsg::FpEditorSetNextPadSide(side) => self.fp_editor_set_next_pad_side(*side),
            PanelMsg::FpEditorSetNextPadRotation(value) => {
                self.fp_editor_set_next_pad_rotation(value.clone())
            }
            PanelMsg::FpEditorSetSelectedPadRotation { idx, value } => {
                self.fp_editor_set_selected_pad_rotation(*idx, value.clone())
            }
            // v0.20 — Altium-parity Pad Properties / Pad Stack / Pad
            // Features form for the next placed pad. Each handler
            // mutates the matching slice of `editor.state.next_pad_defaults`
            // (and its `stack` sub-struct) so the next placement
            // click picks up the new value.
            PanelMsg::FpEditorSetNextPadShape(shape) => {
                self.fp_editor_set_next_pad_shape(shape.clone())
            }
            PanelMsg::FpEditorSetNextPadKind(kind) => self.fp_editor_set_next_pad_kind(*kind),
            PanelMsg::FpEditorSetNextPadDrillDiameter(v) => {
                self.fp_editor_set_next_pad_drill_diameter(v.clone())
            }
            PanelMsg::FpEditorSetNextPadDrillSlotLength(v) => {
                self.fp_editor_set_next_pad_drill_slot_length(v.clone())
            }
            PanelMsg::FpEditorSetNextPadCornerRadiusPct(v) => {
                self.fp_editor_set_next_pad_corner_radius_pct(v.clone())
            }
            PanelMsg::FpEditorSetNextPadTemplate(v) => {
                self.fp_editor_set_next_pad_template(v.clone())
            }
            PanelMsg::FpEditorSetNextPadTemplateLibrary(v) => {
                self.fp_editor_set_next_pad_template_library(v.clone())
            }
            PanelMsg::FpEditorSetNextPadPasteMarginTop(v) => {
                self.fp_editor_set_next_pad_paste_margin_top(v.clone())
            }
            PanelMsg::FpEditorSetNextPadPasteMarginBottom(v) => {
                self.fp_editor_set_next_pad_paste_margin_bottom(v.clone())
            }
            PanelMsg::FpEditorToggleNextPadPasteEnabledTop(on) => {
                self.fp_editor_toggle_next_pad_paste_enabled_top(*on)
            }
            PanelMsg::FpEditorToggleNextPadPasteEnabledBottom(on) => {
                self.fp_editor_toggle_next_pad_paste_enabled_bottom(*on)
            }
            PanelMsg::FpEditorSetNextPadMaskMarginTop(v) => {
                self.fp_editor_set_next_pad_mask_margin_top(v.clone())
            }
            PanelMsg::FpEditorSetNextPadMaskMarginBottom(v) => {
                self.fp_editor_set_next_pad_mask_margin_bottom(v.clone())
            }
            PanelMsg::FpEditorToggleNextPadMaskTentedTop(on) => {
                self.fp_editor_toggle_next_pad_mask_tented_top(*on)
            }
            PanelMsg::FpEditorToggleNextPadMaskTentedBottom(on) => {
                self.fp_editor_toggle_next_pad_mask_tented_bottom(*on)
            }
            PanelMsg::FpEditorToggleNextPadThermalRelief(on) => {
                self.fp_editor_toggle_next_pad_thermal_relief(*on)
            }
            PanelMsg::FpEditorSetNextPadFeatureTop(f) => {
                self.fp_editor_set_next_pad_feature_top(*f)
            }
            PanelMsg::FpEditorSetNextPadFeatureBottom(f) => {
                self.fp_editor_set_next_pad_feature_bottom(*f)
            }
            PanelMsg::FpEditorToggleNextPadTestpointTopAssembly(on) => {
                self.fp_editor_toggle_next_pad_testpoint_top_assembly(*on)
            }
            PanelMsg::FpEditorToggleNextPadTestpointTopFab(on) => {
                self.fp_editor_toggle_next_pad_testpoint_top_fab(*on)
            }
            PanelMsg::FpEditorToggleNextPadTestpointBottomAssembly(on) => {
                self.fp_editor_toggle_next_pad_testpoint_bottom_assembly(*on)
            }
            PanelMsg::FpEditorToggleNextPadTestpointBottomFab(on) => {
                self.fp_editor_toggle_next_pad_testpoint_bottom_fab(*on)
            }
            // v0.20 — selected-pad editing routes. Each handler mutates
            // `state.pads[idx]` and triggers a dirty + canvas-cache
            // invalidate so the panel re-renders with the new value.
            PanelMsg::FpEditorSetSelectedPadDesignator { idx, value } => {
                self.fp_editor_set_selected_pad_designator(*idx, value.clone())
            }
            PanelMsg::FpEditorSetSelectedPadSide { idx, side } => {
                self.fp_editor_set_selected_pad_side(*idx, *side)
            }
            PanelMsg::FpEditorSetSelectedPadShape { idx, shape } => {
                self.fp_editor_set_selected_pad_shape(*idx, shape.clone())
            }
            PanelMsg::FpEditorSetSelectedPadKind { idx, kind } => {
                self.fp_editor_set_selected_pad_kind(*idx, *kind)
            }
            PanelMsg::FpEditorSetSelectedPadSizeX { idx, value } => {
                self.fp_editor_set_selected_pad_size_x(*idx, value.clone())
            }
            PanelMsg::FpEditorSetSelectedPadSizeY { idx, value } => {
                self.fp_editor_set_selected_pad_size_y(*idx, value.clone())
            }
            PanelMsg::FpEditorSetSelectedPadDrillDiameter { idx, value } => {
                self.fp_editor_set_selected_pad_drill_diameter(*idx, value.clone())
            }
            PanelMsg::FpEditorSetSelectedPadDrillSlotLength { idx, value } => {
                self.fp_editor_set_selected_pad_drill_slot_length(*idx, value.clone())
            }
            PanelMsg::FpEditorSetSelectedPadCornerRadiusPct { idx, value } => {
                self.fp_editor_set_selected_pad_corner_radius_pct(*idx, value.clone())
            }
            PanelMsg::FpEditorSetSelectedPadTemplate { idx, value } => {
                self.fp_editor_set_selected_pad_template(*idx, value.clone())
            }
            PanelMsg::FpEditorSetSelectedPadTemplateLibrary { idx, value } => {
                self.fp_editor_set_selected_pad_template_library(*idx, value.clone())
            }
            PanelMsg::FpEditorSetSelectedPadPasteMarginTop { idx, value } => {
                self.fp_editor_set_selected_pad_paste_margin_top(*idx, value.clone())
            }
            PanelMsg::FpEditorSetSelectedPadPasteMarginBottom { idx, value } => {
                self.fp_editor_set_selected_pad_paste_margin_bottom(*idx, value.clone())
            }
            PanelMsg::FpEditorToggleSelectedPadPasteEnabledTop { idx, value } => {
                self.fp_editor_toggle_selected_pad_paste_enabled_top(*idx, *value)
            }
            PanelMsg::FpEditorToggleSelectedPadPasteEnabledBottom { idx, value } => {
                self.fp_editor_toggle_selected_pad_paste_enabled_bottom(*idx, *value)
            }
            PanelMsg::FpEditorSetSelectedPadMaskMarginTop { idx, value } => {
                self.fp_editor_set_selected_pad_mask_margin_top(*idx, value.clone())
            }
            PanelMsg::FpEditorSetSelectedPadMaskMarginBottom { idx, value } => {
                self.fp_editor_set_selected_pad_mask_margin_bottom(*idx, value.clone())
            }
            PanelMsg::FpEditorToggleSelectedPadMaskTentedTop { idx, value } => {
                self.fp_editor_toggle_selected_pad_mask_tented_top(*idx, *value)
            }
            PanelMsg::FpEditorToggleSelectedPadMaskTentedBottom { idx, value } => {
                self.fp_editor_toggle_selected_pad_mask_tented_bottom(*idx, *value)
            }
            PanelMsg::FpEditorToggleSelectedPadThermalRelief { idx, value } => {
                self.fp_editor_toggle_selected_pad_thermal_relief(*idx, *value)
            }
            PanelMsg::FpEditorSetSelectedPadFeatureTop { idx, value } => {
                self.fp_editor_set_selected_pad_feature_top(*idx, *value)
            }
            PanelMsg::FpEditorSetSelectedPadFeatureBottom { idx, value } => {
                self.fp_editor_set_selected_pad_feature_bottom(*idx, *value)
            }
            PanelMsg::FpEditorToggleSelectedPadTestpointTopAssembly { idx, value } => {
                self.fp_editor_toggle_selected_pad_testpoint_top_assembly(*idx, *value)
            }
            PanelMsg::FpEditorToggleSelectedPadTestpointTopFab { idx, value } => {
                self.fp_editor_toggle_selected_pad_testpoint_top_fab(*idx, *value)
            }
            PanelMsg::FpEditorToggleSelectedPadTestpointBottomAssembly { idx, value } => {
                self.fp_editor_toggle_selected_pad_testpoint_bottom_assembly(*idx, *value)
            }
            PanelMsg::FpEditorToggleSelectedPadTestpointBottomFab { idx, value } => {
                self.fp_editor_toggle_selected_pad_testpoint_bottom_fab(*idx, *value)
            }
            PanelMsg::FpEditorSetPadStackTab(tab) => self.handle_fp_editor_set_pad_stack_tab(tab),
            PanelMsg::FpEditorSetNextPadElectricalType(v) => {
                self.handle_fp_editor_set_next_pad_electrical_type(v)
            }
            PanelMsg::FpEditorSetNextPadNet(v) => self.handle_fp_editor_set_next_pad_net(v),
            PanelMsg::FpEditorToggleNextPadLocked(on) => {
                self.handle_fp_editor_toggle_next_pad_locked(on)
            }
            PanelMsg::FpEditorSetSelectedPadElectricalType { idx, value } => {
                self.with_selected_pad(*idx, |pad| pad.electrical_type = *value)
            }
            PanelMsg::FpEditorSetSelectedPadNet { idx, value } => {
                self.with_selected_pad(*idx, |pad| pad.net = value.clone())
            }
            PanelMsg::FpEditorToggleSelectedPadLocked { idx, value } => {
                self.with_selected_pad(*idx, |pad| pad.locked = *value)
            }
            PanelMsg::FpEditorSetFootprintDescription(v) => {
                self.handle_fp_editor_set_footprint_description(v)
            }
            PanelMsg::FpEditorSetFootprintDefaultDesignator(v) => {
                self.handle_fp_editor_set_footprint_default_designator(v)
            }
            PanelMsg::FpEditorSetFootprintComponentType(t) => {
                self.handle_fp_editor_set_footprint_component_type(t)
            }
            PanelMsg::FpEditorSetFootprintHeight(v) => {
                self.handle_fp_editor_set_footprint_height(v)
            }
            PanelMsg::FpEditorSetSilkLineFromX(v) => {
                self.fp_editor_set_silk_line_endpoint(SilkLineEndpoint::FromX, v.clone())
            }
            PanelMsg::FpEditorSetSilkLineFromY(v) => {
                self.fp_editor_set_silk_line_endpoint(SilkLineEndpoint::FromY, v.clone())
            }
            PanelMsg::FpEditorSetSilkLineToX(v) => {
                self.fp_editor_set_silk_line_endpoint(SilkLineEndpoint::ToX, v.clone())
            }
            PanelMsg::FpEditorSetSilkLineToY(v) => {
                self.fp_editor_set_silk_line_endpoint(SilkLineEndpoint::ToY, v.clone())
            }
            PanelMsg::FpEditorSetSilkTextPositionX(v) => {
                self.fp_editor_set_silk_text_field(SilkTextField::PositionX, v.clone())
            }
            PanelMsg::FpEditorSetSilkTextPositionY(v) => {
                self.fp_editor_set_silk_text_field(SilkTextField::PositionY, v.clone())
            }
            PanelMsg::FpEditorSetSilkTextSize(v) => {
                self.fp_editor_set_silk_text_field(SilkTextField::Size, v.clone())
            }
            PanelMsg::FpEditorSetSilkStrokeWidth(v) => {
                self.handle_fp_editor_set_silk_stroke_width(v)
            }
            PanelMsg::FpEditorSetNextPadHoleTolerancePlus(v) => {
                self.handle_fp_editor_set_next_pad_hole_tolerance_plus(v)
            }
            PanelMsg::FpEditorSetNextPadHoleToleranceMinus(v) => {
                self.handle_fp_editor_set_next_pad_hole_tolerance_minus(v)
            }
            PanelMsg::FpEditorSetNextPadHoleRotation(v) => {
                self.handle_fp_editor_set_next_pad_hole_rotation(v)
            }
            PanelMsg::FpEditorSetNextPadCopperOffsetX(v) => {
                self.handle_fp_editor_set_next_pad_copper_offset_x(v)
            }
            PanelMsg::FpEditorSetNextPadCopperOffsetY(v) => {
                self.handle_fp_editor_set_next_pad_copper_offset_y(v)
            }
            PanelMsg::FpEditorToggleNextPadPlated(plated) => {
                self.handle_fp_editor_toggle_next_pad_plated(plated)
            }
            PanelMsg::FpEditorSetSelectedPadHoleTolerancePlus { idx, value } => self
                .with_selected_pad(*idx, |pad| {
                    pad.hole_tolerance_plus_mm = fp_parse_optional_mm(value)
                }),
            PanelMsg::FpEditorSetSelectedPadHoleToleranceMinus { idx, value } => self
                .with_selected_pad(*idx, |pad| {
                    pad.hole_tolerance_minus_mm = fp_parse_optional_mm(value)
                }),
            PanelMsg::FpEditorSetSelectedPadHoleRotation { idx, value } => self
                .with_selected_pad(*idx, |pad| {
                    pad.hole_rotation_deg = value.trim().parse::<f64>().ok()
                }),
            PanelMsg::FpEditorSetSelectedPadCopperOffsetX { idx, value } => self
                .with_selected_pad(*idx, |pad| {
                    pad.copper_offset_x_mm = fp_parse_optional_mm(value)
                }),
            PanelMsg::FpEditorSetSelectedPadCopperOffsetY { idx, value } => self
                .with_selected_pad(*idx, |pad| {
                    pad.copper_offset_y_mm = fp_parse_optional_mm(value)
                }),
            PanelMsg::FpEditorToggleSelectedPadPlated { idx, value } => {
                self.with_selected_pad(*idx, |pad| {
                    pad.kind = if *value {
                        signex_library::PadKind::Tht
                    } else {
                        signex_library::PadKind::NptHole
                    }
                })
            }
            // v0.21 — sketch-pad attribute mutations. Each routes
            // through `with_selected_sketch_pad` which mutates the
            // entity's `PadAttr` then triggers solve+bake so geometry
            // re-derives if the change affected dependent expressions.
            PanelMsg::FpEditorSetSketchPadElectricalType { id, value } => {
                self.with_selected_sketch_pad(*id, |attr| attr.electrical_type = *value)
            }
            PanelMsg::FpEditorSetSketchPadNet { id, value } => {
                self.with_selected_sketch_pad(*id, |attr| attr.net = value.clone())
            }
            PanelMsg::FpEditorToggleSketchPadLocked { id, value } => {
                self.with_selected_sketch_pad(*id, |attr| attr.locked = *value)
            }
            PanelMsg::FpEditorSetSketchPadTemplate { id, value } => {
                self.with_selected_sketch_pad(*id, |attr| attr.template = value.clone())
            }
            PanelMsg::FpEditorSetSketchPadTemplateLibrary { id, value } => {
                self.with_selected_sketch_pad(*id, |attr| attr.library = value.clone())
            }
            PanelMsg::FpEditorSetSketchPadFeatureTop { id, value } => {
                self.with_selected_sketch_pad(*id, |attr| attr.feature_top = *value)
            }
            PanelMsg::FpEditorSetSketchPadFeatureBottom { id, value } => {
                self.with_selected_sketch_pad(*id, |attr| attr.feature_bottom = *value)
            }
            PanelMsg::FpEditorToggleSketchPadTestpointTopAssembly { id, value } => {
                self.with_selected_sketch_pad(*id, |attr| attr.testpoint.top_assembly = *value)
            }
            PanelMsg::FpEditorToggleSketchPadTestpointTopFab { id, value } => {
                self.with_selected_sketch_pad(*id, |attr| attr.testpoint.top_fab = *value)
            }
            PanelMsg::FpEditorToggleSketchPadTestpointBottomAssembly { id, value } => {
                self.with_selected_sketch_pad(*id, |attr| attr.testpoint.bottom_assembly = *value)
            }
            PanelMsg::FpEditorToggleSketchPadTestpointBottomFab { id, value } => {
                self.with_selected_sketch_pad(*id, |attr| attr.testpoint.bottom_fab = *value)
            }
            PanelMsg::FpEditorToggleSketchPadThermalRelief { id, value } => {
                self.with_selected_sketch_pad(*id, |attr| attr.stack.thermal_relief = *value)
            }
            PanelMsg::FpEditorToggleSketchPadMaskTentedTop { id, value } => {
                self.with_selected_sketch_pad(*id, |attr| attr.stack.mask_top_tented = *value)
            }
            PanelMsg::FpEditorToggleSketchPadMaskTentedBottom { id, value } => {
                self.with_selected_sketch_pad(*id, |attr| attr.stack.mask_bottom_tented = *value)
            }
            PanelMsg::FpEditorToggleSketchPadPasteEnabledTop { id, value } => {
                self.with_selected_sketch_pad(*id, |attr| attr.stack.paste_top_enabled = *value)
            }
            PanelMsg::FpEditorToggleSketchPadPasteEnabledBottom { id, value } => {
                self.with_selected_sketch_pad(*id, |attr| attr.stack.paste_bottom_enabled = *value)
            }
            PanelMsg::FpEditorSetSketchPadHoleTolerancePlus { id, value } => self
                .with_selected_sketch_pad(*id, |attr| {
                    attr.hole_tolerance_plus_mm = fp_parse_optional_mm(value)
                }),
            PanelMsg::FpEditorSetSketchPadHoleToleranceMinus { id, value } => self
                .with_selected_sketch_pad(*id, |attr| {
                    attr.hole_tolerance_minus_mm = fp_parse_optional_mm(value)
                }),
            PanelMsg::FpEditorSetSketchPadHoleRotation { id, value } => self
                .with_selected_sketch_pad(*id, |attr| {
                    attr.hole_rotation_deg = value.trim().parse::<f64>().ok()
                }),
            PanelMsg::FpEditorSetSketchPadCopperOffsetX { id, value } => self
                .with_selected_sketch_pad(*id, |attr| {
                    attr.copper_offset_x_mm = fp_parse_optional_mm(value)
                }),
            PanelMsg::FpEditorSetSketchPadCopperOffsetY { id, value } => self
                .with_selected_sketch_pad(*id, |attr| {
                    attr.copper_offset_y_mm = fp_parse_optional_mm(value)
                }),
            PanelMsg::FpEditorSetSketchPadCornerRadiusPct { id, value } => self
                .with_selected_sketch_pad(*id, |attr| {
                    attr.stack.corner_radius_pct = value
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|v| (0.0..=50.0).contains(v))
                }),
            PanelMsg::FpEditorEditPadInSketch { pad_idx } => {
                self.handle_fp_editor_edit_pad_in_sketch(pad_idx)
            }
            PanelMsg::FpEditorEditPadShapeParam {
                pad_idx,
                key,
                value,
            } => {
                follow = self.handle_fp_editor_edit_pad_shape_param(pad_idx, key, value);
                true
            }
            PanelMsg::FpEditorUnlinkCornerRadius { arc_entity_id } => {
                follow = self.handle_fp_editor_unlink_corner_radius(arc_entity_id);
                true
            }
            PanelMsg::FpEditorEditSketchPadInPads { id } => {
                self.handle_fp_editor_edit_sketch_pad_in_pads(id)
            }
            PanelMsg::FpEditorSelectSketchEntity { id } => {
                self.handle_fp_editor_select_sketch_entity(id)
            }
            PanelMsg::HistoryRestoreClicked { sha } => {
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
            PanelMsg::FpEditorHoverOverConstraint { constraint } => {
                self.handle_fp_editor_hover_over_constraint(constraint)
            }
            PanelMsg::FpEditorToggleSilkFilled(on) => self.handle_fp_editor_toggle_silk_filled(on),
            PanelMsg::FpEditorSetPourNet { id, value } => {
                self.fp_editor_set_pour_net(*id, value.clone())
            }
            PanelMsg::FpEditorSetPourFillType { id, value } => {
                self.fp_editor_set_pour_fill_type(*id, *value)
            }
            PanelMsg::FpEditorSetPourPriority { id, value } => {
                self.fp_editor_set_pour_priority(*id, value.clone())
            }
            PanelMsg::FpEditorSetKeepoutKind { id, kind, value } => {
                self.fp_editor_set_keepout_kind(*id, *kind, *value)
            }
            PanelMsg::FpEditorSetCutoutEdgeRadius { id, value } => {
                self.fp_editor_set_cutout_edge_radius(*id, value.clone())
            }
            PanelMsg::FpEditorSetCutoutThrough { id, value } => {
                self.fp_editor_set_cutout_through(*id, *value)
            }
            PanelMsg::FpEditorEditArrayParam {
                array_id,
                field,
                value,
            } => self.fp_editor_edit_array_param(*array_id, *field, value.clone()),
            PanelMsg::FpEditorSetArrayNumberingScheme { array_id, scheme } => {
                self.fp_editor_set_array_numbering_scheme(*array_id, *scheme)
            }
            PanelMsg::FpEditorSetBgaSkipLetters { array_id, value } => {
                self.fp_editor_set_bga_skip_letters(*array_id, *value)
            }
            PanelMsg::FpEditorSetBgaStartRow { array_id, value } => {
                self.fp_editor_set_bga_start_row(*array_id, value.clone())
            }
            PanelMsg::FpEditorSetBgaStartCol { array_id, value } => {
                self.fp_editor_set_bga_start_col(*array_id, value.clone())
            }
            PanelMsg::FpEditorDeleteArray { array_id } => self.fp_editor_delete_array(*array_id),
            PanelMsg::FpEditorBeginRepickPolarCenter { array_id } => {
                self.fp_editor_begin_repick_polar_center(*array_id)
            }
            PanelMsg::FpEditorToggleArrayInstance {
                array_id,
                i,
                j,
                value,
            } => self.fp_editor_toggle_array_instance(*array_id, *i, *j, *value),
            PanelMsg::FpEditorToggleSnapOption(flag) => self.fp_editor_toggle_snap_option(*flag),
            PanelMsg::FpEditorSetSnapGridStep(value) => self.fp_editor_set_snap_grid_step(value),
            PanelMsg::FpEditorSetSnapDistance(value) => {
                self.handle_fp_set_snap_distance(value.clone())
            }
            PanelMsg::FpEditorSetAxisSnapRange(value) => {
                self.handle_fp_set_axis_snap_range(value.clone())
            }
            PanelMsg::FpEditorSetFootprintName(name) => {
                self.handle_fp_editor_set_footprint_name(name)
            }
            PanelMsg::FpEditorToggleSelectionFilter(kind) => {
                self.handle_fp_editor_toggle_selection_filter(kind)
            }
            PanelMsg::FpEditorOpenSelectionFilterCustom => {
                follow = self.update(Message::SelectionFilter(SelectionFilterMsg::OpenCustom));
                true
            }
            PanelMsg::FpEditorSetSnapSubTab(tab) => self.handle_fp_editor_set_snap_subtab(tab),
            PanelMsg::FpEditorSetSnappingMode(mode) => {
                self.handle_fp_editor_set_snapping_mode(mode)
            }
            PanelMsg::FpEditorGridManagerAdd => self.handle_fp_editor_grid_manager_add(),
            PanelMsg::FpEditorGridManagerProperties => {
                follow = self.handle_fp_editor_grid_manager_properties();
                true
            }
            PanelMsg::FpEditorGridManagerDelete => self.handle_fp_editor_grid_manager_delete(),
            PanelMsg::FpEditorGridSetActive(idx) => self.handle_fp_editor_grid_set_active(idx),
            PanelMsg::FpEditorGuideManagerAdd => self.handle_fp_editor_guide_manager_add(),
            PanelMsg::FpEditorGuideAddVertical => self.handle_fp_editor_guide_add_vertical(),
            PanelMsg::FpEditorGuideAddHorizontal => self.handle_fp_editor_guide_add_horizontal(),
            PanelMsg::FpEditorGuideDelete(idx) => self.handle_fp_editor_guide_delete(idx),
            PanelMsg::FpEditorGuideToggle(idx) => self.handle_fp_editor_guide_toggle(idx),
            PanelMsg::FpEditorGuideSetPosition(idx, raw) => {
                self.handle_fp_editor_guide_set_position(idx, raw)
            }
            PanelMsg::FpEditorSetSilkText(value) => self.handle_fp_editor_set_silk_text(value),
            PanelMsg::FpEditorDeleteSelectedSilk => self.handle_fp_editor_delete_selected_silk(),
            PanelMsg::FpEditorEditParameter { name, expr } => {
                follow = self.handle_fp_editor_edit_parameter(name, expr);
                true
            }
            PanelMsg::SchLibrarySelectSymbol(idx) => self.sch_library_select_symbol(*idx),
            PanelMsg::SchLibraryAddSymbol => self.sch_library_add_symbol(),
            PanelMsg::SchLibraryDeleteSymbol(idx) => self.sch_library_delete_symbol(*idx),
            PanelMsg::SymEditorSetPinNumber { pin_idx, value } => {
                self.sym_editor_set_pin_number(*pin_idx, value.clone())
            }
            PanelMsg::SymEditorSetPinName { pin_idx, value } => {
                self.sym_editor_set_pin_name(*pin_idx, value.clone())
            }
            PanelMsg::SymEditorSetPinLength { pin_idx, value } => {
                self.sym_editor_set_pin_length(*pin_idx, *value)
            }
            PanelMsg::SymEditorSetSymbolName(value) => {
                self.sym_editor_set_symbol_name(value.clone())
            }
            PanelMsg::SymEditorSetPinElectrical { pin_idx, value } => {
                self.sym_editor_set_pin_electrical(*pin_idx, *value)
            }
            PanelMsg::SymEditorSetPinOrientation { pin_idx, value } => {
                self.sym_editor_set_pin_orientation(*pin_idx, *value)
            }
            PanelMsg::SymEditorSetPinX { pin_idx, value } => {
                self.sym_editor_set_pin_x(*pin_idx, *value)
            }
            PanelMsg::SymEditorSetPinY { pin_idx, value } => {
                self.sym_editor_set_pin_y(*pin_idx, *value)
            }
            PanelMsg::SymEditorSelectPin(idx) => self.sym_editor_select_pin(*idx),
            PanelMsg::SymEditorSetPinDescription { pin_idx, value } => {
                self.sym_editor_mutate_pin(*pin_idx, |pin| pin.description = value.clone())
            }
            PanelMsg::SymEditorSetPinFunctionCsv { pin_idx, value } => {
                let parsed: Vec<String> = value
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect();
                self.sym_editor_mutate_pin(*pin_idx, move |pin| {
                    pin.function = parsed.clone();
                })
            }
            PanelMsg::SymEditorTogglePinDesignatorVisible(pin_idx) => {
                self.sym_editor_mutate_pin(*pin_idx, |pin| {
                    pin.designator_visible = !pin.designator_visible;
                })
            }
            PanelMsg::SymEditorTogglePinNameVisible(pin_idx) => {
                self.sym_editor_mutate_pin(*pin_idx, |pin| {
                    pin.name_visible = !pin.name_visible;
                })
            }
            PanelMsg::SymEditorTogglePinHidden(pin_idx) => {
                self.sym_editor_mutate_pin(*pin_idx, |pin| pin.hidden = !pin.hidden)
            }
            PanelMsg::SymEditorTogglePinLocked(pin_idx) => {
                self.sym_editor_mutate_pin(*pin_idx, |pin| pin.locked = !pin.locked)
            }
            PanelMsg::SymEditorSetPinSymbol {
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
                })
            }
            PanelMsg::SymEditorSetPinPartNumber { pin_idx, value } => {
                self.sym_editor_mutate_pin(*pin_idx, |pin| pin.part_number = *value)
            }
            PanelMsg::SymEditorSelectGraphic(idx) => self.sym_editor_select_graphic(*idx),
            PanelMsg::SymEditorSelectPart(part) => self.sym_editor_select_part(*part),
            PanelMsg::SymEditorSetGraphicField { idx, field, value } => {
                self.sym_editor_mutate_graphic(*idx, |g| apply_graphic_field(g, *field, *value))
            }
            PanelMsg::SymEditorSetGraphicText { idx, value } => {
                self.sym_editor_mutate_graphic(*idx, |g| {
                    if let signex_library::SymbolGraphicKind::Text { content, .. } = &mut g.kind {
                        *content = value.clone();
                    }
                })
            }
            PanelMsg::SymEditorSetSymbolDesignator(value) => {
                self.sym_editor_mutate_symbol(|s| s.designator = value.clone())
            }
            PanelMsg::SymEditorSetSymbolComment(value) => {
                self.sym_editor_mutate_symbol(|s| s.comment = value.clone())
            }
            PanelMsg::SymEditorSetSymbolDescription(value) => {
                self.sym_editor_mutate_symbol(|s| s.description = value.clone())
            }
            PanelMsg::SymEditorSetSymbolType(value) => {
                self.sym_editor_mutate_symbol(|s| s.component_type = *value)
            }
            PanelMsg::SymEditorToggleSymbolMirrored => {
                self.sym_editor_mutate_symbol(|s| s.mirrored = !s.mirrored)
            }
            PanelMsg::SymEditorCycleLocalFillColor => self.sym_editor_mutate_symbol(|s| {
                s.local_fill_color = cycle_local_color(s.local_fill_color);
            }),
            PanelMsg::SymEditorCycleLocalLineColor => self.sym_editor_mutate_symbol(|s| {
                s.local_line_color = cycle_local_color(s.local_line_color);
            }),
            PanelMsg::SymEditorCycleLocalPinColor => self.sym_editor_mutate_symbol(|s| {
                s.local_pin_color = cycle_local_color(s.local_pin_color);
            }),
            PanelMsg::SymEditorSetDisplaySheetColor(color) => {
                self.sym_editor_mutate_display(|d| d.sheet_color = *color)
            }
            PanelMsg::SymEditorToggleDisplayGrid => {
                self.sym_editor_mutate_display(|d| d.grid_visible = !d.grid_visible)
            }
            PanelMsg::SymEditorCycleDisplayGridSize => self.sym_editor_mutate_display(|d| {
                let sizes = crate::canvas::grid::GRID_SIZES_MM;
                let i = sizes
                    .iter()
                    .position(|s| (s - d.grid_size_mm).abs() < f32::EPSILON)
                    .unwrap_or(2);
                d.grid_size_mm = sizes[(i + 1) % sizes.len()];
            }),
            PanelMsg::SymEditorCycleDisplayUnit => self.sym_editor_mutate_display(|d| {
                use signex_types::coord::Unit;
                d.unit = match d.unit {
                    Unit::Mm => Unit::Mil,
                    Unit::Mil => Unit::Inch,
                    Unit::Inch => Unit::Micrometer,
                    Unit::Micrometer => Unit::Mm,
                };
            }),
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
