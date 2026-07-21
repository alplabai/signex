//! Sketch-mode Pad Attributes sub-form. Split from `subforms.rs`.

use iced::widget::{Column, container, text};
use iced::{Color, Length};

use super::super::super::{CollapsedSections, FootprintEditorPanelContext, PanelMsg};
use super::super::pad::{pad_check_row, pad_input_row, pad_pick_row};
use super::super::{fp_is_collapsed, props_section_header};

/// v0.21 — Sketch-mode Pad Attributes sub-form. Renders when the
/// selected sketch entity carries a `PadAttr`. Mirrors the
/// Altium-parity Pad Properties / Pad Stack / Pad Features fields
/// surfaced for Pads-mode placement, but bound to the sketch
/// entity's PadAttr rather than the flat-pad list. Geometry-shaping
/// fields (size_x_expr / size_y_expr / mask_margin_expr / etc.)
/// stay sketch-parameterised — those are authored via the Sketch
/// parameter editor, not this form.
pub(in crate::panels::footprint_editor_properties) fn render_sketch_pad_subform<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
    collapsed_sections: &'a CollapsedSections,
) -> Column<'a, PanelMsg> {
    let Some(p) = fp.selected_sketch_pad.as_ref() else {
        return col;
    };
    let id = p.id;

    col = col.push(props_section_header(
        "Pad Attributes",
        "fp_sketch_pad_attrs",
        collapsed_sections,
        primary,
        border_c,
    ));
    if fp_is_collapsed("fp_sketch_pad_attrs", collapsed_sections) {
        return col;
    }

    // Properties group.
    col = col.push(pad_pick_row(
        "Electrical Type",
        signex_sketch::attr::ElectricalType::ALL,
        p.electrical_type,
        move |v: signex_sketch::attr::ElectricalType| {
            PanelMsg::FpEditorSetSketchPadElectricalType { id, value: v }
        },
        muted,
    ));
    col = col.push(pad_input_row(
        "Net",
        "(unassigned)",
        p.net.clone(),
        move |v| PanelMsg::FpEditorSetSketchPadNet { id, value: v },
        muted,
        primary,
        border_c,
    ));
    col = col.push(pad_check_row(
        "Locked",
        p.locked,
        move |v| PanelMsg::FpEditorToggleSketchPadLocked { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_input_row(
        "Template",
        "",
        p.template.clone(),
        move |v| PanelMsg::FpEditorSetSketchPadTemplate { id, value: v },
        muted,
        primary,
        border_c,
    ));
    col = col.push(pad_input_row(
        "Library",
        "",
        p.template_library.clone(),
        move |v| PanelMsg::FpEditorSetSketchPadTemplateLibrary { id, value: v },
        muted,
        primary,
        border_c,
    ));

    // Pad Features.
    col = col.push(
        container(text("Pad Features").size(9).color(muted))
            .padding([6, 8])
            .width(Length::Fill),
    );
    col = col.push(pad_pick_row(
        "Top Side",
        signex_sketch::attr::PadFeature::ALL,
        p.feature_top,
        move |v: signex_sketch::attr::PadFeature| PanelMsg::FpEditorSetSketchPadFeatureTop {
            id,
            value: v,
        },
        muted,
    ));
    col = col.push(pad_pick_row(
        "Bottom Side",
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
        container(text("Testpoint").size(9).color(muted))
            .padding([6, 8])
            .width(Length::Fill),
    );
    col = col.push(pad_check_row(
        "Top Assembly",
        p.testpoint.top_assembly,
        move |v| PanelMsg::FpEditorToggleSketchPadTestpointTopAssembly { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        "Top Fab",
        p.testpoint.top_fab,
        move |v| PanelMsg::FpEditorToggleSketchPadTestpointTopFab { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        "Bottom Assembly",
        p.testpoint.bottom_assembly,
        move |v| PanelMsg::FpEditorToggleSketchPadTestpointBottomAssembly { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        "Bottom Fab",
        p.testpoint.bottom_fab,
        move |v| PanelMsg::FpEditorToggleSketchPadTestpointBottomFab { id, value: v },
        muted,
        primary,
    ));

    // Pad Stack overrides (the bool-typed subset; expression-typed
    // mask/paste margins stay sketch-parameterised).
    col = col.push(
        container(text("Pad Stack").size(9).color(muted))
            .padding([6, 8])
            .width(Length::Fill),
    );
    col = col.push(pad_check_row(
        "Top paste enabled",
        p.paste_top_enabled,
        move |v| PanelMsg::FpEditorToggleSketchPadPasteEnabledTop { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        "Bottom paste enabled",
        p.paste_bottom_enabled,
        move |v| PanelMsg::FpEditorToggleSketchPadPasteEnabledBottom { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        "Top mask tented",
        p.mask_top_tented,
        move |v| PanelMsg::FpEditorToggleSketchPadMaskTentedTop { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        "Bottom mask tented",
        p.mask_bottom_tented,
        move |v| PanelMsg::FpEditorToggleSketchPadMaskTentedBottom { id, value: v },
        muted,
        primary,
    ));
    col = col.push(pad_check_row(
        "Thermal relief",
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
        "Corner radius %",
        "25",
        corner_buf,
        move |v| PanelMsg::FpEditorSetSketchPadCornerRadiusPct { id, value: v },
        muted,
        primary,
        border_c,
    ));

    // Hole Details (THT pads only).
    if p.has_drill {
        col = col.push(
            container(text("Hole Details").size(9).color(muted))
                .padding([6, 8])
                .width(Length::Fill),
        );
        let tol_p_buf = p
            .hole_tolerance_plus_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Tolerance + (mm)",
            "0",
            tol_p_buf,
            move |v| PanelMsg::FpEditorSetSketchPadHoleTolerancePlus { id, value: v },
            muted,
            primary,
            border_c,
        ));
        let tol_m_buf = p
            .hole_tolerance_minus_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Tolerance − (mm)",
            "0",
            tol_m_buf,
            move |v| PanelMsg::FpEditorSetSketchPadHoleToleranceMinus { id, value: v },
            muted,
            primary,
            border_c,
        ));
        let rot_buf = p
            .hole_rotation_deg
            .map(|v| format!("{v:.1}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Hole rotation (°)",
            "0",
            rot_buf,
            move |v| PanelMsg::FpEditorSetSketchPadHoleRotation { id, value: v },
            muted,
            primary,
            border_c,
        ));
        let cox_buf = p
            .copper_offset_x_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Copper offset X (mm)",
            "0",
            cox_buf,
            move |v| PanelMsg::FpEditorSetSketchPadCopperOffsetX { id, value: v },
            muted,
            primary,
            border_c,
        ));
        let coy_buf = p
            .copper_offset_y_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Copper offset Y (mm)",
            "0",
            coy_buf,
            move |v| PanelMsg::FpEditorSetSketchPadCopperOffsetY { id, value: v },
            muted,
            primary,
            border_c,
        ));
    }

    // v0.22 Phase D6 — "Edit in Pads ▸" jump button. Mirror of the
    // v0.21 "Edit in Sketch ▸" button on the Pads-mode pad form. Flips
    // the editor to Pads mode and selects the pad whose
    // `sketch_entity_id` matches this entity. The dispatcher resolves
    // the EditorPad index from the entity ID.
    col = col.push(
        container(
            iced::widget::button(text("Edit in Pads ▸").size(10).color(primary))
                .padding([4, 10])
                .on_press(PanelMsg::FpEditorEditSketchPadInPads { id })
                .style(iced::widget::button::primary),
        )
        .padding([6, 8])
        .width(Length::Fill),
    );

    col
}
