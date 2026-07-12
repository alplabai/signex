//! Role-specific sub-forms — Sketch-mode pad attributes, Pour,
//! Keepout, Cutout, Pattern (array). Each is rendered when the
//! Properties panel detects the matching role on the selected sketch
//! entity / pad and shows only the relevant fields.

use iced::widget::{
    Column, Space, button, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Background, Border, Color, Element, Length, Theme};

use super::super::{CollapsedSections, FootprintEditorPanelContext, KeepoutKindFlag, PanelMsg};
use super::pad::{pad_check_row, pad_input_row, pad_pick_row};
use super::{fp_is_collapsed, props_section_header};

// v0.23 — safety caps mirrored from mod.rs's constants for the
// per-instance checkbox grids in the Pattern sub-form.
const MAX_GRID_CHECKBOX_DIM: u32 = 32;
const MAX_POLAR_CHECKBOX_COUNT: u32 = 64;

/// v0.21 — Sketch-mode Pad Attributes sub-form. Renders when the
/// selected sketch entity carries a `PadAttr`. Mirrors the
/// Altium-parity Pad Properties / Pad Stack / Pad Features fields
/// surfaced for Pads-mode placement, but bound to the sketch
/// entity's PadAttr rather than the flat-pad list. Geometry-shaping
/// fields (size_x_expr / size_y_expr / mask_margin_expr / etc.)
/// stay sketch-parameterised — those are authored via the Sketch
/// parameter editor, not this form.
pub(super) fn render_sketch_pad_subform<'a>(
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

