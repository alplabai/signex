//! Properties surface for a single selected schematic element.
//!
//! Draws the header, dispatches to the per-kind builder (`symbol`,
//! `symbol_field`, `label`, `text_note`, `power_port`, `generic`, plus the
//! `drawing` and `child_sheet` surfaces), then the shared wire Style and
//! Lock sections and the status bar.

use super::super::*;
use super::rows::PanelStyle;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;

#[cfg(test)]
use super::rows::{
    h_align_from_row, justify_h_row, justify_v_row, pick_list_degrees, rotation_degrees,
};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

pub(in crate::panels) fn view_selected_element_properties<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    let palette = PanelPalette::from_tokens(&ctx.tokens);
    let PanelPalette { panel_bg, .. } = palette;
    let st = PanelStyle {
        muted,
        primary,
        border_c,
        palette,
    };
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);

    let elem_type = ctx
        .selection_info
        .iter()
        .find(|(k, _)| k == "Type")
        .map(|(_, v)| v.as_str())
        .unwrap_or("Object");

    let uuid = ctx.selected_uuid;
    let selected_kind = ctx.selected_kind;
    let get = |key: &str| -> String {
        ctx.selection_info
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };

    // ── Editable properties based on element type ──
    // Power ports get their own Altium-style panel; regular symbols keep the
    // existing designator/value/footprint layout.
    let is_power_port = elem_type == "Power Port";
    let is_placed_part =
        !is_power_port && selected_kind == Some(signex_types::schematic::SelectedKind::Symbol);

    // ── Header ──
    col = col.push(super::header::selection_header(
        ctx,
        elem_type,
        is_placed_part,
        &get("Description"),
        primary,
    ));
    col = col.push(thin_sep(border_c));

    if is_power_port && let Some(id) = uuid {
        col = super::power_port::push_power_port_sections(col, ctx, id, st);
        return scrollable(col).width(Length::Fill).boxed();
    }

    match selected_kind {
        Some(signex_types::schematic::SelectedKind::Symbol) => {
            col = super::symbol::push_symbol_sections(col, ctx, uuid, st);
        }
        Some(signex_types::schematic::SelectedKind::SymbolRefField)
        | Some(signex_types::schematic::SelectedKind::SymbolValField) => {
            col = super::symbol_field::push_symbol_field_sections(col, ctx, uuid, st);
        }
        Some(signex_types::schematic::SelectedKind::Label) => {
            col = super::label::push_label_sections(col, ctx, uuid, st);
        }
        Some(signex_types::schematic::SelectedKind::TextNote) => {
            col = super::text_note::push_text_note_sections(col, ctx, uuid, st);
        }
        Some(signex_types::schematic::SelectedKind::Drawing) => {
            col = col.push(view_drawing_properties(ctx, muted, primary, border_c));
        }
        Some(signex_types::schematic::SelectedKind::ChildSheet) => {
            col = col.push(view_child_sheet_properties(ctx, muted, primary, border_c));
        }
        _ => {
            col = super::generic::push_generic_sections(col, ctx, st);
        }
    }

    // ── Wire style ──: width preset and own colour, undoable.
    if let (Some(id), Some((width, color))) = (uuid, ctx.selected_wire_style) {
        let picker = ctx.wire_color_picker;
        col = col.push(
            ctx.collapsible(
                "sel_wire_style",
                "Style",
                &ctx.collapsed_sections,
                move || {
                    Column::new()
                        .spacing(0)
                        .width(Length::Fill)
                        .push(crate::panels::wire_width_row(
                            width,
                            move |preset| PanelMsg::EditWireWidth(id, preset),
                            muted,
                        ))
                        .push(crate::panels::wire_color_row(
                            color,
                            picker,
                            std::rc::Rc::new(move |c| PanelMsg::EditWireColor(id, c)),
                            muted,
                            border_c,
                        ))
                },
            )
            .boxed(),
        );
    }

    // ── Lock ── (#1573): wires, labels, text notes and drawings.
    if let (Some(id), Some(kind)) = (uuid, selected_kind)
        && matches!(
            kind,
            signex_types::schematic::SelectedKind::Wire
                | signex_types::schematic::SelectedKind::Label
                | signex_types::schematic::SelectedKind::TextNote
                | signex_types::schematic::SelectedKind::Drawing
        )
    {
        let locked = get("Locked") == "Yes";
        let item = signex_types::schematic::SelectedItem::new(id, kind);
        col = col.push(
            ctx.collapsible("sel_state", "State", &ctx.collapsed_sections, move || {
                Column::new()
                    .spacing(0)
                    .width(Length::Fill)
                    .push(form_check_row(
                        "Locked",
                        locked,
                        PanelMsg::ToggleItemLocked(item),
                        muted,
                        panel_bg,
                    ))
            })
            .boxed(),
        );
    }

    // ── Status bar ──
    col = col.push(Space::new().height(8.0).boxed());
    col = col.push(thin_sep(border_c));
    col = col.push(
        container(
            text(signex_i18n::t!(
                ctx.i18n,
                signex_i18n::ids::PANELS_PROPERTIES_ONE_OBJECT_SELECTED
            ))
            .size(TS.caption)
            .color(muted),
        )
        .padding([SP.s, SP.m])
        .boxed(),
    );

    scrollable(col).width(Length::Fill).boxed()
}

