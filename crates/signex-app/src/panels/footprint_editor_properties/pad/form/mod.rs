//! Pad-form types, message-helper macro, row primitives and the pad form
//! of the Properties panel (mockup row 125): the sections live one per file.
//!
//! Cross-module surface:
//! - `PadEditTarget` / `PadFormValues` are shared with the pad table so
//!   both read the same form snapshot and edit routing.
//! - The `pad_msg_fns!` macro stamps out the message-builder helpers.
//! - `pad_input_row_w` / `pad_pick_row_w` are the row chrome of the
//!   per-side stack rows.

use iced::widget::{Column, Widget as _, container, pick_list, row};
use iced::{Color, Element, Length};
use signex_i18n::{I18n, MessageId, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::property_illustration::{FieldId, IllustrationHover};
use signex_widgets::{Label, raw, text, text_input};

use super::super::super::{
    CollapsedSections, FootprintEditorPanelContext, FootprintPadSummary, PanelMsg, PanelPalette,
};
use crate::library::editor::footprint::{PadSide, pad_field_buffer_key};
use crate::panels::{collapsible_section_header, is_section_collapsed};
use crate::styles::{Spacing, TypeScale};

mod features;
mod hole;
mod paste_mask;
mod properties;
mod stack;
mod template;
mod view;

pub(in crate::panels::footprint_editor_properties) use view::view_pad_form;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

#[derive(Debug, Clone, Copy)]
pub(in crate::panels::footprint_editor_properties) enum PadEditTarget {
    Next,
    Selected(usize),
}

impl PadEditTarget {
    /// The selected pad's index; `None` for the next pad placed.
    pub(in crate::panels::footprint_editor_properties) fn selected_idx(self) -> Option<usize> {
        match self {
            PadEditTarget::Next => None,
            PadEditTarget::Selected(idx) => Some(idx),
        }
    }
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
    /// The selected pad's position, mm; `None` for the next pad placed.
    pub(super) position_mm: Option<[f64; 2]>,
    /// What the Template dropdown offers (mockup row 125).
    pub(super) template_choices: template::TemplateChoices,
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
            position_mm: None,
            template_choices: template::choices(
                &fp.pad_outlines,
                None,
                &fp.next_pad_template,
                &fp.next_pad_template_library,
            ),
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
            drill_slot_length_mm: pad.drill_slot_length_mm,
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
            position_mm: Some(pad.position_mm),
            template_choices: template::choices(
                &fp.pad_outlines,
                Some(pad.idx),
                &pad.template,
                &pad.template_library,
            ),
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
    (pad_trapezoid_delta_x_msg, String, FpEditorSetNextPadTrapezoidDeltaX, FpEditorSetSelectedPadTrapezoidDeltaX, value);
    (pad_trapezoid_delta_y_msg, String, FpEditorSetNextPadTrapezoidDeltaY, FpEditorSetSelectedPadTrapezoidDeltaY, value);
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
#[allow(clippy::too_many_arguments)]
pub(in crate::panels::footprint_editor_properties) fn pad_input_row_w<'a>(
    fw: (Length, Length),
    label: impl Into<Label<'a>>,
    placeholder: &'a str,
    value: String,
    on_input: impl Fn(String) -> PanelMsg + 'a,
    muted: Color,
) -> iced::Element<'a, PanelMsg> {
    container(
        row![
            text(label).size(TS.caption).color(muted).width(fw.0),
            text_input(raw(placeholder), value)
                .size(TS.caption)
                .width(fw.1)
                .padding(SP.xs)
                .style(crate::panels::form_look::text_input_style)
                .on_input(on_input),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// v0.20 — pick_list row for a Pad Properties field.
pub(in crate::panels::footprint_editor_properties) fn pad_pick_row_w<'a, T>(
    fw: (Length, Length),
    label: impl Into<Label<'a>>,
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
            text(label).size(TS.caption).color(muted).width(fw.0),
            pick_list(Some(selected), options, |t: &T| t.to_string())
                .graphite()
                .on_select(on_change)
                .padding([3.0, SP.m])
                .width(fw.1),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// What the pad is, for the form: what applies is shown, nothing else
/// (mockup row 125, "show only what applies"). Through-hole follows the Layer
/// picker (Multi-Layer gives the pad a drill); a non-plated hole is its
/// kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PadClass {
    /// One copper layer, no pad stack.
    Smd(PadSide),
    ThroughHole,
    /// A through-hole pad whose drill has a slot length.
    Slot,
    /// A non-plated hole: no copper, no paste.
    Npth,
}

impl PadClass {
    pub(super) fn of(values: &PadFormValues) -> Self {
        if matches!(values.kind, signex_library::PadKind::NptHole) {
            return PadClass::Npth;
        }
        match values.side {
            PadSide::All if values.drill_slot_length_mm.is_some() => PadClass::Slot,
            PadSide::All => PadClass::ThroughHole,
            side => PadClass::Smd(side),
        }
    }

    pub(super) fn has_hole(self) -> bool {
        !matches!(self, PadClass::Smd(_))
    }

    /// Copper on every layer, so a pad stack and a thermal relief.
    pub(super) fn has_copper_stack(self) -> bool {
        matches!(self, PadClass::ThroughHole | PadClass::Slot)
    }
}

/// Everything a pad form section reads, borrowed for one `view`.
#[derive(Clone, Copy)]
pub(super) struct Form<'f> {
    pub(super) values: &'f PadFormValues,
    pub(super) target: PadEditTarget,
    pub(super) class: PadClass,
    pub(super) palette: PanelPalette,
    pub(super) tokens: &'f ThemeTokens,
    pub(super) hover: &'f IllustrationHover,
    pub(super) i18n: &'f I18n,
    pub(super) collapsed: &'f CollapsedSections,
}

impl<'f> Form<'f> {
    pub(super) fn fw(&self) -> (Length, Length) {
        self.palette.field_widths()
    }

    /// The refused text of `field` (a `pad_field` name) on this target.
    pub(super) fn buffer(&self, field: &str) -> Option<&'f String> {
        let key = pad_field_buffer_key(self.target.selected_idx(), field);
        self.values.numeric_buffers.get(&key)
    }

    /// `content` linked to `field` in the illustration.
    pub(super) fn linked<'a>(
        &self,
        content: Element<'a, PanelMsg>,
        field: FieldId,
    ) -> Element<'a, PanelMsg> {
        super::super::illustration::linked_field(content, field, self.hover, self.tokens)
    }

    /// A collapsible section header titled `title`; `true` while open.
    pub(super) fn section<'a>(
        &self,
        col: Column<Element<'a, PanelMsg>>,
        key: &str,
        title: MessageId,
    ) -> (Column<Element<'a, PanelMsg>>, bool) {
        let title = t!(self.i18n, title).to_string();
        let header = collapsible_section_header(
            key,
            &title,
            self.collapsed,
            self.palette.primary,
            self.palette.border,
        );
        (
            col.push(header.boxed()),
            !is_section_collapsed(key, self.collapsed),
        )
    }
}