/// v0.16.4 — Pour role sub-form. Renders when the entity's `pour`
/// attr is set; otherwise the column passes through unchanged.
pub(super) fn render_pour_subform<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    id: signex_sketch::id::SketchEntityId,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Column<'a, PanelMsg> {
    let Some(pour) = fp.selected_pour.as_ref() else {
        return col;
    };
    col = col.push(
        container(text("Pour properties").size(10).color(primary))
            .padding([4, 8])
            .width(Length::Fill),
    );

    // Net (text input — empty = unassigned)
    let net_buf = pour.net.clone().unwrap_or_default();
    col = col.push(
        container(
            row![
                text("Net").size(10).color(muted).width(Length::Fixed(80.0)),
                text_input("(none)", &net_buf)
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
                    .on_input(move |v| PanelMsg::FpEditorSetPourNet { id, value: v }),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );

    // Fill type (Solid / Hatched / Outline)
    let fill_picker = pick_list(
        signex_sketch::attr::PourFillType::ALL,
        Some(pour.fill_type),
        move |v| PanelMsg::FpEditorSetPourFillType { id, value: v },
    )
    .text_size(10)
    .padding([3, 8]);
    col = col.push(
        container(
            row![
                text("Fill")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(80.0)),
                fill_picker,
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );

    // Priority (u32 text input)
    col = col.push(
        container(
            row![
                text("Priority")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(80.0)),
                text_input("0", &pour.priority.to_string())
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
                    .on_input(move |v| PanelMsg::FpEditorSetPourPriority { id, value: v }),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );

    col
}

/// v0.16.4 — Keepout role sub-form. Renders the 6 kind flags as a
/// vertical checklist when the entity's `keepout` attr is set.
pub(super) fn render_keepout_subform<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    id: signex_sketch::id::SketchEntityId,
    muted: Color,
    primary: Color,
    _border_c: Color,
) -> Column<'a, PanelMsg> {
    let Some(k) = fp.selected_keepout.as_ref() else {
        return col;
    };
    col = col.push(
        container(text("Keepout kinds").size(10).color(primary))
            .padding([4, 8])
            .width(Length::Fill),
    );

    // v0.18.25 — keepout kinds use the schematic's `form_check_row`
    // (real iced checkbox + On/Off) so the chrome matches the
    // schematic Properties panel byte-for-byte.
    col = col.push(super::super::form_check_row(
        "No routing",
        k.no_routing,
        PanelMsg::FpEditorSetKeepoutKind {
            id,
            kind: KeepoutKindFlag::NoRouting,
            value: !k.no_routing,
        },
        muted,
    ));
    col = col.push(super::super::form_check_row(
        "No components",
        k.no_components,
        PanelMsg::FpEditorSetKeepoutKind {
            id,
            kind: KeepoutKindFlag::NoComponents,
            value: !k.no_components,
        },
        muted,
    ));
    col = col.push(super::super::form_check_row(
        "No copper",
        k.no_copper,
        PanelMsg::FpEditorSetKeepoutKind {
            id,
            kind: KeepoutKindFlag::NoCopper,
            value: !k.no_copper,
        },
        muted,
    ));
    col = col.push(super::super::form_check_row(
        "No vias",
        k.no_vias,
        PanelMsg::FpEditorSetKeepoutKind {
            id,
            kind: KeepoutKindFlag::NoVias,
            value: !k.no_vias,
        },
        muted,
    ));
    col = col.push(super::super::form_check_row(
        "No drilling",
        k.no_drilling,
        PanelMsg::FpEditorSetKeepoutKind {
            id,
            kind: KeepoutKindFlag::NoDrilling,
            value: !k.no_drilling,
        },
        muted,
    ));
    col = col.push(super::super::form_check_row(
        "No pours",
        k.no_pours,
        PanelMsg::FpEditorSetKeepoutKind {
            id,
            kind: KeepoutKindFlag::NoPours,
            value: !k.no_pours,
        },
        muted,
    ));

    col
}

/// v0.16.4 — BoardCutout role sub-form. Edge-radius expression input
/// + through-vs-partial-depth toggle.
pub(super) fn render_cutout_subform<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    id: signex_sketch::id::SketchEntityId,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Column<'a, PanelMsg> {
    let Some(c) = fp.selected_cutout.as_ref() else {
        return col;
    };
    col = col.push(
        container(text("Cutout properties").size(10).color(primary))
            .padding([4, 8])
            .width(Length::Fill),
    );

    let radius_buf = c.edge_radius_expr.clone().unwrap_or_default();
    col = col.push(
        container(
            row![
                text("Edge radius")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(80.0)),
                text_input("(sharp)", &radius_buf)
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
                    .on_input(move |v| PanelMsg::FpEditorSetCutoutEdgeRadius { id, value: v }),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );

    col = col.push(super::super::form_check_row(
        "Through (full board depth)",
        c.through,
        PanelMsg::FpEditorSetCutoutThrough {
            id,
            value: !c.through,
        },
        muted,
    ));

    col
}

/// v0.23 — Pattern Properties sub-form. Renders the editable
/// expressions for a Linear / Grid / Polar [`signex_sketch::array`]
/// when the selected sketch entity is its source. Each text input
/// emits a [`PanelMsg::FpEditorEditArrayParam`]; the numbering
/// pick_list emits [`PanelMsg::FpEditorSetArrayNumberingScheme`]; the
/// Delete button emits [`PanelMsg::FpEditorDeleteArray`]; the Re-pick
/// centre button (Polar only) emits
/// [`PanelMsg::FpEditorBeginRepickPolarCenter`].
pub(super) fn render_pattern_subform<'a>(
    mut col: Column<'a, PanelMsg>,
    fp: &'a FootprintEditorPanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
    collapsed_sections: &'a CollapsedSections,
) -> Column<'a, PanelMsg> {
    use crate::panels::{ArrayKindSummary, ArrayParamField, NumberingSchemeKindUi};
    let Some(arr) = fp.selected_array.as_ref() else {
        return col;
    };
    let array_id = arr.array_id;

    col = col.push(props_section_header(
        "Pattern",
        "fp_pattern",
        collapsed_sections,
        primary,
        border_c,
    ));
    if fp_is_collapsed("fp_pattern", collapsed_sections) {
        return col;
    }

    let kind_label: &'static str = match arr.kind {
        ArrayKindSummary::Linear { .. } => "Linear",
        ArrayKindSummary::Grid { .. } => "Grid",
        ArrayKindSummary::Polar { .. } => "Polar",
    };
    col = col.push(
        container(
            row![
                text("Kind")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(80.0)),
                text(kind_label).size(10).color(primary),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );

    match &arr.kind {
        ArrayKindSummary::Linear {
            count_expr,
            dx_expr,
            dy_expr,
        } => {
            col = col.push(pad_input_row(
                "Count",
                "1",
                count_expr.clone(),
                move |v| PanelMsg::FpEditorEditArrayParam {
                    array_id,
                    field: ArrayParamField::LinearCountExpr,
                    value: v,
                },
                muted,
                primary,
                border_c,
            ));
            col = col.push(pad_input_row(
                "dX",
                "0",
                dx_expr.clone(),
                move |v| PanelMsg::FpEditorEditArrayParam {
                    array_id,
                    field: ArrayParamField::LinearDxExpr,
                    value: v,
                },
                muted,
                primary,
                border_c,
            ));
            col = col.push(pad_input_row(
                "dY",
                "0",
                dy_expr.clone(),
                move |v| PanelMsg::FpEditorEditArrayParam {
                    array_id,
                    field: ArrayParamField::LinearDyExpr,
                    value: v,
                },
                muted,
                primary,
                border_c,
            ));
        }
        ArrayKindSummary::Grid {
            nx_expr,
            ny_expr,
            dx_expr,
            dy_expr,
            mask_expr,
            suppressed_instances,
            nx_value,
            ny_value,
        } => {
            col = col.push(pad_input_row(
                "nx",
                "1",
                nx_expr.clone(),
                move |v| PanelMsg::FpEditorEditArrayParam {
                    array_id,
                    field: ArrayParamField::GridNxExpr,
                    value: v,
                },
                muted,
                primary,
                border_c,
            ));
            col = col.push(pad_input_row(
                "ny",
                "1",
                ny_expr.clone(),
                move |v| PanelMsg::FpEditorEditArrayParam {
                    array_id,
                    field: ArrayParamField::GridNyExpr,
                    value: v,
                },
                muted,
                primary,
                border_c,
            ));
            col = col.push(pad_input_row(
                "dX",
                "0",
                dx_expr.clone(),
                move |v| PanelMsg::FpEditorEditArrayParam {
                    array_id,
                    field: ArrayParamField::GridDxExpr,
                    value: v,
                },
                muted,
                primary,
                border_c,
            ));
            col = col.push(pad_input_row(
                "dY",
                "0",
                dy_expr.clone(),
                move |v| PanelMsg::FpEditorEditArrayParam {
                    array_id,
                    field: ArrayParamField::GridDyExpr,
                    value: v,
                },
                muted,
                primary,
                border_c,
            ));
            col = col.push(pad_input_row(
                "Mask",
                "i != 0 || j != 0",
                mask_expr.clone(),
                move |v| PanelMsg::FpEditorEditArrayParam {
                    array_id,
                    field: ArrayParamField::MaskExpr,
                    value: v,
                },
                muted,
                primary,
                border_c,
            ));
            // v0.23 — Per-instance checkbox grid. Renders only when
            // both nx and ny resolve to a concrete integer; parameter-
            // driven counts (e.g. "= row_count") fall back to the mask
            // expression input.
            if let (Some(nx), Some(ny)) = (nx_value, ny_value) {
                let nx = (*nx).min(MAX_GRID_CHECKBOX_DIM);
                let ny = (*ny).min(MAX_GRID_CHECKBOX_DIM);
                col = col.push(
                    container(text("Instances").size(10).color(muted))
                        .padding([6, 8])
                        .width(Length::Fill),
                );
                let mut grid_col = Column::new().spacing(2).padding([0, 8]);
                for j in 0..ny {
                    let mut grid_row = iced::widget::Row::new().spacing(2);
                    for i in 0..nx {
                        let suppressed = suppressed_instances
                            .iter()
                            .any(|(si, sj)| *si == i && *sj == j);
                        let on = !suppressed;
                        let cb = iced::widget::checkbox(on).size(12).spacing(0).on_toggle(
                            move |checked| PanelMsg::FpEditorToggleArrayInstance {
                                array_id,
                                i,
                                j,
                                value: checked,
                            },
                        );
                        grid_row = grid_row.push(cb);
                    }
                    grid_col = grid_col.push(grid_row);
                }
                col = col.push(grid_col);
            }
        }
        ArrayKindSummary::Polar {
            count_expr,
            sweep_angle_expr,
            center_position_mm,
            mask_expr,
            suppressed_instances,
            count_value,
        } => {
            col = col.push(pad_input_row(
                "Count",
                "1",
                count_expr.clone(),
                move |v| PanelMsg::FpEditorEditArrayParam {
                    array_id,
                    field: ArrayParamField::PolarCountExpr,
                    value: v,
                },
                muted,
                primary,
                border_c,
            ));
            col = col.push(pad_input_row(
                "Sweep",
                "360deg",
                sweep_angle_expr.clone(),
                move |v| PanelMsg::FpEditorEditArrayParam {
                    array_id,
                    field: ArrayParamField::PolarSweepAngleExpr,
                    value: v,
                },
                muted,
                primary,
                border_c,
            ));
            // Centre point picker — show coordinates if known; the
            // "Re-pick" button arms ToolPending so the next sketch
            // click on a Point overwrites the centre.
            let centre_label = match center_position_mm {
                Some([x, y]) => format!("({:.3}, {:.3}) mm", x, y),
                None => "(centre lost)".to_string(),
            };
            col = col.push(
                container(
                    row![
                        text("Centre")
                            .size(10)
                            .color(muted)
                            .width(Length::Fixed(80.0)),
                        text(centre_label).size(10).color(primary),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                )
                .padding([2, 8])
                .width(Length::Fill),
            );
            let repick_label = if arr.repicking_polar_center {
                "Click a Point on canvas… (Esc to cancel)"
            } else {
                "Re-pick centre"
            };
            let repick_btn_style = if arr.repicking_polar_center {
                iced::widget::button::primary
            } else {
                iced::widget::button::secondary
            };
            col = col.push(
                container(
                    button(text(repick_label).size(10).color(primary))
                        .padding([3, 8])
                        .style(repick_btn_style)
                        .on_press(PanelMsg::FpEditorBeginRepickPolarCenter { array_id }),
                )
                .padding([2, 8])
                .width(Length::Fill),
            );
            col = col.push(pad_input_row(
                "Mask",
                "i != 0",
                mask_expr.clone(),
                move |v| PanelMsg::FpEditorEditArrayParam {
                    array_id,
                    field: ArrayParamField::MaskExpr,
                    value: v,
                },
                muted,
                primary,
                border_c,
            ));
            // v0.23 — Per-instance checkbox row for Polar arrays.
            // Renders only when count resolves to a concrete integer.
            if let Some(count) = count_value {
                let count = (*count).min(MAX_POLAR_CHECKBOX_COUNT);
                col = col.push(
                    container(text("Instances").size(10).color(muted))
                        .padding([6, 8])
                        .width(Length::Fill),
                );
                let mut grid_row = iced::widget::Row::new().spacing(2).padding([0, 8]);
                for i in 0..count {
                    let suppressed = suppressed_instances.iter().any(|si| *si == i);
                    let on = !suppressed;
                    let cb =
                        iced::widget::checkbox(on)
                            .size(12)
                            .spacing(0)
                            .on_toggle(move |checked| PanelMsg::FpEditorToggleArrayInstance {
                                array_id,
                                i,
                                j: 0,
                                value: checked,
                            });
                    grid_row = grid_row.push(cb);
                }
                col = col.push(grid_row);
            }
        }
    }

    // Numbering scheme dropdown.
    col = col.push(
        container(
            row![
                text("Numbering")
                    .size(10)
                    .color(muted)
                    .width(Length::Fixed(80.0)),
                pick_list(
                    NumberingSchemeKindUi::ALL.as_slice(),
                    Some(arr.numbering),
                    move |scheme| PanelMsg::FpEditorSetArrayNumberingScheme { array_id, scheme }
                )
                .text_size(10)
                .padding([3, 8])
                .width(Length::Fill),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .width(Length::Fill),
    );

    // v0.25 polish — BGA-specific config rows. Surface only when the
    // array's numbering is BgaRowCol so the user gets the IPC-7351
    // letter-skip toggle + the start letter / column inputs.
    if let Some(bga) = arr.bga_config.as_ref() {
        col = col.push(pad_check_row(
            "Skip I/O/Q/S/X/Z (IPC-7351)",
            bga.skip_letters,
            move |v| PanelMsg::FpEditorSetBgaSkipLetters { array_id, value: v },
            muted,
            primary,
        ));
        col = col.push(pad_input_row(
            "Start row",
            "A",
            bga.start_row.to_string(),
            move |v| PanelMsg::FpEditorSetBgaStartRow { array_id, value: v },
            muted,
            primary,
            border_c,
        ));
        col = col.push(pad_input_row(
            "Start col",
            "1",
            bga.start_col.to_string(),
            move |v| PanelMsg::FpEditorSetBgaStartCol { array_id, value: v },
            muted,
            primary,
            border_c,
        ));
    }

    // Delete button — destructive style. Source entity stays put.
    col = col.push(
        container(
            button(text("Delete pattern").size(10).color(primary))
                .padding([3, 8])
                .style(iced::widget::button::danger)
                .on_press(PanelMsg::FpEditorDeleteArray { array_id }),
        )
        .padding([4, 8])
        .width(Length::Fill),
    );

    col
}