#[cfg(test)]
mod tests {
    use super::{
        h_align_from_row, justify_h_row, justify_v_row, pick_list_degrees, rotation_degrees,
    };
    use signex_types::schematic::HAlign;

    /// #828 -- a non-finite rotation text never reaches the element.
    #[test]
    fn non_finite_rotation_reads_as_zero() {
        for bad in ["nan", "NaN", "inf", "-inf", "1e999", "inf°"] {
            assert_eq!(rotation_degrees(bad), 0.0, "{bad:?}");
            assert_eq!(pick_list_degrees(&format!("{bad} Degrees")), 0.0, "{bad:?}");
        }
    }

    #[test]
    fn ordinary_rotation_still_reads() {
        assert_eq!(rotation_degrees("90°"), 90.0);
        assert_eq!(rotation_degrees(" 270 "), 270.0);
        assert_eq!(pick_list_degrees("180 Degrees"), 180.0);
        assert_eq!(pick_list_degrees(""), 0.0);
    }

    /// #2024 - a left-justified label shows Left: its row, as the engine
    /// writes it, reads back as `HAlign::Left` through the shared key.
    #[test]
    fn a_left_justified_label_row_reads_back_as_left() {
        use signex_types::schematic::{
            Label, LabelType, Point, SchematicSheet, SelectedItem, SelectedKind,
        };
        let label = Label::new("NET", Point::new(0.0, 0.0), LabelType::Net);
        let id = label.uuid;
        let engine = signex_engine::Engine::new(SchematicSheet {
            labels: vec![label],
            ..Default::default()
        })
        .expect("engine");
        let details = engine
            .describe_single_selection(&[SelectedItem::new(id, SelectedKind::Label)])
            .expect("details");
        let row = justify_h_row(&details.info);
        assert_eq!(row, "Left", "the row the panel shows");
        assert_eq!(h_align_from_row(&row), HAlign::Left);
        assert_eq!(h_align_from_row("Right"), HAlign::Right);
        assert_eq!(h_align_from_row("Center"), HAlign::Center);
    }

