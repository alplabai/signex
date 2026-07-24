//! Pad-form types, message-helper macro, widget primitives, and the
//! three Properties-panel render functions for Pads-mode (Properties
//! / Pad Stack / Pad Features).
//!
//! Cross-module surface:
//! - `PadEditTarget` / `PadFormValues` are exported `pub(super)` so
//!   `pad_table`, `pad_stack_preview`, and the role sub-forms can
//!   share the same form snapshot + edit-routing semantics.
//! - The `pad_msg_fns!` macro stamps out ~30 near-identical message-
//!   builder helpers; without it each builder cost ~7 lines of
//!   boilerplate. The macro is declared file-local but the emitted
//!   `pub(super) fn` are reachable from `pad_table`.
//! - `pad_input_row` / `pad_pick_row` / `pad_check_row` are the row-
//!   chrome primitives; `subforms` and `pad_table` import them.

use iced::widget::{Column, container, pick_list, row, text, text_input};
use iced::{Color, Length, Theme};

use super::super::super::{
    CollapsedSections, FootprintEditorPanelContext, FootprintPadSummary, PanelMsg,
};
use super::super::{fp_is_collapsed, props_section_header};
use super::stack_preview::{
    ExpansionMode, HoleShapeChoice, PadShapeChoice, pad_stack_preview, pad_stack_tab_strip,
};
use super::table::{
    pad_copper_row, pad_table_check_cell, pad_table_disabled_cell, pad_table_header,
    pad_table_input_cell, pad_table_picklist_cell, pad_table_row, pad_table_static_cell,
};

#[derive(Debug, Clone, Copy)]
pub(in crate::panels::footprint_editor_properties) enum PadEditTarget {
    Next,
    Selected(usize),
}

/// v0.20 — snapshot of the values the form renders. Built from
/// `FootprintEditorPanelContext.next_pad_*` for placement, or from
/// `FootprintPadSummary` for the selected-pad branch.
#[derive(Debug, Clone)]
pub(in crate::panels::footprint_editor_properties) struct PadFormValues {
    pub(super) designator: String,
    pub(super) side: crate::library::editor::footprint::state::PadSide,
    pub(super) rotation_deg: f64,
    pub(super) template: String,
    pub(super) template_library: String,
    pub(super) shape: signex_library::PadShape,
    pub(super) kind: signex_library::PadKind,
    pub(super) size_x_mm: f64,
    pub(super) size_y_mm: f64,
    pub(super) drill_diameter_mm: Option<f64>,
    pub(super) drill_slot_length_mm: Option<f64>,
    pub(super) stack: crate::library::editor::footprint::state::PadStackUi,
    pub(super) feature_top: signex_sketch::attr::PadFeature,
    pub(super) feature_bottom: signex_sketch::attr::PadFeature,
    pub(super) testpoint: signex_sketch::attr::TestpointFlags,
    /// Active Pad Stack tab. Drives the preview + which body the
    /// Pad Stack section renders.
    pub(super) pad_stack_tab: crate::library::editor::footprint::state::PadStackTab,
    /// v0.21 — Altium-parity electrical-type / net / locked.
    pub(super) electrical_type: signex_sketch::attr::ElectricalType,
    pub(super) net: String,
    pub(super) locked: bool,
    /// v0.21 — Pad Hole detail fields (Multi-Layer only).
    pub(super) hole_tolerance_plus_mm: Option<f64>,
    pub(super) hole_tolerance_minus_mm: Option<f64>,
    pub(super) hole_rotation_deg: Option<f64>,
    pub(super) copper_offset_x_mm: Option<f64>,
    pub(super) copper_offset_y_mm: Option<f64>,
    /// v0.25 polish — verbatim per-input buffers shared across the
    /// pad-properties form. Renderer reads `numeric_buffers.get(key)`
    /// to display the user''s in-flight typed text rather than
    /// reformatting the canonical f64 every keystroke. Carrying
    /// the whole map (rather than per-field Option<String>) keeps
    /// the call sites tight as more fields adopt the buffer
    /// pattern.
    pub(super) numeric_buffers: std::collections::HashMap<String, String>,
}

