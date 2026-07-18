//! BOM preview — the spreadsheet-style data grid (row-number gutter,
//! draggable/resizable/sortable column headers, and the scrollable body
//! rows). Extracted from `dialogs/bom.rs` (ADR-0001, issue #164) as pure
//! code motion — the child-push order inside every row/column is preserved
//! byte-for-byte, so the rendered table is pixel-identical.

use super::super::*;
use iced::widget::{Row, Space, column, container, row, scrollable, text};
use iced::{Background, Color, Element, Length, Theme};

impl Signex {
    /// Build the scrollable BOM data grid (header strip + data rows) for the
    /// active preview. Returns the `scrollable` body the modal drops into its
    /// main row.
    pub(super) fn bom_table(&self) -> Element<'_, Message> {
        use signex_output::BomColumn;
        let Some(ref preview) = self.document_state.bom_preview else {
            return container(Space::new()).into();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
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
        body.into()
    }
}