    /// #2024 - a text note's Properties rows carry its justification, read
    /// from the same keys the engine writes them under.
    #[test]
    fn a_text_note_row_carries_its_justification() {
        use signex_types::schematic::{
            Point, SchematicSheet, SelectedItem, SelectedKind, TextNote, VAlign,
        };
        let note = TextNote {
            uuid: uuid::Uuid::now_v7(),
            text: "note".into(),
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: HAlign::Left,
            justify_v: VAlign::Top,
            locked: false,
        };
        let id = note.uuid;
        let engine = signex_engine::Engine::new(SchematicSheet {
            text_notes: vec![note],
            ..Default::default()
        })
        .expect("engine");
        let details = engine
            .describe_single_selection(&[SelectedItem::new(id, SelectedKind::TextNote)])
            .expect("details");
        assert_eq!(justify_h_row(&details.info), "Left");
        assert_eq!(justify_v_row(&details.info), "Top");
    }

    /// The text a laid-out Properties view shows for a selection of `kind`
    /// whose info rows are `info`.
    fn shown_texts(
        kind: signex_types::schematic::SelectedKind,
        info: Vec<(String, String)>,
    ) -> Vec<String> {
        use super::view_selected_element_properties;
        let (app, _task) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.selected_kind = Some(kind);
        ctx.selected_uuid = Some(uuid::Uuid::now_v7());
        ctx.selection_info = info;
        let view = view_selected_element_properties(
            &ctx,
            iced::Color::WHITE,
            iced::Color::WHITE,
            iced::Color::BLACK,
        );
        crate::layout_probe::lay_out_texts(view, iced::Size::new(400.0, 4000.0))
            .into_iter()
            .map(|t| t.text)
            .collect()
    }

    /// What the Label view looks like drawn, with its justification row set
    /// to `row`: the anchor picker highlights the cell the row selects.
    fn label_pixels(row: &str) -> Vec<u8> {
        use super::view_selected_element_properties;
        let (app, _task) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.selected_kind = Some(signex_types::schematic::SelectedKind::Label);
        ctx.selected_uuid = Some(uuid::Uuid::nil());
        ctx.selection_info = vec![
            ("Rotation".to_string(), "0".to_string()),
            (
                signex_engine::info_keys::JUSTIFY_H.to_string(),
                row.to_string(),
            ),
        ];
        let view = view_selected_element_properties(
            &ctx,
            iced::Color::WHITE,
            iced::Color::WHITE,
            iced::Color::BLACK,
        );
        crate::layout_probe::draw_pixels(view, iced::Size::new(400.0, 1200.0))
    }

    /// #2024 - the label view selects the anchor cell its justification row
    /// names: a Left row and a Right row draw differently. A view that read
    /// the row under another key would see neither and draw both the same.
    #[test]
    fn the_label_view_selects_the_cell_its_justification_row_names() {
        use signex_types::schematic::HAlign;
        let window = (400_usize, 1200_usize);
        let (left, right) = (label_pixels("Left"), label_pixels("Right"));
        let (left_grid, right_grid) = (anchor_grid(HAlign::Left), anchor_grid(HAlign::Right));
        assert_ne!(left_grid.pixels, right_grid.pixels, "the references differ");
        // Each row's view holds the picker drawn for its own alignment far
        // better than the other one's: a swapped row reading fails both.
        let score = |block: &Block, frame: &[u8]| block.best_match(frame, window.0);
        let (ll, lr) = (score(&left_grid, &left), score(&right_grid, &left));
        let (rr, rl) = (score(&right_grid, &right), score(&left_grid, &right));
        // The picker is laid out in a column with other controls, so the match
        // is partial; the two pickers differ by two cells, a gap of about 0.17.
        assert!(ll > lr + 0.1, "Left row: Left {ll}, Right {lr}");
        assert!(rr > rl + 0.1, "Right row: Right {rr}, Left {rl}");
    }

    /// The anchor picker drawn alone for one alignment, cropped to its
    /// non-background pixels.
    struct Block {
        pixels: Vec<u8>,
        width: usize,
        height: usize,
    }