impl PadFormValues {
    pub(in crate::panels::footprint_editor_properties) fn from_next_pad(
        fp: &FootprintEditorPanelContext,
    ) -> Self {
        Self {
            designator: fp.next_pad_designator_override.clone().unwrap_or_default(),
            side: fp.next_pad_side,
            rotation_deg: fp.next_pad_rotation_deg,
            template: fp.next_pad_template.clone(),
            template_library: fp.next_pad_template_library.clone(),
            shape: fp.next_pad_shape.clone(),
            kind: fp.next_pad_kind,
            size_x_mm: fp.next_pad_size_x_mm,
            size_y_mm: fp.next_pad_size_y_mm,
            drill_diameter_mm: fp.next_pad_drill_diameter_mm,
            drill_slot_length_mm: fp.next_pad_drill_slot_length_mm,
            stack: fp.next_pad_stack.clone(),
            feature_top: fp.next_pad_feature_top,
            feature_bottom: fp.next_pad_feature_bottom,
            testpoint: fp.next_pad_testpoint,
            pad_stack_tab: fp.pad_stack_tab,
            electrical_type: fp.next_pad_electrical_type,
            net: fp.next_pad_net.clone(),
            locked: fp.next_pad_locked,
            hole_tolerance_plus_mm: fp.next_pad_hole_tolerance_plus_mm,
            hole_tolerance_minus_mm: fp.next_pad_hole_tolerance_minus_mm,
            hole_rotation_deg: fp.next_pad_hole_rotation_deg,
            copper_offset_x_mm: fp.next_pad_copper_offset_x_mm,
            copper_offset_y_mm: fp.next_pad_copper_offset_y_mm,
            numeric_buffers: fp.numeric_buffers.clone(),
        }
    }
    pub(in crate::panels::footprint_editor_properties) fn from_selected_pad(
        pad: &FootprintPadSummary,
        fp: &FootprintEditorPanelContext,
    ) -> Self {
        Self {
            designator: pad.number.clone(),
            side: pad.side,
            rotation_deg: pad.rotation_deg,
            template: pad.template.clone(),
            template_library: pad.template_library.clone(),
            shape: pad.shape.clone(),
            kind: pad.kind,
            size_x_mm: pad.size_mm[0],
            // v0.21 — selected-pad hole-detail values pulled from
            // FootprintPadSummary (populated in runtime.rs).
            // (Note: the Default::default()-set fields below are
            // overridden after the struct literal — this comment
            // is intentional anchor for v0.22 cleanup.)
            size_y_mm: pad.size_mm[1],
            drill_diameter_mm: pad.drill_diameter_mm,
            drill_slot_length_mm: None, // selected-pad slot_length follow-up in v0.21
            stack: pad.stack.clone(),
            feature_top: pad.feature_top,
            feature_bottom: pad.feature_bottom,
            testpoint: pad.testpoint,
            pad_stack_tab: fp.pad_stack_tab,
            electrical_type: pad.electrical_type,
            net: pad.net.clone(),
            locked: pad.locked,
            hole_tolerance_plus_mm: pad.hole_tolerance_plus_mm,
            hole_tolerance_minus_mm: pad.hole_tolerance_minus_mm,
            hole_rotation_deg: pad.hole_rotation_deg,
            copper_offset_x_mm: pad.copper_offset_x_mm,
            copper_offset_y_mm: pad.copper_offset_y_mm,
            numeric_buffers: fp.numeric_buffers.clone(),
        }
    }
}

// v0.20 — message constructor helpers. Each switches on PadEditTarget
// to emit the matching FpEditorSetNextPad*/FpEditorSetSelectedPad*
// variant. The form's row builders consume these via a closure
// captured by `move |v| pad_*_msg(target, v)` so the same render
// functions service both targets.
//
// Mass-defined via `pad_msg_fns!` to kill the boilerplate that used to
// expand to 30+ near-identical 7-line functions. The macro takes a
// list of (fn_name, value_type, Next-variant, Selected-variant,
// value-field-name) tuples and stamps out the matching helper for
// each. Special enum-tagged messages (`Side`, `Shape`) use a
// non-`value` field name; the macro handles that uniformly.
macro_rules! pad_msg_fns {
    ($(($fn_name:ident, $ty:ty, $next:ident, $sel:ident, $field:ident);)*) => {
        $(
            pub(in crate::panels::footprint_editor_properties) fn $fn_name(t: PadEditTarget, v: $ty) -> PanelMsg {
                match t {
                    PadEditTarget::Next => PanelMsg::$next(v),
                    PadEditTarget::Selected(idx) => PanelMsg::$sel { idx, $field: v },
                }
            }
        )*
    };
}

