//! Sketch-mode Pad Attributes sub-form. Split from `subforms.rs`.

use iced::widget::{Column, container};
use iced::{Color, Element, Length, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_widgets::{raw, text};

use super::super::super::{CollapsedSections, FootprintEditorPanelContext, PanelMsg};
use super::super::pad::{pad_check_row, pad_input_row, pad_pick_row};
use crate::panels::{collapsible_section_header, is_section_collapsed};
use crate::styles::{Spacing, TypeScale};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// v0.21 — Sketch-mode Pad Attributes sub-form. Renders when the
/// selected sketch entity carries a `PadAttr`. Mirrors the
/// Altium-parity Pad Properties / Pad Stack / Pad Features fields
/// surfaced for Pads-mode placement, but bound to the sketch
/// entity's PadAttr rather than the flat-pad list. Geometry-shaping
/// fields (size_x_expr / size_y_expr / mask_margin_expr / etc.)
/// stay sketch-parameterised — those are authored via the Sketch
/// parameter editor, not this form.
pub(in crate::panels::footprint_editor_properties) fn render_sketch_pad_subform<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    fp: &'a FootprintEditorPanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
    collapsed_sections: &'a CollapsedSections,
    i18n: &I18n,
) -> Column<Element<'a, PanelMsg>> {
    let Some(p) = fp.selected_sketch_pad.as_ref() else {
        return col;
    };
    let id = p.id;

    col = col.push(
        collapsible_section_header(
            "fp_sketch_pad_attrs",
            "Pad Attributes",
            collapsed_sections,
            primary,
            border_c,
        )
        .boxed(),
    );
    if is_section_collapsed("fp_sketch_pad_attrs", collapsed_sections) {
        return col;
    }

    // Properties group.
    col = col.push(pad_pick_row(
        t!(i18n, ids::EDITORS_PAD_ELECTRICAL_TYPE),
        signex_sketch::attr::ElectricalType::ALL,
        p.electrical_type,
        move |v: signex_sketch::attr::ElectricalType| {
            PanelMsg::FpEditorSetSketchPadElectricalType { id, value: v }
        },
        muted,
    ));
    col = col.push(pad_input_row(
        t!(i18n, ids::EDITORS_PAD_NET),
        "(unassigned)",
        p.net.clone(),
        move |v| PanelMsg::FpEditorSetSketchPadNet { id, value: v },
        muted,
    ));
    col = col.push(pad_check_row(
        t!(i18n, ids::EDITORS_PAD_LOCKED),
        p.locked,
        move |v| PanelMsg::FpEditorToggleSketchPadLocked { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_input_row(
        t!(i18n, ids::EDITORS_PAD_TEMPLATE),
        "",
        p.template.clone(),
        move |v| PanelMsg::FpEditorSetSketchPadTemplate { id, value: v },
        muted,
    ));
    col = col.push(pad_input_row(
        t!(i18n, ids::EDITORS_PAD_TEMPLATE_LIBRARY),
        "",
        p.template_library.clone(),
        move |v| PanelMsg::FpEditorSetSketchPadTemplateLibrary { id, value: v },
        muted,
    ));

    // Pad Features.
    col = col.push(
        container(text(raw("Pad Features")).size(TS.caption).color(muted))
            .padding([6.0, SP.m])
            .width(Length::Fill)
            .boxed(),
    );
    col = col.push(pad_pick_row(
        t!(i18n, ids::EDITORS_PAD_TOP_SIDE),
        signex_sketch::attr::PadFeature::ALL,
        p.feature_top,
        move |v: signex_sketch::attr::PadFeature| PanelMsg::FpEditorSetSketchPadFeatureTop {
            id,
            value: v,
        },
        muted,
    ));
    col = col.push(pad_pick_row(
        t!(i18n, ids::EDITORS_PAD_BOTTOM_SIDE),
        signex_sketch::attr::PadFeature::ALL,
        p.feature_bottom,
        move |v: signex_sketch::attr::PadFeature| PanelMsg::FpEditorSetSketchPadFeatureBottom {
            id,
            value: v,
        },
        muted,
    ));

    // Testpoint flags.
    col = col.push(
        container(text(raw("Testpoint")).size(TS.caption).color(muted))
            .padding([6.0, SP.m])
            .width(Length::Fill)
            .boxed(),
    );
    col = col.push(pad_check_row(
        t!(i18n, ids::EDITORS_PAD_TOP_ASSEMBLY),
        p.testpoint.top_assembly,
        move |v| PanelMsg::FpEditorToggleSketchPadTestpointTopAssembly { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        t!(i18n, ids::EDITORS_PAD_TOP_FAB),
        p.testpoint.top_fab,
        move |v| PanelMsg::FpEditorToggleSketchPadTestpointTopFab { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        t!(i18n, ids::EDITORS_PAD_BOTTOM_ASSEMBLY),
        p.testpoint.bottom_assembly,
        move |v| PanelMsg::FpEditorToggleSketchPadTestpointBottomAssembly { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        t!(i18n, ids::EDITORS_PAD_BOTTOM_FAB),
        p.testpoint.bottom_fab,
        move |v| PanelMsg::FpEditorToggleSketchPadTestpointBottomFab { id, value: v },
        muted,
        primary,
    ));

    // Pad Stack overrides (the bool-typed subset; expression-typed
    // mask/paste margins stay sketch-parameterised).
    col = col.push(
        container(text(raw("Pad Stack")).size(TS.caption).color(muted))
            .padding([6.0, SP.m])
            .width(Length::Fill)
            .boxed(),
    );
    col = col.push(pad_check_row(
        t!(i18n, ids::EDITORS_PAD_TOP_PASTE_ENABLED),
        p.paste_top_enabled,
        move |v| PanelMsg::FpEditorToggleSketchPadPasteEnabledTop { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        t!(i18n, ids::EDITORS_PAD_BOTTOM_PASTE_ENABLED),
        p.paste_bottom_enabled,
        move |v| PanelMsg::FpEditorToggleSketchPadPasteEnabledBottom { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        t!(i18n, ids::EDITORS_PAD_TOP_MASK_TENTED),
        p.mask_top_tented,
        move |v| PanelMsg::FpEditorToggleSketchPadMaskTentedTop { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        t!(i18n, ids::EDITORS_PAD_BOTTOM_MASK_TENTED),
        p.mask_bottom_tented,
        move |v| PanelMsg::FpEditorToggleSketchPadMaskTentedBottom { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        t!(i18n, ids::EDITORS_PAD_THERMAL_RELIEF),
        p.thermal_relief,
        move |v| PanelMsg::FpEditorToggleSketchPadThermalRelief { id, value: v },
        muted,
        primary,
    ));
    let corner_buf = p
        .corner_radius_pct
        .map(|v| format!("{v:.0}"))
        .unwrap_or_default();
    col = col.push(pad_input_row(
        t!(i18n, ids::EDITORS_PAD_CORNER_RADIUS),
        "25",
        corner_buf,
        move |v| PanelMsg::FpEditorSetSketchPadCornerRadiusPct { id, value: v },
        muted,
    ));

    // Hole Details (THT pads only).
    if p.has_drill {
        col = col.push(
            container(text(raw("Hole Details")).size(TS.caption).color(muted))
                .padding([6.0, SP.m])
                .width(Length::Fill)
                .boxed(),
        );
        let tol_p_buf = p
            .hole_tolerance_plus_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            t!(i18n, ids::EDITORS_PAD_HOLE_TOLERANCE_PLUS),
            "0",
            tol_p_buf,
            move |v| PanelMsg::FpEditorSetSketchPadHoleTolerancePlus { id, value: v },
            muted,
        ));
        let tol_m_buf = p
            .hole_tolerance_minus_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            t!(i18n, ids::EDITORS_PAD_HOLE_TOLERANCE_MINUS),
            "0",
            tol_m_buf,
            move |v| PanelMsg::FpEditorSetSketchPadHoleToleranceMinus { id, value: v },
            muted,
        ));
        let rot_buf = p
            .hole_rotation_deg
            .map(|v| format!("{v:.1}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            t!(i18n, ids::EDITORS_PAD_HOLE_ROTATION),
            "0",
            rot_buf,
            move |v| PanelMsg::FpEditorSetSketchPadHoleRotation { id, value: v },
            muted,
        ));
        let cox_buf = p
            .copper_offset_x_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            t!(i18n, ids::EDITORS_PAD_COPPER_OFFSET_X),
            "0",
            cox_buf,
            move |v| PanelMsg::FpEditorSetSketchPadCopperOffsetX { id, value: v },
            muted,
        ));
        let coy_buf = p
            .copper_offset_y_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            t!(i18n, ids::EDITORS_PAD_COPPER_OFFSET_Y),
            "0",
            coy_buf,
            move |v| PanelMsg::FpEditorSetSketchPadCopperOffsetY { id, value: v },
            muted,
        ));
    }

    // v0.22 Phase D6 — "Edit in Pads ▸" jump button. Mirror of the
    // v0.21 "Edit in Sketch ▸" button on the Pads-mode pad form. Flips
    // the editor to Pads mode and selects the pad whose
    // `sketch_entity_id` matches this entity. The dispatcher resolves
    // the EditorPad index from the entity ID.
    col = col.push(
        container(
            signex_widgets::button(text(raw("Edit in Pads ▸")).size(TS.caption).color(primary))
                .padding([SP.s, 10.0])
                .on_press(PanelMsg::FpEditorEditSketchPadInPads { id })
                .style(iced::widget::button::primary),
        )
        .padding([6.0, SP.m])
        .width(Length::Fill)
        .boxed(),
    );

    col
}