    fn anchor_grid(h: signex_types::schematic::HAlign) -> Block {
        let (app, _task) = crate::app::Signex::new();
        let ctx = &app.document_state.panel_ctx;
        let palette = crate::panels::palette::PanelPalette::from_tokens(&ctx.tokens);
        let grid =
            crate::panels::justification_grid(uuid::Uuid::nil(), 0.0, h, palette, ctx.theme_id);
        let (w, hgt) = (120_usize, 120_usize);
        let data = crate::layout_probe::draw_pixels(grid, iced::Size::new(w as f32, hgt as f32));
        let lit = |x: usize, y: usize| data[(y * w + x) * 4..(y * w + x) * 4 + 3] != [0, 0, 0];
        let xs: Vec<usize> = (0..w).filter(|&x| (0..hgt).any(|y| lit(x, y))).collect();
        let ys: Vec<usize> = (0..hgt).filter(|&y| (0..w).any(|x| lit(x, y))).collect();
        let (x0, x1) = (xs[0], xs[xs.len() - 1]);
        let (y0, y1) = (ys[0], ys[ys.len() - 1]);
        let mut pixels = Vec::new();
        for y in y0..=y1 {
            pixels.extend_from_slice(&data[(y * w + x0) * 4..(y * w + x1 + 1) * 4]);
        }
        Block {
            pixels,
            width: x1 - x0 + 1,
            height: y1 - y0 + 1,
        }
    }

    impl Block {
        /// The best fraction of this block's pixels that equal (within a few
        /// levels) those of `frame`, `frame_width` px wide, over every
        /// position. Edges are anti-aliased, so an exact match is too strict.
        fn best_match(&self, frame: &[u8], frame_width: usize) -> f32 {
            let rows = frame.len() / 4 / frame_width;
            // The fraction of every `step`-th pixel of the block equal at (x, y).
            let at = |x: usize, y: usize, step: usize| {
                let (mut same, mut seen) = (0usize, 0usize);
                for r in (0..self.height).step_by(step) {
                    for c in (0..self.width).step_by(step) {
                        let a = ((y + r) * frame_width + x + c) * 4;
                        let b = (r * self.width + c) * 4;
                        seen += 1;
                        same += usize::from(
                            (0..3).all(|k| frame[a + k].abs_diff(self.pixels[b + k]) <= 6),
                        );
                    }
                }
                same as f32 / seen as f32
            };
            // Coarse over every 4th position on a sampled block, then the
            // full block around the best of them.
            let (mut best_at, mut best) = ((0, 0), -1.0_f32);
            for y in (0..=rows - self.height).step_by(4) {
                for x in (0..=frame_width - self.width).step_by(4) {
                    let score = at(x, y, 4);
                    if score > best {
                        (best_at, best) = ((x, y), score);
                    }
                }
            }
            let mut refined = 0.0_f32;
            for y in best_at.1.saturating_sub(4)..=(best_at.1 + 4).min(rows - self.height) {
                for x in best_at.0.saturating_sub(4)..=(best_at.0 + 4).min(frame_width - self.width)
                {
                    refined = refined.max(at(x, y, 1));
                }
            }
            refined
        }
    }

    fn justify_info() -> Vec<(String, String)> {
        vec![
            (
                signex_engine::info_keys::JUSTIFY_H.to_string(),
                "Lefty".to_string(),
            ),
            (
                signex_engine::info_keys::JUSTIFY_V.to_string(),
                "Topsy".to_string(),
            ),
        ]
    }

    /// #2024 - the symbol-field and text-note views show the justification
    /// rows under the keys the engine writes.
    #[test]
    fn the_field_and_note_views_show_the_justification_rows() {
        use signex_types::schematic::SelectedKind;
        for kind in [SelectedKind::SymbolRefField, SelectedKind::TextNote] {
            let texts = shown_texts(kind, justify_info());
            for want in ["Lefty", "Topsy"] {
                assert!(texts.iter().any(|t| t == want), "{kind:?} lacks {want}");
            }
        }
    }
}