pad_msg_fns! {
    // String-valued fields.
    (pad_designator_msg, String, FpEditorSetNextPadDesignator, FpEditorSetSelectedPadDesignator, value);
    (pad_net_msg, String, FpEditorSetNextPadNet, FpEditorSetSelectedPadNet, value);
    (pad_rotation_msg, String, FpEditorSetNextPadRotation, FpEditorSetSelectedPadRotation, value);
    (pad_template_msg, String, FpEditorSetNextPadTemplate, FpEditorSetSelectedPadTemplate, value);
    (pad_template_library_msg, String, FpEditorSetNextPadTemplateLibrary, FpEditorSetSelectedPadTemplateLibrary, value);
    (pad_size_x_msg, String, FpEditorSetNextPadSizeX, FpEditorSetSelectedPadSizeX, value);
    (pad_size_y_msg, String, FpEditorSetNextPadSizeY, FpEditorSetSelectedPadSizeY, value);
    (pad_drill_diameter_msg, String, FpEditorSetNextPadDrillDiameter, FpEditorSetSelectedPadDrillDiameter, value);
    (pad_drill_slot_length_msg, String, FpEditorSetNextPadDrillSlotLength, FpEditorSetSelectedPadDrillSlotLength, value);
    (pad_corner_radius_msg, String, FpEditorSetNextPadCornerRadiusPct, FpEditorSetSelectedPadCornerRadiusPct, value);
    (pad_paste_top_msg, String, FpEditorSetNextPadPasteMarginTop, FpEditorSetSelectedPadPasteMarginTop, value);
    (pad_paste_bottom_msg, String, FpEditorSetNextPadPasteMarginBottom, FpEditorSetSelectedPadPasteMarginBottom, value);
    (pad_mask_top_msg, String, FpEditorSetNextPadMaskMarginTop, FpEditorSetSelectedPadMaskMarginTop, value);
    (pad_mask_bottom_msg, String, FpEditorSetNextPadMaskMarginBottom, FpEditorSetSelectedPadMaskMarginBottom, value);
    (pad_hole_tolerance_plus_msg, String, FpEditorSetNextPadHoleTolerancePlus, FpEditorSetSelectedPadHoleTolerancePlus, value);
    (pad_hole_tolerance_minus_msg, String, FpEditorSetNextPadHoleToleranceMinus, FpEditorSetSelectedPadHoleToleranceMinus, value);
    (pad_hole_rotation_msg, String, FpEditorSetNextPadHoleRotation, FpEditorSetSelectedPadHoleRotation, value);
    (pad_copper_offset_x_msg, String, FpEditorSetNextPadCopperOffsetX, FpEditorSetSelectedPadCopperOffsetX, value);
    (pad_copper_offset_y_msg, String, FpEditorSetNextPadCopperOffsetY, FpEditorSetSelectedPadCopperOffsetY, value);
    // Bool-valued fields (toggle variants).
    (pad_locked_msg, bool, FpEditorToggleNextPadLocked, FpEditorToggleSelectedPadLocked, value);
    (pad_plated_msg, bool, FpEditorToggleNextPadPlated, FpEditorToggleSelectedPadPlated, value);
    (pad_paste_enabled_top_msg, bool, FpEditorToggleNextPadPasteEnabledTop, FpEditorToggleSelectedPadPasteEnabledTop, value);
    (pad_paste_enabled_bottom_msg, bool, FpEditorToggleNextPadPasteEnabledBottom, FpEditorToggleSelectedPadPasteEnabledBottom, value);
    (pad_mask_tented_top_msg, bool, FpEditorToggleNextPadMaskTentedTop, FpEditorToggleSelectedPadMaskTentedTop, value);
    (pad_mask_tented_bottom_msg, bool, FpEditorToggleNextPadMaskTentedBottom, FpEditorToggleSelectedPadMaskTentedBottom, value);
    (pad_thermal_relief_msg, bool, FpEditorToggleNextPadThermalRelief, FpEditorToggleSelectedPadThermalRelief, value);
    (pad_testpoint_top_assembly_msg, bool, FpEditorToggleNextPadTestpointTopAssembly, FpEditorToggleSelectedPadTestpointTopAssembly, value);
    (pad_testpoint_top_fab_msg, bool, FpEditorToggleNextPadTestpointTopFab, FpEditorToggleSelectedPadTestpointTopFab, value);
    (pad_testpoint_bottom_assembly_msg, bool, FpEditorToggleNextPadTestpointBottomAssembly, FpEditorToggleSelectedPadTestpointBottomAssembly, value);
    (pad_testpoint_bottom_fab_msg, bool, FpEditorToggleNextPadTestpointBottomFab, FpEditorToggleSelectedPadTestpointBottomFab, value);
    // Enum-typed fields.
    (pad_electrical_type_msg, signex_sketch::attr::ElectricalType, FpEditorSetNextPadElectricalType, FpEditorSetSelectedPadElectricalType, value);
    (pad_feature_top_msg, signex_sketch::attr::PadFeature, FpEditorSetNextPadFeatureTop, FpEditorSetSelectedPadFeatureTop, value);
    (pad_feature_bottom_msg, signex_sketch::attr::PadFeature, FpEditorSetNextPadFeatureBottom, FpEditorSetSelectedPadFeatureBottom, value);
    // Special-cased field names — `side:` and `shape:` instead of `value:`.
    (pad_side_msg, crate::library::editor::footprint::state::PadSide, FpEditorSetNextPadSide, FpEditorSetSelectedPadSide, side);
    (pad_shape_msg, signex_library::PadShape, FpEditorSetNextPadShape, FpEditorSetSelectedPadShape, shape);
}

