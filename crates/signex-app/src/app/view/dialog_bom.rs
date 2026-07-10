//! BOM (Bill of Materials) preview modal — the pick-list of columns,
//! export options, and the live preview table.
//!
//! Extracted verbatim from `view/dialogs.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;
use iced::widget::{Column, Row, Space, button, column, container, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Theme};

use super::dialog_widgets::{
    close_x_button, detached_header, draggable_header, primary_button_themed, section_header,
    wrap_modal,
};
use super::dialogs::{MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE};

impl Signex {
    pub(super) fn view_bom_preview(&self) -> Element<'_, Message> {
        let modal_w = 1000.0_f32;
        let modal_h = 700.0_f32;
        let dialog = self.view_bom_preview_body_inner(true);
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::state::ModalId::BomPreview)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(
            dialog,
            offset,
            self.ui_state.window_size,
            (modal_w, modal_h),
        )
    }

    pub(super) fn view_bom_preview_body(&self) -> Element<'_, Message> {
        self.view_bom_preview_body_inner(false)
    }

    fn view_bom_preview_body_inner(&self, draggable: bool) -> Element<'_, Message> {
        use signex_output::{BomColumn, BomFormat, BomGrouping};
        let Some(ref preview) = self.document_state.bom_preview else {
            return container(Space::new()).into();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        let theme_id = self.ui_state.theme_id;

        let header_content: Element<'_, Message> = container(
            row![
                text("Bill of Materials")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(
                    Message::BomPreview(BomPreviewMsg::Close),
                    theme_id,
                    text_muted
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let _ = border_c;
        let header = if draggable {
            draggable_header(
                header_content,
                super::super::state::ModalId::BomPreview,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            detached_header(header_content, super::super::state::ModalId::BomPreview)
        };

        // Pill style mirrors the main-app right-side Properties
        // panel: subtle theme-border at rest, theme-accent border
        // when active (orange on Altium Dark, cyan on Alp Lab,
        // etc.). Active fill is the same accent at 15 % alpha so
        // the pill reads as "selected" without being a hard solid
        // fill that fights the theme.
        let accent_c = crate::styles::ti(tokens.accent);
        let row_pill = |label: String, on: bool, msg: Message| -> Element<'_, Message> {
            button(text(label).size(11).color(text_c))
                .padding([4, 10])
                .on_press(msg)
                .style(move |_: &Theme, status: button::Status| {
                    let bg = match (on, status) {
                        (true, _) => Color {
                            a: 0.15,
                            ..accent_c
                        },
                        (false, button::Status::Hovered | button::Status::Pressed) => {
                            Color::from_rgba(1.0, 1.0, 1.0, 0.10)
                        }
                        _ => Color::from_rgba(1.0, 1.0, 1.0, 0.04),
                    };
                    let border_color = if on { accent_c } else { border_c };
                    button::Style {
                        background: Some(Background::Color(bg)),
                        border: Border {
                            width: 1.0,
                            radius: 3.0.into(),
                            color: border_color,
                        },
                        text_color: text_c,
                        ..button::Style::default()
                    }
                })
                .into()
        };

        let grouping_row: Element<'_, Message> = row![
            text("Grouping:").size(11).color(text_muted),
            Space::new().width(8),
            row_pill(
                "Grouped".to_string(),
                preview.options.grouping == BomGrouping::Grouped,
                Message::BomPreview(BomPreviewMsg::SetGrouping(BomGrouping::Grouped)),
            ),
            Space::new().width(4),
            row_pill(
                "Ungrouped".to_string(),
                preview.options.grouping == BomGrouping::Ungrouped,
                Message::BomPreview(BomPreviewMsg::SetGrouping(BomGrouping::Ungrouped)),
            ),
            Space::new().width(4),
            row_pill(
                "Flat".to_string(),
                preview.options.grouping == BomGrouping::Flat,
                Message::BomPreview(BomPreviewMsg::SetGrouping(BomGrouping::Flat)),
            ),
        ]
        .align_y(iced::Alignment::Center)
        .into();

        let format_row: Element<'_, Message> = row![
            text("Format:").size(11).color(text_muted),
            Space::new().width(8),
            row_pill(
                "CSV".to_string(),
                preview.options.format == BomFormat::Csv,
                Message::BomPreview(BomPreviewMsg::SetFormat(BomFormat::Csv)),
            ),
            Space::new().width(4),
            row_pill(
                "XLSX".to_string(),
                preview.options.format == BomFormat::Xlsx,
                Message::BomPreview(BomPreviewMsg::SetFormat(BomFormat::Xlsx)),
            ),
            Space::new().width(4),
            row_pill(
                "HTML".to_string(),
                preview.options.format == BomFormat::Html,
                Message::BomPreview(BomPreviewMsg::SetFormat(BomFormat::Html)),
            ),
        ]
        .align_y(iced::Alignment::Center)
        .into();

        let toggle_row: Element<'_, Message> = row![
            row_pill(
                "Include DNP".to_string(),
                preview.options.include_dnp,
                Message::BomPreview(BomPreviewMsg::SetIncludeDnp(!preview.options.include_dnp)),
            ),
            Space::new().width(4),
            row_pill(
                "Include Not Fitted".to_string(),
                preview.options.include_not_fitted,
                Message::BomPreview(BomPreviewMsg::SetIncludeNotFitted(
                    !preview.options.include_not_fitted
                )),
            ),
        ]
        .align_y(iced::Alignment::Center)
        .into();

        // Variant picker — only shown when the active project
        // declares variants. "Base" is the no-override view.
        let variant_row: Element<'_, Message> = if preview.variants.is_empty() {
            Space::new().into()
        } else {
            let active_label = preview
                .options
                .active_variant
                .clone()
                .unwrap_or_else(|| "Base".to_string());
            let mut r = row![
                text("Variant:").size(11).color(text_muted),
                Space::new().width(8),
                row_pill(
                    "Base".to_string(),
                    preview.options.active_variant.is_none(),
                    Message::BomPreview(BomPreviewMsg::SetVariant(None)),
                ),
            ]
            .align_y(iced::Alignment::Center);
            for variant in &preview.variants {
                let is_active = active_label.as_str() == variant.as_str();
                r = r.push(Space::new().width(4));
                r = r.push(row_pill(
                    variant.clone(),
                    is_active,
                    Message::BomPreview(BomPreviewMsg::SetVariant(Some(variant.clone()))),
                ));
            }
            r.into()
        };

        // Column picker — toggles for the standard column set + any
        // custom-field column discovered in the rolled-up rows. The
        // pill state mirrors `preview.options.columns`; clicking
        // adds/removes from that Vec via `handle_bom_preview_toggle_column`.
        let mut custom_keys: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for r in &preview.table.rows {
            for k in r.custom.keys() {
                custom_keys.insert(k.clone());
            }
        }
        let column_options: Vec<(BomColumn, &'static str)> = vec![
            (BomColumn::Name, "Name"),
            (BomColumn::Description, "Description"),
            (BomColumn::Designator, "Designator"),
            (BomColumn::Value, "Value"),
            (BomColumn::Footprint, "Footprint"),
            (BomColumn::LibRef, "LibRef"),
            (BomColumn::Qty, "Qty"),
        ];
        let mut column_row = row![
            text("Columns:").size(11).color(text_muted),
            Space::new().width(8)
        ]
        .align_y(iced::Alignment::Center);
        for (col, label) in &column_options {
            let on = preview.options.columns.iter().any(|c| c == col);
            column_row = column_row.push(row_pill(
                label.to_string(),
                on,
                Message::BomPreview(BomPreviewMsg::ToggleColumn(col.clone())),
            ));
            column_row = column_row.push(Space::new().width(4));
        }
        for key in &custom_keys {
            let col = BomColumn::Custom(key.clone());
            let on = preview.options.columns.iter().any(|c| c == &col);
            column_row = column_row.push(row_pill(
                key.clone(),
                on,
                Message::BomPreview(BomPreviewMsg::ToggleColumn(col)),
            ));
            column_row = column_row.push(Space::new().width(4));
        }
        let column_row: Element<'_, Message> = scrollable(column_row)
            .direction(iced::widget::scrollable::Direction::Horizontal(
                iced::widget::scrollable::Scrollbar::new()
                    .width(0)
                    .scroller_width(0),
            ))
            .into();

        // Build the table from `preview.options.columns`. Width
        // resolution: user override (set by the column-resize
        // handle drag) takes precedence; otherwise fall back to
        // the per-`BomColumn` default below.
        let default_column_width = |c: &BomColumn| -> f32 {
            match c {
                BomColumn::Name => 140.0,
                BomColumn::Description => 220.0,
                BomColumn::Designator | BomColumn::Reference => 220.0,
                BomColumn::Value => 110.0,
                BomColumn::Footprint => 140.0,
                BomColumn::LibRef => 160.0,
                BomColumn::Qty => 50.0,
                BomColumn::Custom(_) => 120.0,
            }
        };
        let column_width = |idx: usize, c: &BomColumn| -> f32 {
            preview
                .column_widths
                .get(&idx)
                .copied()
                .unwrap_or_else(|| default_column_width(c))
        };
        let column_value = |c: &BomColumn, r: &signex_output::BomRow| -> String {
            match c {
                BomColumn::Name => r.name.clone(),
                BomColumn::Description => r.description.clone(),
                BomColumn::Designator | BomColumn::Reference => r.references.join(", "),
                BomColumn::Value => r.value.clone(),
                BomColumn::Footprint => r.footprint.clone(),
                BomColumn::LibRef => r.lib_ref.clone(),
                BomColumn::Qty => r.qty.to_string(),
                BomColumn::Custom(key) => r.custom.get(key).cloned().unwrap_or_default(),
            }
        };
        // Compute the table's full content width — gutter + 1 px
        // gutter→data divider + sum(column_width) + (n-1) inter-
        // column dividers + (n-1) resize-handle slots. Used as
        // the Fixed width on the table inner so Direction::Both
        // scrollable knows when to engage horizontal scrolling.
        let data_columns_width: f32 = preview
            .options
            .columns
            .iter()
            .enumerate()
            .map(|(idx, c)| column_width(idx, c))
            .sum();
        let n_data = preview.options.columns.len();
        let dividers_width = n_data.saturating_sub(1) as f32;
        let resize_slots_width = n_data.saturating_sub(1) as f32 * 4.0;
        let table_width: f32 =
            36.0 + 1.0 + data_columns_width + dividers_width + resize_slots_width;
        // Headers: clickable + draggable. Click cycles sort; press
        // arms a drag, release on another header drops the source
        // column at that index. Sort indicator (▲/▼) appears next to
        // the active sort column.
        let sort_arrow = |idx: usize| -> &'static str {
            match preview.sort {
                Some((c, true)) if c == idx => " ▲",
                Some((c, false)) if c == idx => " ▼",
                _ => "",
            }
        };
        // Row-number column (left) — Altium parity. No header label,
        // just shows the row position. Width 36 holds 1-9999 cleanly.
        // Header cells share `HEADER_ROW_H`; data rows share
        // `DATA_ROW_H`. Both centered vertically so text doesn't
        // float at the top of its stripe.
        const NUM_COL_WIDTH: f32 = 36.0;
        const HEADER_ROW_H: f32 = 24.0;
        const DATA_ROW_H: f32 = 22.0;
        let num_header: Element<'_, Message> = container(text(""))
            .width(Length::Fixed(NUM_COL_WIDTH))
            .height(HEADER_ROW_H)
            .into();

        let mut header_row: Row<'_, Message> = Row::new()
            .spacing(0)
            .align_y(iced::Alignment::Center)
            .push(num_header);
        // Vertical divider between row-number column and the data
        // columns so the gutter is visibly its own zone.
        header_row = header_row.push(container(Space::new()).width(1).height(HEADER_ROW_H).style(
            move |_: &Theme| container::Style {
                background: Some(Background::Color(border_c)),
                ..container::Style::default()
            },
        ));
        // Index of the last data column — the one that uses
        // Length::Fill so the table eats any leftover horizontal
        // space when the modal is wider than the sum of fixed
        // column widths.
        let last_data_col_idx = preview.options.columns.len().saturating_sub(1);
        // Only treat the press as a "real" drag once the cursor
        // has moved past 6 px from where it pressed — same
        // threshold the tab drag-ghost uses. A press-and-release
        // without motion counts as a click and never lights the
        // header up.
        const COL_DRAG_THRESHOLD_PX: f32 = 6.0;
        let cursor_x = self.interaction_state.last_mouse_pos.0;
        let active_drag = match (preview.column_drag, preview.column_drag_press_x) {
            (Some(idx), Some(ox)) if (cursor_x - ox).abs() > COL_DRAG_THRESHOLD_PX => Some(idx),
            _ => None,
        };
        let _ = preview.column_hover;
        for (idx, c) in preview.options.columns.iter().enumerate() {
            // Header bg is neutral by default. The only time a
            // column lights up is while the user is actively
            // dragging its header to reorder (cursor moved past
            // threshold) — sort doesn't get a highlight (the
            // ▲/▼ glyph already conveys that).
            let cell_bg = if active_drag == Some(idx) {
                Some(Background::Color(Color {
                    a: 0.22,
                    ..crate::styles::ti(tokens.accent)
                }))
            } else {
                None
            };
            // Header cell content: column name on the left, sort
            // arrow (when this column is the active sort) anchored
            // to the right via Space::Fill. Spreadsheet convention.
            let arrow = sort_arrow(idx).trim();
            let cell_content = row![
                text(c.header().to_string()).size(11).color(text_c),
                Space::new().width(Length::Fill),
                text(arrow.to_string()).size(11).color(text_c),
            ]
            .align_y(iced::Alignment::Center);
            // All headers use Fixed widths — needed for the
            // Direction::Both scrollable to know its content
            // size and engage horizontal scroll when columns
            // overflow the viewport. The previous Fill-last
            // experiment killed horizontal scrolling because
            // Both-direction collapses Fill children to zero.
            let header_cell_width = Length::Fixed(column_width(idx, c));
            let _ = last_data_col_idx;
            let cell = container(cell_content)
                .width(header_cell_width)
                .height(HEADER_ROW_H)
                .padding([0, 6])
                .align_y(iced::alignment::Vertical::Center)
                .style(move |_: &Theme| container::Style {
                    background: cell_bg,
                    ..container::Style::default()
                });
            // mouse_area gives us press (drag-start) + release
            // + on_enter / on_exit. on_release fires on the
            // press-source widget; the drop logic uses
            // `column_hover` (set via on_enter on the target) to
            // resolve where the cursor was at release time.
            let header_cell: Element<'_, Message> = iced::widget::mouse_area(cell)
                .on_press(Message::BomPreview(BomPreviewMsg::ColumnDragStart(idx)))
                .on_release(if preview.column_drag == Some(idx) {
                    Message::BomPreview(BomPreviewMsg::SortColumn(idx))
                } else {
                    Message::BomPreview(BomPreviewMsg::ColumnDragDrop(idx))
                })
                .on_enter(Message::BomPreview(BomPreviewMsg::ColumnHoverEnter(idx)))
                .on_exit(Message::BomPreview(BomPreviewMsg::ColumnHoverExit(idx)))
                .interaction(iced::mouse::Interaction::Pointer)
                .into();
            header_row = header_row.push(header_cell);
            if idx + 1 < preview.options.columns.len() {
                // 1 px theme-border vertical divider + 4 px
                // transparent resize handle. No accent bg in
                // either — they sit between columns and shouldn't
                // bleed sort highlights into neighbouring cells.
                header_row = header_row.push(container(Space::new()).width(1).height(24).style(
                    move |_: &Theme| container::Style {
                        background: Some(Background::Color(border_c)),
                        ..container::Style::default()
                    },
                ));
                let resize_handle: Element<'_, Message> =
                    iced::widget::mouse_area(container(Space::new()).width(4).height(HEADER_ROW_H))
                        .on_press(Message::BomPreview(BomPreviewMsg::ColumnResizeStart(idx)))
                        .on_release(Message::BomPreview(BomPreviewMsg::ColumnResizeEnd))
                        .interaction(iced::mouse::Interaction::ResizingHorizontally)
                        .into();
                header_row = header_row.push(resize_handle);
            }
        }
        // Header strip locked to `table_width` so it scrolls in
        // lockstep with the body rows when the user pans
        // horizontally.
        let table_header_el: Element<'_, Message> = container(header_row)
            .style(crate::styles::toolbar_strip(tokens))
            .width(Length::Fixed(table_width.max(100.0)))
            .into();

        // Sort the row order if a sort spec is set. We sort indexes
        // into preview.table.rows so we can borrow rows by reference
        // without cloning the BomRow Vec. Numeric columns (Qty) sort
        // numerically; the rest sort case-insensitively.
        let mut row_order: Vec<usize> = (0..preview.table.rows.len()).collect();
        if let Some((sort_idx, asc)) = preview.sort
            && let Some(sort_col) = preview.options.columns.get(sort_idx)
        {
            let key = |r: &signex_output::BomRow| column_value(sort_col, r);
            row_order.sort_by(|&a, &b| {
                let ra = &preview.table.rows[a];
                let rb = &preview.table.rows[b];
                let cmp = match sort_col {
                    BomColumn::Qty => ra.qty.cmp(&rb.qty),
                    _ => key(ra)
                        .to_ascii_lowercase()
                        .cmp(&key(rb).to_ascii_lowercase()),
                };
                if asc { cmp } else { cmp.reverse() }
            });
        }

        let mut rows: Vec<Element<'_, Message>> = Vec::with_capacity(preview.table.rows.len());
        for (visible_idx, &row_idx) in row_order.iter().enumerate() {
            let r = &preview.table.rows[row_idx];
            let alt_bg = if visible_idx % 2 == 0 {
                Color::from_rgba(1.0, 1.0, 1.0, 0.0)
            } else {
                Color::from_rgba(1.0, 1.0, 1.0, 0.025)
            };
            // Single-line cell: rely on `wrapping(None)` +
            // `clip(true)` to cut overflow at the right edge.
            // Manual char-count truncation was over-trimming —
            // hard to estimate per-glyph width across themes /
            // fonts, and the "…" suffix never showed when our
            // estimate was too wide. iced 0.14 lacks a built-in
            // ellipsis primitive; clipping is the honest choice.
            const CELL_TEXT_SIZE: f32 = 11.0;
            let cell = |s: String, width: Length, _truncate_at: f32| -> Element<'_, Message> {
                container(
                    text(s)
                        .size(CELL_TEXT_SIZE)
                        .color(text_c)
                        .wrapping(iced::widget::text::Wrapping::None),
                )
                .width(width)
                .height(DATA_ROW_H)
                .padding([0, 6])
                .align_y(iced::alignment::Vertical::Center)
                .clip(true)
                .into()
            };
            // Leftmost row-number gutter cell — muted text, right-aligned.
            let num_cell: Element<'_, Message> = container(
                text((visible_idx + 1).to_string())
                    .size(CELL_TEXT_SIZE)
                    .color(text_muted),
            )
            .width(Length::Fixed(36.0))
            .height(DATA_ROW_H)
            .padding([0, 6])
            .align_x(iced::alignment::Horizontal::Right)
            .align_y(iced::alignment::Vertical::Center)
            .into();
            // Body rows: subtle 1 px vertical column dividers —
            // theme-border at 30 % alpha so they read as faint
            // groove lines without competing with the header
            // dividers above. Same ordering as the header so
            // columns line up pixel-for-pixel.
            let subtle_divider_color = Color { a: 0.3, ..border_c };
            let body_divider = move || -> Element<'_, Message> {
                container(Space::new())
                    .width(1)
                    .height(DATA_ROW_H)
                    .style(move |_: &Theme| container::Style {
                        background: Some(Background::Color(subtle_divider_color)),
                        ..container::Style::default()
                    })
                    .into()
            };
            let mut row_inner: Row<'_, Message> = Row::new().spacing(0).push(num_cell);
            row_inner = row_inner.push(body_divider());
            for (col_idx, c) in preview.options.columns.iter().enumerate() {
                // Every column uses Fixed widths so the
                // Direction::Both scrollable can compute total
                // content width and turn on horizontal scroll
                // when columns overflow the viewport.
                let w = column_width(col_idx, c);
                let (cell_width, truncate_at) = (Length::Fixed(w), w);
                row_inner = row_inner.push(cell(column_value(c, r), cell_width, truncate_at));
                if col_idx + 1 < preview.options.columns.len() {
                    row_inner = row_inner.push(body_divider());
                    // 4 px transparent spacer to mirror the
                    // header's resize-handle width — keeps
                    // columns pixel-aligned with the header.
                    row_inner = row_inner.push(Space::new().width(4));
                }
            }
            let row_el = container(row_inner)
                .width(Length::Fixed(table_width.max(100.0)))
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(alt_bg)),
                    ..container::Style::default()
                });
            rows.push(row_el.into());
        }
        // Header + rows live inside the same vertical column so the
        // horizontal scroll moves them together. Without this, the
        // header was wrapped separately and stayed fixed while the
        // body rows scrolled past it.
        // Both-direction scrollable. Header + rows live inside
        // the same fixed-width column so horizontal scrolling
        // moves them together (the user pans columns left/right
        // without the header staying in place). The 16 px
        // bottom buffer keeps the last data row above the
        // horizontal scrollbar's reserved zone — without it,
        // row N would clip when the scrollable's H bar paints
        // inside the viewport at the bottom.
        let table_inner: Element<'_, Message> = column![
            table_header_el,
            column(rows).spacing(0),
            Space::new().height(16),
        ]
        .width(Length::Fixed(table_width.max(100.0)))
        .into();
        let body = scrollable(table_inner)
            .direction(iced::widget::scrollable::Direction::Both {
                vertical: iced::widget::scrollable::Scrollbar::default(),
                horizontal: iced::widget::scrollable::Scrollbar::default(),
            })
            .width(Length::Fill)
            .height(Length::Fill);

        let active_variant_label = preview
            .options
            .active_variant
            .clone()
            .unwrap_or_else(|| "Base".to_string());
        let row_count = preview.table.rows.len();
        let total_count = preview.table.rows.len();
        let status_label = format!("{} of {} lines visible", row_count, total_count);
        let variant_label = format!("Current variant: {}", active_variant_label);

        // Variant dropdown for the top toolbar — placeholder in the
        // header strip area, sits to the left. (i) info badge sits
        // on the right.
        let variant_dropdown: Element<'_, Message> = if preview.variants.is_empty() {
            text(format!("Variant: {}", active_variant_label))
                .size(11)
                .color(text_muted)
                .into()
        } else {
            container(
                row![
                    Space::new().width(Length::Fill),
                    text(active_variant_label.clone()).size(11).color(text_c),
                    Space::new().width(Length::Fill),
                    text("\u{25BE}").size(10).color(text_c),
                ]
                .align_y(iced::Alignment::Center),
            )
            .width(160)
            .height(24)
            .padding([0, 10])
            .style(move |_: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.04,
                ))),
                border: iced::Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border_c,
                },
                ..iced::widget::container::Style::default()
            })
            .into()
        };
        let info_badge: Element<'_, Message> =
            container(text("i").size(11).color(iced::Color::WHITE))
                .width(20)
                .height(20)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .style(move |_: &iced::Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(iced::Color {
                        a: 0.7,
                        ..crate::styles::ti(tokens.accent)
                    })),
                    border: iced::Border {
                        width: 0.0,
                        radius: 10.0.into(),
                        color: iced::Color::TRANSPARENT,
                    },
                    ..iced::widget::container::Style::default()
                })
                .into();
        let toolbar_strip = container(
            row![
                variant_dropdown,
                Space::new().width(Length::Fill),
                info_badge,
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([8, 14])
        .width(Length::Fill)
        .style(crate::styles::toolbar_strip(tokens));

        // Properties sidebar — General / Columns tabs, collapsible
        // sections inside.
        use crate::app::state::BomSidebarTab;
        let sidebar_tab = preview.sidebar_tab;
        let tab_pill = |label: &'static str, target: BomSidebarTab| -> Element<'_, Message> {
            let on = sidebar_tab == target;
            button(text(label.to_string()).size(11).color(text_c))
                .padding([4, 12])
                .on_press(Message::BomPreview(BomPreviewMsg::SetSidebarTab(target)))
                .style(move |_: &iced::Theme, status: button::Status| {
                    let bg = match (on, status) {
                        (true, _) => iced::Color {
                            a: 0.15,
                            ..accent_c
                        },
                        (false, button::Status::Hovered) => {
                            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06)
                        }
                        _ => iced::Color::TRANSPARENT,
                    };
                    let border_color = if on { accent_c } else { border_c };
                    button::Style {
                        background: Some(iced::Background::Color(bg)),
                        border: iced::Border {
                            width: 1.0,
                            radius: 3.0.into(),
                            color: border_color,
                        },
                        text_color: text_c,
                        ..button::Style::default()
                    }
                })
                .into()
        };

        let general_pane: Element<'_, Message> = column![
            section_header("BOM Items", text_muted),
            container(
                column![
                    grouping_row,
                    Space::new().height(6),
                    toggle_row,
                    Space::new().height(6),
                    variant_row,
                ]
                .spacing(0),
            )
            .padding([8, 12]),
            Space::new().height(8),
            section_header("Export Options", text_muted),
            container(column![format_row].spacing(0),).padding([8, 12]),
        ]
        .spacing(0)
        .into();

        // Columns tab — vertical list with a tick checkbox per row,
        // mirroring Altium's Columns table. Same standard column
        // set + any custom-field column discovered in the rolled
        // rows. Each row is clickable and toggles via
        // `Message::BomPreview(BomPreviewMsg::ToggleColumn)` — the existing handler
        // already does add/remove on the Vec.
        let _ = column_row; // pill row obsoleted by the list layout
        let mut col_list: Column<'_, Message> = Column::new().spacing(0);
        col_list = col_list.push(section_header("Columns", text_muted));
        let mut list_items: Column<'_, Message> = Column::new().spacing(0);
        let render_col_row = |col: BomColumn, label: String, on: bool| -> Element<'_, Message> {
            let pip_bg = if on {
                Color::from_rgb(0.00, 0.47, 0.84)
            } else {
                Color::from_rgba(1.0, 1.0, 1.0, 0.04)
            };
            let inner = if on {
                text("\u{2713}").size(9).color(Color::WHITE)
            } else {
                text(" ").size(9)
            };
            let pip = container(inner)
                .width(14)
                .height(14)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(pip_bg)),
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: border_c,
                    },
                    ..container::Style::default()
                });
            button(
                row![
                    pip,
                    Space::new().width(8),
                    text(label).size(11).color(text_c),
                    Space::new().width(Length::Fill),
                ]
                .align_y(iced::Alignment::Center),
            )
            .on_press(Message::BomPreview(BomPreviewMsg::ToggleColumn(col)))
            .padding([3, 12])
            .width(Length::Fill)
            .style(move |_: &Theme, status: button::Status| {
                let bg = match status {
                    button::Status::Hovered | button::Status::Pressed => {
                        Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04)))
                    }
                    _ => None,
                };
                button::Style {
                    background: bg,
                    border: Border::default(),
                    text_color: text_c,
                    ..button::Style::default()
                }
            })
            .into()
        };
        for (col, label) in &column_options {
            let on = preview.options.columns.iter().any(|c| c == col);
            list_items = list_items.push(render_col_row(col.clone(), label.to_string(), on));
        }
        for key in &custom_keys {
            let col = BomColumn::Custom(key.clone());
            let on = preview.options.columns.iter().any(|c| c == &col);
            list_items = list_items.push(render_col_row(col, key.clone(), on));
        }
        col_list = col_list.push(container(list_items).padding([4, 0]));
        let columns_pane: Element<'_, Message> = col_list.into();

        let sidebar_body: Element<'_, Message> = match sidebar_tab {
            BomSidebarTab::General => general_pane,
            BomSidebarTab::Columns => columns_pane,
        };
        let sidebar = column![
            container(text("Properties").size(13).color(text_c))
                .padding([10, 12])
                .width(Length::Fill),
            container(
                row![
                    tab_pill("General", BomSidebarTab::General),
                    Space::new().width(4),
                    tab_pill("Columns", BomSidebarTab::Columns),
                ]
                .align_y(iced::Alignment::Center),
            )
            .padding([0, 12]),
            Space::new().height(8),
            scrollable(sidebar_body)
                .width(Length::Fill)
                .height(Length::Fill),
        ]
        .spacing(0);

        // Main row: scrollable table + 1 px divider + sidebar.
        // Body now uses nested scrollables (vertical inside
        // horizontal) so the horizontal bar lives below the last
        // row instead of clipping it — no bottom-padding hack
        // needed.
        let main_row = row![
            container(body)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding([6, 8]),
            container(Space::new())
                .width(1)
                .height(Length::Fill)
                .style(move |_: &iced::Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(border_c)),
                    ..iced::widget::container::Style::default()
                }),
            container(sidebar)
                .width(Length::Fixed(320.0))
                .height(Length::Fill),
        ]
        .height(Length::Fill);

        // Combined footer: status text on the left + Export button
        // on the right, in ONE strip with bottom-rounded radius.
        // Dropping the separate status / button rows removes the
        // empty band between them and tightens the modal's
        // bottom chrome.
        let footer = container(
            row![
                text(status_label).size(10).color(text_muted),
                Space::new().width(12),
                text("|").size(10).color(text_muted),
                Space::new().width(12),
                text(variant_label).size(10).color(text_muted),
                Space::new().width(Length::Fill),
                primary_button_themed(
                    "Export\u{2026}",
                    Some(Message::BomPreview(BomPreviewMsg::Export)),
                    border_c,
                    Some(crate::styles::ti(tokens.accent)),
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([6, 14])
        .width(Length::Fill)
        .style(crate::styles::modal_footer_strip(tokens));

        // 1 px theme-border line — same style the main app uses
        // between the menu bar and the document tab strip
        // (`crate::styles::chrome_separator`). The inner Space is
        // sized Fill on both axes so iced doesn't shrink the
        // container to zero height in some layout scenarios.
        let title_separator = container(Space::new().width(Length::Fill).height(Length::Fill))
            .width(Length::Fill)
            .height(1)
            .style(crate::styles::chrome_separator(tokens));

        let dialog = container(
            column![header, title_separator, toolbar_strip, main_row, footer]
                .spacing(0)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }
}
