//! Pattern (array) role sub-form. Split from `subforms.rs`.

use iced::widget::{Column, button, container, pick_list, row, text};
use iced::{Color, Length};

use super::super::super::{CollapsedSections, FootprintEditorPanelContext, PanelMsg};
use super::super::pad::{pad_check_row, pad_input_row};
use super::super::{fp_is_collapsed, props_section_header};

// v0.23 — safety caps mirrored from mod.rs's constants for the
// per-instance checkbox grids in the Pattern sub-form.
const MAX_GRID_CHECKBOX_DIM: u32 = 32;
const MAX_POLAR_CHECKBOX_COUNT: u32 = 64;

/// v0.23 — Pattern Properties sub-form. Renders the editable
/// expressions for a Linear / Grid / Polar [`signex_sketch::array`]
/// when the selected sketch entity is its source. Each text input
/// emits a [`PanelMsg::FpEditorEditArrayParam`]; the numbering
/// pick_list emits [`PanelMsg::FpEditorSetArrayNumberingScheme`]; the
/// Delete button emits [`PanelMsg::FpEditorDeleteArray`]; the Re-pick
/// centre button (Polar only) emits
/// [`PanelMsg::FpEditorBeginRepickPolarCenter`].
pub(in crate::panels::footprint_editor_properties) fn render_pattern_subform<'a>(
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