/// v0.20 — single-line label + text-input row used by every Pad
/// Properties field. Mirrors the existing rotation/size_x rows'
/// chrome (40 px label, padded input, dim border).
pub(in crate::panels::footprint_editor_properties) fn pad_input_row<'a>(
    label: &'a str,
    placeholder: &'a str,
    value: String,
    on_input: impl Fn(String) -> PanelMsg + 'a,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> iced::Element<'a, PanelMsg> {
    container(
        row![
            text(label)
                .size(10)
                .color(muted)
                .width(Length::Fixed(110.0)),
            text_input(placeholder, &value)
                .size(10)
                .padding(2)
                .style(move |_: &Theme, _| iced::widget::text_input::Style {
                    background: iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.04,
                    )),
                    border: iced::Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: border_c,
                    },
                    icon: iced::Color::TRANSPARENT,
                    placeholder: muted,
                    value: primary,
                    selection: iced::Color::from_rgba(0.4, 0.6, 1.0, 0.4),
                })
                .on_input(on_input),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, 8])
    .width(Length::Fill)
    .into()
}

/// v0.20 — pick_list row for a Pad Properties field.
pub(in crate::panels::footprint_editor_properties) fn pad_pick_row<'a, T>(
    label: &'a str,
    options: &'a [T],
    selected: T,
    on_change: impl Fn(T) -> PanelMsg + 'a + Clone,
    muted: Color,
) -> iced::Element<'a, PanelMsg>
where
    T: Clone + Eq + std::fmt::Display + 'static,
{
    container(
        row![
            text(label)
                .size(10)
                .color(muted)
                .width(Length::Fixed(110.0)),
            pick_list(options, Some(selected), on_change)
                .text_size(10)
                .padding([3, 8])
                .width(Length::Fill),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, 8])
    .width(Length::Fill)
    .into()
}

/// v0.20 — checkbox row for a Pad Properties field. Label on left,
/// flat checkbox on right.
pub(in crate::panels::footprint_editor_properties) fn pad_check_row<'a>(
    label: &'a str,
    on: bool,
    on_toggle: impl Fn(bool) -> PanelMsg + 'a,
    muted: Color,
    primary: Color,
) -> iced::Element<'a, PanelMsg> {
    container(
        row![
            text(label)
                .size(10)
                .color(if on { primary } else { muted })
                .width(Length::Fixed(110.0)),
            iced::widget::checkbox(on)
                .on_toggle(on_toggle)
                .size(12)
                .spacing(0),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, 8])
    .width(Length::Fill)
    .into()
}

