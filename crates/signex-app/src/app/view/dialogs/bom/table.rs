//! BOM dialog — the data grid: a sticky header of sortable, draggable and
//! resizable column heads over the scrolling lines, each line led by a
//! row-number gutter that carries its validation flag.

use super::super::*;
use iced::widget::{Row, Space, column, container, mouse_area, row, scrollable};
use iced::{Background, Color, Element, Length, Theme, Widget};
use signex_i18n::UserText;
use signex_output::{BomColumn, BomIssueSeverity, BomRow};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, text};

use crate::app::state::BomPreviewState;
use crate::styles::tokens::resolve;
use crate::styles::{ChromeMetrics, HAIRLINE, Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Height of the header and of every line (rule included), as drawn: a
/// field tall.
const ROW_H: f32 = ChromeMetrics::DEFAULT.field;
/// Width of the row-number gutter, a column width fixed by the m112 mockup.
const GUTTER_W: f32 = 34.0;
/// Width of the flag edge at the left of a flagged line's gutter.
const FLAG_EDGE: f32 = SP.xs;
/// Width of the resize handle between two column heads; lines leave the
/// same gap so cells stay under their heads.
const HANDLE_W: f32 = SP.s + SP.xs / 2.0;
/// A header press becomes a drag once the cursor has moved this far, the
/// same threshold the tab drag ghost uses; a press-and-release without
/// motion is a click (sort).
const COL_DRAG_THRESHOLD_PX: f32 = 6.0;
/// Strength of a flagged line's soft tint: the drawn 70 % mix.
const FLAG_FILL_ALPHA: f32 = 0.7;

/// First designator of a BOM row — the key the export pipeline orders rows by,
/// so the preview's Designator sort matches the exported file exactly.
fn first_reference(row: &BomRow) -> &str {
    row.references.first().map(String::as_str).unwrap_or("")
}

/// Rendered text of one cell — also the sort key for the text columns. The
/// same text every export writer emits.
fn column_value(c: &BomColumn, r: &BomRow) -> String {
    signex_output::bom::cell_text(r, c)
}

/// Columns of numbers: right-aligned, in the mono face.
fn is_numeric(c: &BomColumn) -> bool {
    matches!(
        c,
        BomColumn::Qty
            | BomColumn::UnitPrice
            | BomColumn::BuyQty
            | BomColumn::ExtendedPrice
            | BomColumn::Stock
    )
}

/// Columns set in the mono face: the numbers and the footprint name.
fn is_mono(c: &BomColumn) -> bool {
    is_numeric(c) || matches!(c, BomColumn::Footprint)
}

/// Default on-screen width of a column, in px, until the user resizes it.
pub(in crate::app) fn default_column_width(c: &BomColumn) -> f32 {
    match c {
        BomColumn::Name => 140.0,
        BomColumn::Description => 220.0,
        BomColumn::Designator | BomColumn::Reference => 128.0,
        BomColumn::Value => 74.0,
        BomColumn::Footprint => 118.0,
        BomColumn::LibRef => 160.0,
        BomColumn::Qty => 44.0,
        BomColumn::Custom(_) => 120.0,
        BomColumn::UnitPrice | BomColumn::ExtendedPrice => 74.0,
        BomColumn::BuyQty | BomColumn::Stock => 66.0,
        BomColumn::Currency => 70.0,
        BomColumn::Distributor | BomColumn::PriceAge => 100.0,
    }
}

/// Row indexes into `rows` in display order.
///
/// Indexes rather than rows so the caller borrows without cloning the `BomRow`
/// vec. Qty sorts numerically; a designator column sorts with the *same*
/// natural order the export pipeline applies (`signex_bom::build_table`), so
/// the modal and the CSV/HTML/XLSX the user exports next never disagree — a
/// plain `str` compare here would show R1, R10, R2 over an export reading
/// R1, R2, R10. Value sorts by parsed quantity ahead of its text; every other column sorts case-insensitively on its cell text.
pub(in crate::app) fn sorted_row_order(
    rows: &[BomRow],
    columns: &[BomColumn],
    sort: Option<(usize, bool)>,
) -> Vec<usize> {
    let mut row_order: Vec<usize> = (0..rows.len()).collect();
    let Some((sort_idx, asc)) = sort else {
        return row_order;
    };
    let Some(sort_col) = columns.get(sort_idx) else {
        return row_order;
    };
    match sort_col {
        // Numbers sort as numbers; a row without one (an unpriced row in a
        // price column) sorts after every row that has one, either way.
        BomColumn::Qty
        | BomColumn::UnitPrice
        | BomColumn::BuyQty
        | BomColumn::ExtendedPrice
        | BomColumn::Stock => row_order.sort_by(|&a, &b| {
            let number = |i: usize| signex_output::bom::cell_number(&rows[i], sort_col);
            match (number(a), number(b)) {
                (Some(x), Some(y)) => {
                    let cmp = x.total_cmp(&y);
                    if asc { cmp } else { cmp.reverse() }
                }
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        }),
        BomColumn::Designator | BomColumn::Reference => row_order.sort_by(|&a, &b| {
            let cmp = signex_types::designator::compare_references(
                first_reference(&rows[a]),
                first_reference(&rows[b]),
            );
            if asc { cmp } else { cmp.reverse() }
        }),
        // Parsed values first, by (unit family, base value), then the rest
        // as natural text. The parse is sort-only and nothing keeps it.
        BomColumn::Value => {
            let keys: Vec<(Option<signex_types::quantity::Quantity>, String)> = rows
                .iter()
                .map(|r| {
                    let text = column_value(sort_col, r);
                    let parsed = signex_types::quantity::sort_key_from_value_text(&text);
                    // On-screen order must match the export, so no locale
                    // collation here (`designator_sort_matches_the_export_order`).
                    // ast-grep-ignore: no-case-in-view
                    (parsed, text.to_ascii_lowercase())
                })
                .collect();
            row_order.sort_by(|&a, &b| {
                let cmp = match (&keys[a].0, &keys[b].0) {
                    (Some(x), Some(y)) => signex_types::quantity::compare_for_sort(x, y),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => {
                        signex_types::designator::compare_references(&keys[a].1, &keys[b].1)
                    }
                };
                if asc { cmp } else { cmp.reverse() }
            });
        }
        // One lowercase key per row (O(n) allocations), not two per
        // comparison. Stable, like `sort_by`, so ties keep rollup order.
        _ => {
            let keys: Vec<String> = rows
                .iter()
                // On-screen order must match the export, so no locale
                // collation here (`designator_sort_matches_the_export_order`).
                // ast-grep-ignore: no-case-in-view
                .map(|r| column_value(sort_col, r).to_ascii_lowercase())
                .collect();
            row_order.sort_by(|&a, &b| {
                let cmp = keys[a].cmp(&keys[b]);
                if asc { cmp } else { cmp.reverse() }
            });
        }
    }
    row_order
}

/// Ink of a flag (the gutter number and its edge): the warning or error
/// colour. `None` for a clean line; notes never flag one.
fn flag_ink(tokens: &ThemeTokens, flag: Option<BomIssueSeverity>) -> Option<Color> {
    match flag? {
        BomIssueSeverity::Error => Some(ti(tokens.error)),
        BomIssueSeverity::Warning => Some(ti(tokens.warning)),
        BomIssueSeverity::Info => None,
    }
}

/// Fill of a line at rest: the neutral selection fill when selected, else
/// the soft tint of its flag, else none. Selection never borrows the warning
/// amber (decisions F25).
fn line_fill(
    tokens: &ThemeTokens,
    flag: Option<BomIssueSeverity>,
    selected: bool,
) -> Option<Color> {
    if selected {
        return Some(ti(tokens.selection));
    }
    let r = resolve(tokens);
    let soft = match flag? {
        BomIssueSeverity::Error => r.err_soft,
        BomIssueSeverity::Warning => r.warn_soft,
        BomIssueSeverity::Info => return None,
    };
    Some(Color {
        a: soft.a * FLAG_FILL_ALPHA,
        ..soft
    })
}

/// A container filled with `color`.
fn filled(color: Option<Color>) -> impl Fn(&Theme) -> container::Style + 'static {
    move |_| container::Style {
        background: color.map(Background::Color),
        ..container::Style::default()
    }
}

impl Signex {
    /// The data grid: the sticky header over the scrolling lines.
    pub(super) fn bom_table(&self) -> Element<'_, Message> {
        let Some(ref preview) = self.document_state.bom_preview else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        // A resized column keeps its width; the rest take their default.
        let widths: Vec<f32> = preview
            .options
            .columns
            .iter()
            .enumerate()
            .map(|(idx, c)| {
                preview
                    .column_widths
                    .get(&idx)
                    .copied()
                    .unwrap_or_else(|| default_column_width(c))
            })
            .collect();
        let gaps = widths.len().saturating_sub(1) as f32 * HANDLE_W;
        // Fixed, so the body scrolls sideways once the columns overflow.
        let table_width = (GUTTER_W + widths.iter().sum::<f32>() + gaps).max(100.0);

        let mut lines: Vec<Element<'_, Message>> = Vec::with_capacity(preview.visible_lines.len());
        for &line in &preview.visible_lines {
            // `visible_lines` and `row_order` are rebuilt with `table` in
            // `update`; a stale index means that pairing broke.
            let Some((row_idx, r)) = preview
                .row_order
                .get(line)
                .and_then(|&row_idx| preview.table.rows.get(row_idx).map(|r| (row_idx, r)))
            else {
                debug_assert!(false, "stale BOM line {line}");
                continue;
            };
            lines.push(table_line(
                preview,
                r,
                row_idx,
                line + 1,
                &widths,
                table_width,
                tokens,
            ));
        }
        // Room under the last line for the floating horizontal scrollbar.
        lines.push(
            Space::new()
                .height(ChromeMetrics::DEFAULT.scrollbar_width())
                .boxed(),
        );
        let body = scrollable(column(lines).width(Length::Fixed(table_width)))
            .id(BomPreviewState::BODY_SCROLL)
            .direction(crate::styles::thin_both())
            .style(crate::styles::scrollbar_style(tokens))
            .on_scroll(|scroll| {
                Message::BomPreview(BomPreviewMsg::BodyScrolled(
                    scroll.viewport.absolute_offset().x,
                ))
            })
            .width(Length::Fill)
            .height(Length::Fill);
        column![
            self.bom_table_header(preview, &widths, table_width, tokens),
            body
        ]
        .spacing(0)
        .boxed()
    }

    /// The sticky header: it stays put while the lines scroll down and
    /// follows them sideways (`BodyScrolled` moves it). A click sorts, a
    /// drag reorders, the handle between heads resizes.
    fn bom_table_header<'a>(
        &'a self,
        preview: &'a BomPreviewState,
        widths: &[f32],
        table_width: f32,
        tokens: &ThemeTokens,
    ) -> Element<'a, Message> {
        let r = resolve(tokens);
        let ink = ti(tokens.text);
        let cursor_x = self.interaction_state.last_mouse_pos.0;
        let active_drag = match (preview.column_drag, preview.column_drag_press_x) {
            (Some(idx), Some(ox)) if (cursor_x - ox).abs() > COL_DRAG_THRESHOLD_PX => Some(idx),
            _ => None,
        };
        let medium = iced::Font {
            weight: iced::font::Weight::Medium,
            ..iced::Font::DEFAULT
        };
        let n = preview.options.columns.len();
        let mut heads: Row<Element<'a, Message>> =
            Row::new().push(Space::new().width(GUTTER_W).height(ROW_H).boxed());
        for (idx, c) in preview.options.columns.iter().enumerate() {
            let sorted = match preview.sort {
                Some((s, asc)) if s == idx => Some(asc),
                _ => None,
            };
            // raw: the column name comes from signex-output (#2060).
            let mut label: Row<Element<'a, Message>> = Row::new()
                .spacing(SP.xs)
                .align_y(iced::Alignment::Center)
                .push(
                    text(raw(c.header().to_string()))
                        .size(TS.body)
                        .font(medium)
                        .color(if sorted.is_some() { ink } else { r.ink3 })
                        .wrapping(iced::widget::text::Wrapping::None)
                        .boxed(),
                );
            if let Some(asc) = sorted {
                let arrow = if asc { "\u{25B2}" } else { "\u{25BC}" };
                // raw: the sort arrow is a glyph.
                label = label.push(
                    text(raw(arrow))
                        .size(TS.caption)
                        .color(r.accent_text)
                        .boxed(),
                );
            }
            // A head lights up only while it is being dragged to a new place.
            let fill = (active_drag == Some(idx)).then_some(r.accent_soft);
            let cell = container(label)
                .width(Length::Fixed(widths[idx]))
                .height(ROW_H)
                .padding([0.0, SP.m])
                .align_x(if is_numeric(c) {
                    iced::alignment::Horizontal::Right
                } else {
                    iced::alignment::Horizontal::Left
                })
                .align_y(iced::alignment::Vertical::Center)
                .clip(true)
                .style(filled(fill));
            // The release lands on the head under the cursor: the pressed
            // head itself sorts, another one takes the dragged column.
            heads = heads.push(
                mouse_area(cell)
                    .on_press(Message::BomPreview(BomPreviewMsg::ColumnDragStart(idx)))
                    .on_release(if preview.column_drag == Some(idx) {
                        Message::BomPreview(BomPreviewMsg::SortColumn(idx))
                    } else {
                        Message::BomPreview(BomPreviewMsg::ColumnDragDrop(idx))
                    })
                    .on_enter(Message::BomPreview(BomPreviewMsg::ColumnHoverEnter(idx)))
                    .on_exit(Message::BomPreview(BomPreviewMsg::ColumnHoverExit(idx)))
                    .interaction(iced::mouse::Interaction::Pointer)
                    .boxed(),
            );
            if idx + 1 < n {
                let rule = container(Space::new())
                    .width(HAIRLINE)
                    .height(Length::Fill)
                    .style(filled(Some(r.line_soft)));
                let handle = container(rule)
                    .width(HANDLE_W)
                    .height(ROW_H)
                    .padding([SP.s, 0.0])
                    .align_x(iced::alignment::Horizontal::Center);
                heads = heads.push(
                    mouse_area(handle)
                        .on_press(Message::BomPreview(BomPreviewMsg::ColumnResizeStart(idx)))
                        .on_release(Message::BomPreview(BomPreviewMsg::ColumnResizeEnd))
                        .interaction(iced::mouse::Interaction::ResizingHorizontally)
                        .boxed(),
                );
            }
        }
        let strip = scrollable(container(heads).width(Length::Fixed(table_width)))
            .id(BomPreviewState::HEADER_SCROLL)
            .direction(iced::widget::scrollable::Direction::Horizontal(
                iced::widget::scrollable::Scrollbar::hidden(),
            ))
            .width(Length::Fill);
        let chrome = ti(tokens.toolbar_bg);
        let edge = ti(tokens.border);
        column![
            container(strip)
                .width(Length::Fill)
                .style(filled(Some(chrome))),
            container(Space::new())
                .width(Length::Fill)
                .height(HAIRLINE)
                .style(filled(Some(edge))),
        ]
        .boxed()
    }
}

