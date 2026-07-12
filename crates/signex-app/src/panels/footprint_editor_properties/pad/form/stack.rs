//! Pad Stack section renderer, split out of `form/mod.rs` to keep both
//! files under the 800-line cap (ADR-0001 #165). Verbatim code motion.

use iced::widget::{Column, container, text};
use iced::{Color, Length};

use super::super::super::super::{CollapsedSections, PanelMsg};
use super::super::super::{fp_is_collapsed, props_section_header};
use super::super::stack_preview::{
    ExpansionMode, HoleShapeChoice, PadShapeChoice, pad_stack_preview, pad_stack_tab_strip,
};
use super::super::table::{
    pad_copper_row, pad_table_check_cell, pad_table_disabled_cell, pad_table_header,
    pad_table_input_cell, pad_table_picklist_cell, pad_table_row,
};
use super::*;

/// v0.20 — render the "Pad Stack" section: copper shape + size,
/// hole, per-side paste / mask expansions, tented flags, thermal
/// relief, corner radius. Mirrors the Altium PCB Library Pad Stack
/// section: a stylized 2D preview at the top, a Simple/Top-Middle-
/// Bottom/Full Stack tab strip, then the field rows. The tabs are
/// UI-only structure today; per-layer overrides require a v0.21
/// schema follow-up so all three tabs render the same content.
///
/// v0.24 Phase 3 (Track A2) — `shape_params` carries the linked
/// sketch-parameter handles (e.g. `"corner_r"` / `"diameter"`) for
/// the selected pad. Each entry renders an editable text-input row
/// reading / writing the live sketch parameter expression so the
/// user can drive parametric pad geometry from the Properties panel
/// without entering Sketch mode. Empty for pads with no parametric
/// handles (Rect / Oval) and during pad placement (no minted
/// entities yet).
pub(in crate::panels::footprint_editor_properties) fn render_pad_form_pad_stack<'a>(
    mut col: Column<'a, PanelMsg>,
    values: &PadFormValues,
    target: PadEditTarget,
    muted: Color,
    primary: Color,
    border_c: Color,
    collapsed_sections: &'a CollapsedSections,
    shape_params: &'a [crate::panels::PadShapeParamSummary],
) -> Column<'a, PanelMsg> {
    col = col.push(props_section_header(
        "Pad Stack",
        "fp_pad_stack",
        collapsed_sections,
        primary,
        border_c,
    ));
    if fp_is_collapsed("fp_pad_stack", collapsed_sections) {
        return col;
    }

    // ── Preview ──
    col = col.push(pad_stack_preview(values));

    // ── Tab strip (Simple / Top-Middle-Bottom / Full Stack) ──
    col = col.push(pad_stack_tab_strip(values, primary, muted, border_c));

    // ── Altium-parity table layout ── COPPER / HOLE / PASTE / SOLDER
    // each render as a header row + N data rows where every cell sits
    // side-by-side. Column widths use FillPortion so the table
    // adapts to any panel width without truncation.
    use crate::library::editor::footprint::state::{PadSide, PadStackTab};
    let is_multilayer = matches!(values.side, PadSide::All);
    let show_top_side = matches!(values.side, PadSide::Top | PadSide::All);
    let show_bottom_side = matches!(values.side, PadSide::Bottom | PadSide::All);

    let current_shape = PadShapeChoice::from_lib(&values.shape);

    // ── COPPER table ──
    col = col.push(pad_table_header(
        &["COPPER", "X-Size", "Y-Size", "Shape", "Relief"],
        &[3, 2, 2, 3, 1],
        muted,
        border_c,
    ));
    let copper_rows: &[&str] = match values.pad_stack_tab {
        PadStackTab::Simple => &["All Layers"],
        PadStackTab::TopMiddleBottom => &["Top Layer", "Mid Layer", "Bottom Layer"],
        PadStackTab::FullStack => match values.side {
            PadSide::Top => &["F.Cu", "F.Mask", "F.Paste"],
            PadSide::Bottom => &["B.Cu", "B.Mask", "B.Paste"],
            PadSide::All => &["Top Layer", "Inner", "Bottom Layer"],
        },
    };
    for (idx, row_label) in copper_rows.iter().enumerate() {
        let is_first = idx == 0;
        col = col.push(pad_copper_row(
            row_label,
            values,
            current_shape,
            target,
            muted,
            primary,
            border_c,
            // Only the first (top) row drives shape / size; mid / bottom
            // mirror until the v0.21 schema lands.
            is_first,
        ));
    }
    // Corner radius stays as its own labelled row beneath the table —
    // it's a shape-modifier, not a per-layer override.
    if matches!(current_shape, PadShapeChoice::RoundedRectangle) {
        let corner_buf = values
            .stack
            .corner_radius_pct
            .map(|v| format!("{v:.0}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Corner radius %",
            "25",
            corner_buf,
            move |v| pad_corner_radius_msg(target, v),
            muted,
            primary,
            border_c,
        ));
    }

    // v0.24 Phase 3 (Track A2) — parametric-handle rows surface the
    // sketch-bound shape parameters for the selected pad. Each entry
    // is a label + expression input writing back through
    // `FpEditorEditPadShapeParam` so the dispatcher can rewrite
    // `sketch.parameters[parameter_name]` and trigger a solve+rebake.
    // Only renders for `PadEditTarget::Selected` (placement-mode pads
    // have no minted entities, so no params to surface).
    if let PadEditTarget::Selected(pad_idx) = target {
        for entry in shape_params.iter() {
            let key = entry.key.clone();
            let value_string = entry.current_expr.clone();
            col = col.push(pad_input_row(
                entry.label.as_str(),
                "0.25mm",
                value_string,
                move |v| PanelMsg::FpEditorEditPadShapeParam {
                    pad_idx,
                    key: key.clone(),
                    value: v,
                },
                muted,
                primary,
                border_c,
            ));
        }
    }

    // ── HOLE table ── Multi-Layer pads only.
    if is_multilayer {
        col = col.push(pad_table_header(
            &["HOLE", "Size", "Length", "Shape", "Plated"],
            &[3, 2, 2, 3, 1],
            muted,
            border_c,
        ));
        let hole_shape_current = if values.drill_slot_length_mm.is_some() {
            HoleShapeChoice::Slot
        } else {
            HoleShapeChoice::Round
        };
        let slot_default = values.drill_diameter_mm.map(|d| d * 1.5).unwrap_or(1.0);
        // v0.25 polish — prefer the verbatim user buffer if one is
        // registered for this input; only fall back to formatting
        // the canonical f64 when no buffer exists. Without this
        // override the renderer reformats every keystroke (e.g.
        // "0" → "0.000" or "0.1." → "" on parse failure) and
        // fights the user''s in-flight typing.
        let drill_buffer_key = match target {
            PadEditTarget::Next => "next.drill_diameter".to_string(),
            PadEditTarget::Selected(idx) => format!("selected.{idx}.drill_diameter"),
        };
        let drill_buf = values
            .numeric_buffers
            .get(&drill_buffer_key)
            .cloned()
            .unwrap_or_else(|| {
                values
                    .drill_diameter_mm
                    .map(|v| format!("{v:.3}"))
                    .unwrap_or_default()
            });
        let slot_buf = values
            .drill_slot_length_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        let slot_enabled = matches!(hole_shape_current, HoleShapeChoice::Slot);
        col = col.push(pad_table_row(
            "Pad Hole",
            vec![
                pad_table_input_cell(
                    drill_buf,
                    "0",
                    move |v| pad_drill_diameter_msg(target, v),
                    muted,
                    primary,
                    border_c,
                ),
                if slot_enabled {
                    pad_table_input_cell(
                        slot_buf,
                        "0",
                        move |v| pad_drill_slot_length_msg(target, v),
                        muted,
                        primary,
                        border_c,
                    )
                } else {
                    pad_table_disabled_cell("0mm", muted, border_c)
                },
                pad_table_picklist_cell(
                    HoleShapeChoice::ALL,
                    hole_shape_current,
                    move |c: HoleShapeChoice| match c {
                        HoleShapeChoice::Round => pad_drill_slot_length_msg(target, String::new()),
                        HoleShapeChoice::Slot => {
                            pad_drill_slot_length_msg(target, format!("{slot_default:.3}"))
                        }
                    },
                ),
                pad_table_check_cell(
                    !matches!(values.kind, signex_library::PadKind::NptHole),
                    move |v| pad_plated_msg(target, v),
                ),
            ],
            &[2, 2, 3, 1],
            muted,
            primary,
            border_c,
        ));
        // v0.21 — Hole detail fields. Tolerance is reporting-only
        // (drives IPC-356 / drill table), Rotation orients Slot/Rect
        // holes, Copper Offset shifts the copper outline relative to
        // the hole centre. Only shown for Multi-Layer pads.
        col = col.push(
            container(text("Hole Details").size(9).color(muted))
                .padding([6, 8])
                .width(Length::Fill),
        );
        let tol_p_buf = values
            .hole_tolerance_plus_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Tolerance + (mm)",
            "0",
            tol_p_buf,
            move |v| pad_hole_tolerance_plus_msg(target, v),
            muted,
            primary,
            border_c,
        ));
        let tol_m_buf = values
            .hole_tolerance_minus_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Tolerance − (mm)",
            "0",
            tol_m_buf,
            move |v| pad_hole_tolerance_minus_msg(target, v),
            muted,
            primary,
            border_c,
        ));
        let rot_buf = values
            .hole_rotation_deg
            .map(|v| format!("{v:.1}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Hole rotation (°)",
            "0",
            rot_buf,
            move |v| pad_hole_rotation_msg(target, v),
            muted,
            primary,
            border_c,
        ));
        let cox_buf = values
            .copper_offset_x_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Copper offset X (mm)",
            "0",
            cox_buf,
            move |v| pad_copper_offset_x_msg(target, v),
            muted,
            primary,
            border_c,
        ));
        let coy_buf = values
            .copper_offset_y_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row(
            "Copper offset Y (mm)",
            "0",
            coy_buf,
            move |v| pad_copper_offset_y_msg(target, v),
            muted,
            primary,
            border_c,
        ));
    }

    // ── PASTE table ──
    if show_top_side || show_bottom_side {
        col = col.push(pad_table_header(
            &["PASTE", "Expansion", "%", "Shape", "Enabled"],
            &[3, 2, 2, 3, 1],
            muted,
            border_c,
        ));
    }
    if show_top_side {
        let paste_top_buf = values
            .stack
            .paste_margin_top
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        let paste_top_mode = if values.stack.paste_margin_top.is_some() {
            ExpansionMode::Manual
        } else {
            ExpansionMode::Rule
        };
        col = col.push(pad_table_row(
            "Top Paste",
            vec![
                pad_table_input_cell(
                    paste_top_buf,
                    "(rule)",
                    move |v| pad_paste_top_msg(target, v),
                    muted,
                    primary,
                    border_c,
                ),
                pad_table_disabled_cell("0%", muted, border_c),
                pad_table_picklist_cell(
                    ExpansionMode::ALL,
                    paste_top_mode,
                    move |m: ExpansionMode| match m {
                        ExpansionMode::Rule => pad_paste_top_msg(target, String::new()),
                        ExpansionMode::Manual => pad_paste_top_msg(target, "0".into()),
                    },
                ),
                pad_table_check_cell(values.stack.paste_enabled_top, move |v| {
                    pad_paste_enabled_top_msg(target, v)
                }),
            ],
            &[2, 2, 3, 1],
            muted,
            primary,
            border_c,
        ));
    }
    if show_bottom_side {
        let paste_bot_buf = values
            .stack
            .paste_margin_bottom
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        let paste_bot_mode = if values.stack.paste_margin_bottom.is_some() {
            ExpansionMode::Manual
        } else {
            ExpansionMode::Rule
        };
        col = col.push(pad_table_row(
            "Bottom Paste",
            vec![
                pad_table_input_cell(
                    paste_bot_buf,
                    "(rule)",
                    move |v| pad_paste_bottom_msg(target, v),
                    muted,
                    primary,
                    border_c,
                ),
                pad_table_disabled_cell("0%", muted, border_c),
                pad_table_picklist_cell(
                    ExpansionMode::ALL,
                    paste_bot_mode,
                    move |m: ExpansionMode| match m {
                        ExpansionMode::Rule => pad_paste_bottom_msg(target, String::new()),
                        ExpansionMode::Manual => pad_paste_bottom_msg(target, "0".into()),
                    },
                ),
                pad_table_check_cell(values.stack.paste_enabled_bottom, move |v| {
                    pad_paste_enabled_bottom_msg(target, v)
                }),
            ],
            &[2, 2, 3, 1],
            muted,
            primary,
            border_c,
        ));
    }

    // ── SOLDER table ──
    if show_top_side || show_bottom_side {
        col = col.push(pad_table_header(
            &["SOLDER", "Expansion", "Shape", "Tented"],
            &[3, 2, 3, 1],
            muted,
            border_c,
        ));
    }
    if show_top_side {
        let mask_top_buf = values
            .stack
            .mask_margin_top
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        let mask_top_mode = if values.stack.mask_margin_top.is_some() {
            ExpansionMode::Manual
        } else {
            ExpansionMode::Rule
        };
        col = col.push(pad_table_row(
            "Top Solder Mask",
            vec![
                pad_table_input_cell(
                    mask_top_buf,
                    "(rule)",
                    move |v| pad_mask_top_msg(target, v),
                    muted,
                    primary,
                    border_c,
                ),
                pad_table_picklist_cell(
                    ExpansionMode::ALL,
                    mask_top_mode,
                    move |m: ExpansionMode| match m {
                        ExpansionMode::Rule => pad_mask_top_msg(target, String::new()),
                        ExpansionMode::Manual => pad_mask_top_msg(target, "0".into()),
                    },
                ),
                pad_table_check_cell(values.stack.mask_tented_top, move |v| {
                    pad_mask_tented_top_msg(target, v)
                }),
            ],
            &[2, 3, 1],
            muted,
            primary,
            border_c,
        ));
    }
    if show_bottom_side {
        let mask_bot_buf = values
            .stack
            .mask_margin_bottom
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        let mask_bot_mode = if values.stack.mask_margin_bottom.is_some() {
            ExpansionMode::Manual
        } else {
            ExpansionMode::Rule
        };
        col = col.push(pad_table_row(
            "Bottom Solder Mask",
            vec![
                pad_table_input_cell(
                    mask_bot_buf,
                    "(rule)",
                    move |v| pad_mask_bottom_msg(target, v),
                    muted,
                    primary,
                    border_c,
                ),
                pad_table_picklist_cell(
                    ExpansionMode::ALL,
                    mask_bot_mode,
                    move |m: ExpansionMode| match m {
                        ExpansionMode::Rule => pad_mask_bottom_msg(target, String::new()),
                        ExpansionMode::Manual => pad_mask_bottom_msg(target, "0".into()),
                    },
                ),
                pad_table_check_cell(values.stack.mask_tented_bottom, move |v| {
                    pad_mask_tented_bottom_msg(target, v)
                }),
            ],
            &[2, 3, 1],
            muted,
            primary,
            border_c,
        ));
    }

    col
}