/// v0.20 — render the "Properties" section. `target` selects whether
/// edits write to `next_pad_defaults` (placement form) or to a
/// specific selected pad. The pause/resume hint banner only shows
/// for the placement form (`PadEditTarget::Next`).
pub(in crate::panels::footprint_editor_properties) fn render_pad_form_properties<'a>(
    mut col: Column<'a, PanelMsg>,
    values: &PadFormValues,
    target: PadEditTarget,
    placement_paused: bool,
    muted: Color,
    primary: Color,
    border_c: Color,
    collapsed_sections: &'a CollapsedSections,
) -> Column<'a, PanelMsg> {
    col = col.push(props_section_header(
        "Properties",
        "fp_pad_properties",
        collapsed_sections,
        primary,
        border_c,
    ));
    if fp_is_collapsed("fp_pad_properties", collapsed_sections) {
        return col;
    }

    // TAB pause hint banner — only for the placement form.
    if matches!(target, PadEditTarget::Next) {
        if placement_paused {
            col = col.push(
                container(
                    text("PAUSED — TAB to resume placement")
                        .size(10)
                        .color(Color::from_rgba(1.0, 0.85, 0.30, 1.0)),
                )
                .padding([2, 8])
                .width(Length::Fill),
            );
        } else {
            col = col.push(
                container(
                    text("TAB to pause placement and edit defaults")
                        .size(10)
                        .color(muted),
                )
                .padding([2, 8])
                .width(Length::Fill),
            );
        }
    }

    col = col.push(pad_input_row(
        "Designator",
        "(auto)",
        values.designator.clone(),
        move |v| pad_designator_msg(target, v),
        muted,
        primary,
        border_c,
    ));

    use crate::library::editor::footprint::state::PadSide;
    col = col.push(pad_pick_row(
        "Layer",
        PadSide::ALL_OPTIONS,
        values.side,
        move |s: PadSide| pad_side_msg(target, s),
        muted,
    ));

    col = col.push(pad_pick_row(
        "Electrical Type",
        signex_sketch::attr::ElectricalType::ALL,
        values.electrical_type,
        move |v: signex_sketch::attr::ElectricalType| pad_electrical_type_msg(target, v),
        muted,
    ));

    col = col.push(pad_input_row(
        "Net",
        "(unassigned)",
        values.net.clone(),
        move |v| pad_net_msg(target, v),
        muted,
        primary,
        border_c,
    ));

    col = col.push(pad_input_row(
        "Rotation (°)",
        "0",
        format!("{:.1}", values.rotation_deg),
        move |v| pad_rotation_msg(target, v),
        muted,
        primary,
        border_c,
    ));

    col = col.push(pad_check_row(
        "Locked",
        values.locked,
        move |v| pad_locked_msg(target, v),
        muted,
        primary,
    ));

    col = col.push(pad_input_row(
        "Template",
        "",
        values.template.clone(),
        move |v| pad_template_msg(target, v),
        muted,
        primary,
        border_c,
    ));
    col = col.push(pad_input_row(
        "Library",
        "",
        values.template_library.clone(),
        move |v| pad_template_library_msg(target, v),
        muted,
        primary,
        border_c,
    ));

    col
}

/// v0.20 — render the "Pad Features" section: top/bottom surface
/// treatment + testpoint flags.
pub(in crate::panels::footprint_editor_properties) fn render_pad_form_pad_features<'a>(
    mut col: Column<'a, PanelMsg>,
    values: &PadFormValues,
    target: PadEditTarget,
    muted: Color,
    primary: Color,
    border_c: Color,
    collapsed_sections: &'a CollapsedSections,
) -> Column<'a, PanelMsg> {
    col = col.push(props_section_header(
        "Pad Features",
        "fp_pad_features",
        collapsed_sections,
        primary,
        border_c,
    ));
    if fp_is_collapsed("fp_pad_features", collapsed_sections) {
        return col;
    }

    use signex_sketch::attr::PadFeature;
    col = col.push(pad_pick_row(
        "Top Side",
        PadFeature::ALL,
        values.feature_top,
        move |v: PadFeature| pad_feature_top_msg(target, v),
        muted,
    ));
    col = col.push(pad_pick_row(
        "Bottom Side",
        PadFeature::ALL,
        values.feature_bottom,
        move |v: PadFeature| pad_feature_bottom_msg(target, v),
        muted,
    ));

    col = col.push(
        container(text("Testpoint").size(9).color(muted))
            .padding([8, 8])
            .width(Length::Fill),
    );
    col = col.push(pad_check_row(
        "Top Assembly",
        values.testpoint.top_assembly,
        move |v| pad_testpoint_top_assembly_msg(target, v),
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        "Top Fab",
        values.testpoint.top_fab,
        move |v| pad_testpoint_top_fab_msg(target, v),
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        "Bottom Assembly",
        values.testpoint.bottom_assembly,
        move |v| pad_testpoint_bottom_assembly_msg(target, v),
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        "Bottom Fab",
        values.testpoint.bottom_fab,
        move |v| pad_testpoint_bottom_fab_msg(target, v),
        muted,
        primary,
    ));
    let _ = border_c;
    col
}

mod stack;
pub(in crate::panels::footprint_editor_properties) use stack::render_pad_form_pad_stack;