/// One line: the gutter (line number, flag edge), the cells, then a soft
/// rule. A click selects it.
fn table_line<'a>(
    preview: &BomPreviewState,
    r: &BomRow,
    row_idx: usize,
    line_no: usize,
    widths: &[f32],
    table_width: f32,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let res = resolve(tokens);
    let ink = ti(tokens.text);
    let mono = super::super::graphite::mono();
    let flag = preview.row_flags.get(row_idx).copied().flatten();
    let edge = flag_ink(tokens, flag);
    let cell_h = ROW_H - HAIRLINE;

    // raw: a line number, not language.
    let number = container(
        text(raw(line_no.to_string()))
            .size(TS.caption)
            .font(mono)
            .color(edge.unwrap_or(res.ink4)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(iced::Padding {
        right: SP.s,
        ..iced::Padding::ZERO
    })
    .align_x(iced::alignment::Horizontal::Right)
    .align_y(iced::alignment::Vertical::Center);
    let gutter = row![
        container(Space::new())
            .width(FLAG_EDGE)
            .height(Length::Fill)
            .style(filled(edge)),
        number,
    ]
    .width(GUTTER_W)
    .height(cell_h);

    let n = preview.options.columns.len();
    let mut cells: Row<Element<'a, Message>> = Row::new().push(gutter.boxed());
    for (idx, c) in preview.options.columns.iter().enumerate() {
        let value = column_value(c, r);
        // An empty cell reads as a muted dash, as drawn.
        let face = if value.is_empty() {
            // raw: a dash glyph for "nothing here", not language.
            text(raw("\u{2014}")).color(res.ink4)
        } else {
            let colour = if matches!(c, BomColumn::Stock) {
                res.ink4
            } else {
                ink
            };
            text(UserText::new(value)).color(colour)
        };
        let face = if is_mono(c) {
            face.font(mono).size(TS.dense)
        } else {
            face.size(TS.body)
        };
        cells = cells.push(
            container(face.wrapping(iced::widget::text::Wrapping::None))
                .width(Length::Fixed(widths[idx]))
                .height(cell_h)
                .padding([0.0, SP.m])
                .align_x(if is_numeric(c) {
                    iced::alignment::Horizontal::Right
                } else {
                    iced::alignment::Horizontal::Left
                })
                .align_y(iced::alignment::Vertical::Center)
                .clip(true)
                .boxed(),
        );
        if idx + 1 < n {
            cells = cells.push(Space::new().width(HANDLE_W).boxed());
        }
    }
    let content = column![
        cells,
        container(Space::new())
            .width(Length::Fill)
            .height(HAIRLINE)
            .style(filled(Some(res.line_soft))),
    ]
    .width(Length::Fixed(table_width));

    let rest = line_fill(tokens, flag, preview.selected_row == Some(row_idx));
    let hover = ti(tokens.hover);
    button(content)
        .padding(0)
        .width(Length::Fixed(table_width))
        .on_press(Message::BomPreview(BomPreviewMsg::RowSelected(row_idx)))
        .style(move |_: &Theme, status: button::Status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            // A flagged or selected line keeps its fill under the pointer.
            let fill = rest.or(hovered.then_some(hover));
            button::Style {
                background: fill.map(Background::Color),
                text_color: ink,
                ..button::Style::default()
            }
        })
        .boxed()
}

#[cfg(test)]
#[path = "table_tests.rs"]
mod tests;
