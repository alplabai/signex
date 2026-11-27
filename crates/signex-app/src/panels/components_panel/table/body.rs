//! The table body: group lines and part rows, windowed so a frame builds
//! widgets only for the lines in view (25,000 parts stay smooth). A part
//! row keeps the old list row's gestures: a press selects it and arms a
//! drag onto the schematic, a right-click opens its menu; a double-click
//! places it (mockup row 52).

use iced::widget::{Column, Row, Space, column, container, mouse_area, row, scrollable};
use iced::{Alignment, Background, Element, Length, Theme, Widget};
use signex_i18n::{I18n, ids, t};
use signex_library::components_view::CellValue;
use signex_library::templates::ColumnKey;
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::super::rows::{RowColors, RowFlags};
use super::super::{HINT_TEXT_SIZE, ROW_TEXT_SIZE, look};
use super::cache::{TableLine, TableRow};
use super::cells;
use super::columns::{caption, is_right_aligned};
use super::header;
use super::view::user_text;
use crate::library::browser::{OVERSCAN_ROWS, visible_range};
use crate::library::messages::{ComponentsTableMsg, LibraryMessage};
use crate::library::state::LibraryState;

/// Height of a table line.
pub(super) const LINE_HEIGHT: f32 = look::ROW_HEIGHT;
/// Indent per group level.
const LEVEL_INDENT: f32 = 10.0;
/// The viewport height assumed until the body first reports its own.
const FIRST_VIEWPORT: f32 = 600.0;

/// Widget id of the body's scrollable.
pub(crate) const BODY_SCROLL: iced::widget::Id = iced::widget::Id::new("components-table-body");

fn msg(m: ComponentsTableMsg) -> LibraryMessage {
    LibraryMessage::ComponentsTable(m)
}

/// The colours of a line, from the theme.
pub(super) fn line_colors(tokens: &ThemeTokens) -> RowColors {
    let r = crate::styles::tokens::resolve(tokens);
    RowColors {
        text: signex_widgets::theme_ext::text_primary(tokens),
        muted: r.ink4,
        // Graphite selects with the neutral selected token, not the accent wash.
        tint: crate::styles::ti(tokens.selection),
        hover: crate::styles::tokens::overlay(tokens, crate::styles::tokens::Overlay::Hover),
        menu: r.accent_soft,
    }
}

/// The lines a frame builds: those in view plus an overscan.
pub(super) fn window(lines: usize, scroll_y: f32, viewport_h: f32) -> std::ops::Range<usize> {
    let viewport_h = if viewport_h > 0.0 {
        viewport_h
    } else {
        FIRST_VIEWPORT
    };
    visible_range(scroll_y, viewport_h, LINE_HEIGHT, lines, OVERSCAN_ROWS)
}

