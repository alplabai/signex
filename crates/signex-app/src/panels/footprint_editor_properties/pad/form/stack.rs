//! Pad Stack section renderer, split out of `form/mod.rs` to keep both
//! files under the 800-line cap (ADR-0001 #165). Verbatim code motion.

use iced::widget::{Column, container};
use iced::{Element, Length, Widget};
use signex_widgets::{raw, text};

use super::super::super::super::{CollapsedSections, PanelMsg, PanelPalette};
use super::super::stack_preview::{
    ExpansionMode, HoleShapeChoice, PadShapeChoice, pad_stack_preview, pad_stack_tab_strip,
};
use super::super::table::{
    COPPER_CELLS, HOLE_CELLS, PASTE_CELLS, SOLDER_CELLS, pad_copper_row, pad_size_buf,
    pad_table_check_cell, pad_table_disabled_cell, pad_table_header, pad_table_input_cell,
    pad_table_picklist_cell, pad_table_row,
};
use super::*;
use crate::panels::{collapsible_section_header, is_section_collapsed};
use crate::styles::Spacing;

/// Default type / spacing scales; equal to the old literals at the default size.
const SP: Spacing = Spacing::DEFAULT;

// Per-side row labels: the layer names `pcb.ftl` gives (pinned by a test).
const TOP_LAYER: &str = "Top Layer";
const BOTTOM_LAYER: &str = "Bottom Layer";
/// The row for every inner copper layer at once, Altium's Top-Middle-Bottom
/// pad stack term; one inner layer is "Mid-Layer N" (`pcb.ftl`).
const MID_LAYERS: &str = "Mid Layer";
const TOP_SOLDER: &str = "Top Solder";
const BOTTOM_SOLDER: &str = "Bottom Solder";
const TOP_PASTE: &str = "Top Paste";
const BOTTOM_PASTE: &str = "Bottom Paste";

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
    mut col: Column<Element<'a, PanelMsg>>,
    values: &PadFormValues,
    target: PadEditTarget,
    palette: PanelPalette,
    collapsed_sections: &'a CollapsedSections,
    shape_params: &'a [crate::panels::PadShapeParamSummary],
    i18n: &signex_i18n::I18n,
) -> Column<Element<'a, PanelMsg>> {
    let PanelPalette {
        muted,
        primary,
        border: border_c,
        ..
    } = palette;
    let fw = palette.field_widths();
    col = col.push(
        collapsible_section_header(
            "fp_pad_stack",
            "Pad Stack",
            collapsed_sections,
            primary,
            border_c,
        )
        .boxed(),
    );
    if is_section_collapsed("fp_pad_stack", collapsed_sections) {
        return col;
    }

    // ── Preview ──
    col = col.push(pad_stack_preview(values));

    // ── Tab strip (Simple / Top-Middle-Bottom / Full Stack) ──
    col = col.push(pad_stack_tab_strip(values, primary, muted));

    // ── Altium-parity table layout ── COPPER / HOLE / PASTE / SOLDER
    // each render as a header row + N data rows where every cell sits
    // side-by-side. Column widths use FillPortion so the table
    // adapts to any panel width without truncation.
    use crate::library::editor::footprint::state::{PadSide, PadStackTab};
    let is_multilayer = matches!(values.side, PadSide::All);
    let show_top_side = matches!(values.side, PadSide::Top | PadSide::All);
    let show_bottom_side = matches!(values.side, PadSide::Bottom | PadSide::All);

    let current_shape = PadShapeChoice::from_lib(&values.shape);

    // ── COPPER ── Simple: one copper for all layers, as label / value
    // rows like the rest of the form. Per-layer tabs: a table.
    let is_simple = values.pad_stack_tab == PadStackTab::Simple;
    if is_simple {
        col = push_simple_copper_rows(col, values, current_shape, target, fw, muted, i18n);
    } else {
        col = col.push(pad_table_header(
            &["COPPER", "X-Size", "Y-Size", "Shape", "Relief"],
            &COPPER_CELLS,
            muted,
            border_c,
        ));
        let copper_rows: &[&str] = match values.pad_stack_tab {
            // Simple drew its rows above; it never reaches the table.
            PadStackTab::Simple | PadStackTab::TopMiddleBottom => {
                &[TOP_LAYER, MID_LAYERS, BOTTOM_LAYER]
            }
            PadStackTab::FullStack => match values.side {
                PadSide::Top => &[TOP_LAYER, TOP_SOLDER, TOP_PASTE],
                PadSide::Bottom => &[BOTTOM_LAYER, BOTTOM_SOLDER, BOTTOM_PASTE],
                PadSide::All => &[TOP_LAYER, MID_LAYERS, BOTTOM_LAYER],
            },
        };
        for (idx, row_label) in copper_rows.iter().enumerate() {
            let is_first = idx == 0;
            col = col.push(pad_copper_row(
                row_label,
                values,
                current_shape,
                target,
                palette,
                // Only the first (top) row drives shape / size; mid / bottom
                // mirror until the schema carries per-layer shape / size.
                is_first,
            ));
        }
    }
    // Corner radius stays as its own labelled row beneath the table —
    // it's a shape-modifier, not a per-layer override.
    if matches!(current_shape, PadShapeChoice::RoundedRectangle) {
        let corner_buf = values
            .stack
            .corner_radius_pct
            .map(|v| format!("{v:.0}"))
            .unwrap_or_default();
        col = col.push(pad_input_row_w(
            fw,
            t!(i18n, ids::EDITORS_PAD_CORNER_RADIUS),
            "25",
            corner_buf,
            move |v| pad_corner_radius_msg(target, v),
            muted,
        ));
    }
    // Simple's thermal relief toggle, the table's Relief column.
    if is_simple {
        col = col.push(pad_check_row_w(
            fw,
            t!(i18n, ids::EDITORS_PAD_RELIEF),
            values.stack.thermal_relief,
            move |v| pad_thermal_relief_msg(target, v),
            muted,
            primary,
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
            col = col.push(pad_input_row_w(
                fw,
                raw(entry.label.as_str()),
                "0.25mm",
                value_string,
                move |v| PanelMsg::FpEditorEditPadShapeParam {
                    pad_idx,
                    key: key.clone(),
                    value: v,
                },
                muted,
            ));
        }
    }

    // ── HOLE table ── Multi-Layer pads only.
    if is_multilayer {
        col = col.push(pad_table_header(
            &["HOLE", "Size", "Length", "Shape", "Plated"],
            &HOLE_CELLS,
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
                pad_table_input_cell(drill_buf, "0", move |v| pad_drill_diameter_msg(target, v)),
                if slot_enabled {
                    pad_table_input_cell(slot_buf, "0", move |v| {
                        pad_drill_slot_length_msg(target, v)
                    })
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
            &HOLE_CELLS,
            primary,
            border_c,
        ));
        // v0.21 — Hole detail fields. Tolerance is reporting-only
        // (drives IPC-356 / drill table), Rotation orients Slot/Rect
        // holes, Copper Offset shifts the copper outline relative to
        // the hole centre. Only shown for Multi-Layer pads.
        col = col.push(
            container(
                text(t!(i18n, ids::EDITORS_PAD_HOLE_DETAILS))
                    .size(TS.caption)
                    .color(muted),
            )
            .padding([6.0, SP.m])
            .width(Length::Fill)
            .boxed(),
        );
        let tol_p_buf = values
            .hole_tolerance_plus_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row_w(
            fw,
            t!(i18n, ids::EDITORS_PAD_HOLE_TOLERANCE_PLUS),
            "0",
            tol_p_buf,
            move |v| pad_hole_tolerance_plus_msg(target, v),
            muted,
        ));
        let tol_m_buf = values
            .hole_tolerance_minus_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row_w(
            fw,
            t!(i18n, ids::EDITORS_PAD_HOLE_TOLERANCE_MINUS),
            "0",
            tol_m_buf,
            move |v| pad_hole_tolerance_minus_msg(target, v),
            muted,
        ));
        let rot_buf = values
            .hole_rotation_deg
            .map(|v| format!("{v:.1}"))
            .unwrap_or_default();
        col = col.push(pad_input_row_w(
            fw,
            t!(i18n, ids::EDITORS_PAD_HOLE_ROTATION),
            "0",
            rot_buf,
            move |v| pad_hole_rotation_msg(target, v),
            muted,
        ));
        let cox_buf = values
            .copper_offset_x_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row_w(
            fw,
            t!(i18n, ids::EDITORS_PAD_COPPER_OFFSET_X),
            "0",
            cox_buf,
            move |v| pad_copper_offset_x_msg(target, v),
            muted,
        ));
        let coy_buf = values
            .copper_offset_y_mm
            .map(|v| format!("{v:.3}"))
            .unwrap_or_default();
        col = col.push(pad_input_row_w(
            fw,
            t!(i18n, ids::EDITORS_PAD_COPPER_OFFSET_Y),
            "0",
            coy_buf,
            move |v| pad_copper_offset_y_msg(target, v),
            muted,
        ));
    }

    // ── PASTE table ──
    if show_top_side || show_bottom_side {
        col = col.push(pad_table_header(
            &["PASTE", "Expansion", "%", "Shape", "Enabled"],
            &PASTE_CELLS,
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
            TOP_PASTE,
            vec![
                pad_table_input_cell(paste_top_buf, "(rule)", move |v| {
                    pad_paste_top_msg(target, v)
                }),
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
            &PASTE_CELLS,
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
            BOTTOM_PASTE,
            vec![
                pad_table_input_cell(paste_bot_buf, "(rule)", move |v| {
                    pad_paste_bottom_msg(target, v)
                }),
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
            &PASTE_CELLS,
            primary,
            border_c,
        ));
    }

    // ── SOLDER table ──
    if show_top_side || show_bottom_side {
        col = col.push(pad_table_header(
            &["SOLDER", "Expansion", "Shape", "Tented"],
            &SOLDER_CELLS,
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
            TOP_SOLDER,
            vec![
                pad_table_input_cell(mask_top_buf, "(rule)", move |v| pad_mask_top_msg(target, v)),
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
            &SOLDER_CELLS,
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
            BOTTOM_SOLDER,
            vec![
                pad_table_input_cell(mask_bot_buf, "(rule)", move |v| {
                    pad_mask_bottom_msg(target, v)
                }),
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
            &SOLDER_CELLS,
            primary,
            border_c,
        ));
    }

    col
}

/// The Simple pad stack's copper as label / value rows: Shape, X-Size and
/// Y-Size, each the full form width so the values are never cut.
fn push_simple_copper_rows<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    values: &PadFormValues,
    current_shape: PadShapeChoice,
    target: PadEditTarget,
    fw: (Length, Length),
    muted: iced::Color,
    i18n: &signex_i18n::I18n,
) -> Column<Element<'a, PanelMsg>> {
    col = col.push(pad_pick_row_w(
        fw,
        t!(i18n, ids::EDITORS_PAD_SHAPE),
        PadShapeChoice::ALL,
        current_shape,
        move |c: PadShapeChoice| pad_shape_msg(target, c.to_lib()),
        muted,
    ));
    col = col.push(pad_input_row_w(
        fw,
        t!(i18n, ids::EDITORS_PAD_SIZE_X),
        "",
        pad_size_buf(values.size_x_mm),
        move |v| pad_size_x_msg(target, v),
        muted,
    ));
    col.push(pad_input_row_w(
        fw,
        t!(i18n, ids::EDITORS_PAD_SIZE_Y),
        "",
        pad_size_buf(values.size_y_mm),
        move |v| pad_size_y_msg(target, v),
        muted,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcb_canvas::layer_style::layer_name;
    use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};

    /// The pad stack's per-side rows name layers as the Layers panel does.
    #[test]
    fn the_per_side_rows_use_the_layer_catalog_names() {
        let i18n = signex_i18n::I18n::en_us();
        let rows = [
            (TOP_LAYER, PcbLayer::Copper(CopperLayer::Top)),
            (BOTTOM_LAYER, PcbLayer::Copper(CopperLayer::Bottom)),
            (
                TOP_SOLDER,
                PcbLayer::Technical(TechnicalLayer::SolderMaskTop),
            ),
            (
                BOTTOM_SOLDER,
                PcbLayer::Technical(TechnicalLayer::SolderMaskBottom),
            ),
            (TOP_PASTE, PcbLayer::Technical(TechnicalLayer::PasteTop)),
            (
                BOTTOM_PASTE,
                PcbLayer::Technical(TechnicalLayer::PasteBottom),
            ),
        ];
        for (label, layer) in rows {
            assert_eq!(label, layer_name(&i18n, layer).into_string(), "{layer:?}");
        }
    }
}