pub(super) fn view<'a>(
    state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
) -> Element<'a, LibraryMessage> {
    let panel = &state.components_panel;
    let table = &panel.table;
    let (columns, total) = header::layout(table);
    let width = header::total_width(total, table);
    let lines = &table.cache.lines;
    let shown = window(lines.len(), table.scroll.y, table.scroll.viewport_h);
    let colors = line_colors(tokens);
    let mut col: Column<Element<'a, LibraryMessage>> = column![].spacing(0);
    if shown.start > 0 {
        col = col.push(
            Space::new()
                .height(shown.start as f32 * LINE_HEIGHT)
                .boxed(),
        );
    }
    for line in &lines[shown.clone()] {
        col = col.push(match line {
            TableLine::Group {
                path,
                column,
                value,
                count,
                depth,
                open,
            } => group_line(path, column, value, *count, *depth, *open, colors, i18n),
            TableLine::Row { row, depth } => match table.cache.base.get(*row) {
                Some(r) => part_line(state, r, &columns, *depth, colors, i18n),
                None => Space::new().height(LINE_HEIGHT).boxed(),
            },
        });
    }
    if shown.end < lines.len() {
        col = col.push(
            Space::new()
                .height((lines.len() - shown.end) as f32 * LINE_HEIGHT)
                .boxed(),
        );
    }
    scrollable(container(col).width(Length::Fixed(width.max(1.0))))
        .id(BODY_SCROLL)
        .direction(crate::styles::thin_both())
        .style(crate::panels::form_look::scrollbar_style)
        .on_scroll(|scroll| {
            let offset = scroll.viewport.absolute_offset();
            msg(ComponentsTableMsg::Scrolled {
                x: offset.x,
                y: offset.y,
                viewport_h: scroll.viewport.bounds().height,
            })
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

/// A group line: chevron, the column's caption, the shared value ("Not
/// set" for the parts without one) and the part count. A click opens or
/// closes it.
#[allow(clippy::too_many_arguments)]
fn group_line<'a>(
    path: &signex_library::components_view::GroupPath,
    column: &ColumnKey,
    value: &CellValue,
    count: usize,
    depth: usize,
    open: bool,
    colors: RowColors,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let shown = cells::shown(value, i18n);
    let is_set = shown.is_some();
    let value_text =
        shown.unwrap_or_else(|| i18n.message_plain(ids::PANELS_COMPONENTS_NOT_SET, &[]));
    let line = row![
        signex_widgets::chevron::chevron(open, HINT_TEXT_SIZE, colors.muted),
        text(user_text(caption(column, i18n)))
            .size(HINT_TEXT_SIZE)
            .color(colors.muted)
            .wrapping(iced::widget::text::Wrapping::None),
        text(user_text(value_text))
            .size(ROW_TEXT_SIZE)
            .color(if is_set { colors.text } else { colors.muted })
            .wrapping(iced::widget::text::Wrapping::None),
        text(t!(i18n, ids::PANELS_COMPONENTS_GROUP_COUNT, "count" => count))
            .size(HINT_TEXT_SIZE)
            .color(colors.muted),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    let hover = colors.hover;
    let face = iced::widget::button(line)
        .padding(iced::Padding {
            left: 4.0 + depth as f32 * LEVEL_INDENT,
            ..iced::Padding::ZERO
        })
        .height(LINE_HEIGHT)
        .width(Length::Fill)
        .on_press(msg(ComponentsTableMsg::GroupToggled(path.clone())))
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            iced::widget::button::Style {
                background: matches!(status, iced::widget::button::Status::Hovered)
                    .then_some(Background::Color(hover)),
                ..iced::widget::button::Style::default()
            }
        });
    face.boxed()
}

/// A part row: one cell per visible column, "—" where a value is not set.
fn part_line<'a>(
    state: &'a LibraryState,
    r: &'a TableRow,
    columns: &[(ColumnKey, f32)],
    depth: usize,
    colors: RowColors,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let panel = &state.components_panel;
    let Some(row_data) = r.row(&state.open_libraries) else {
        return Space::new().height(LINE_HEIGHT).boxed();
    };
    let template = panel.table.cache.template.as_ref();
    let mut cells_row: Row<Element<'a, LibraryMessage>> = row![].spacing(0);
    for (i, (column, width)) in columns.iter().enumerate() {
        let shown = cells::cell_text(row_data, &r.facts, template, column, i18n);
        let (words, ink) = match shown {
            Some(words) => (words, colors.text),
            None => ("\u{2014}".to_owned(), colors.muted),
        };
        let mut cell = text(user_text(words))
            .size(ROW_TEXT_SIZE)
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::None);
        if *column == ColumnKey::PartNumber {
            cell = cell.font(look::mono());
        }
        let left = if i == 0 {
            6.0 + depth as f32 * LEVEL_INDENT
        } else {
            6.0
        };
        cells_row = cells_row.push(
            container(cell)
                .padding(iced::Padding {
                    left,
                    right: 6.0,
                    ..iced::Padding::ZERO
                })
                .width(Length::Fixed(header::cell_width(*width)))
                .height(LINE_HEIGHT)
                .center_y(LINE_HEIGHT)
                .align_x(if is_right_aligned(column) {
                    Alignment::End
                } else {
                    Alignment::Start
                })
                .clip(true)
                .boxed(),
        );
    }
    let key = r.key.clone();
    let flags = RowFlags {
        selected: panel.selected.as_ref() == Some(&key),
        hovered: panel.hovered.as_ref() == Some(&key),
        menu_open: panel.row_menu.as_ref().is_some_and(|m| m.key == key),
    };
    let fill = flags.fill(colors);
    let face = container(cells_row)
        .height(LINE_HEIGHT)
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            ..container::Style::default()
        });
    mouse_area(face)
        .on_press(LibraryMessage::ComponentsPanelRowPressed(key.clone()))
        .on_right_press(LibraryMessage::ComponentsPanelRowRightClicked(key.clone()))
        .on_double_click(LibraryMessage::ComponentsPanelPlace {
            library_path: key.library_path.clone(),
            table: key.table.clone(),
            row_id: key.row_id,
        })
        .on_enter(LibraryMessage::ComponentsPanelRowEntered(key.clone()))
        .on_exit(LibraryMessage::ComponentsPanelRowExited(key))
        .boxed()
}
