//! The header strip: one cell per visible column. A click sorts, Shift
//! adds a nested key (its order number shows once there are two), a
//! right-click opens the column's menu and the handle at a cell's right
//! edge resizes it. The strip scrolls sideways with the body.

use iced::widget::{Row, container, mouse_area, row, scrollable};
use iced::{Alignment, Background, Element, Length, Theme, Widget};
use signex_i18n::I18n;
use signex_library::templates::ColumnKey;
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::super::HINT_TEXT_SIZE;
use super::columns::{caption, is_right_aligned};
use super::state::ComponentsTable;
use super::view::user_text;
use crate::library::messages::{ComponentsTableMsg, LibraryMessage};
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// Height of the header strip.
pub(super) const HEADER_HEIGHT: f32 = 26.0;
/// Width of the resize handle at a header cell's right edge.
const HANDLE_WIDTH: f32 = 6.0;
/// Side padding of the Previews thumbnail cell.
pub(super) const THUMB_PAD: f32 = 4.0;

/// Width of the Previews thumbnail cell that leads every row.
pub(super) fn thumb_cell_width() -> f32 {
    super::super::look::THUMB_WIDTH + 2.0 * THUMB_PAD
}

/// Widget id of the header strip, scrolled with the body.
pub(crate) const HEADER_SCROLL: iced::widget::Id = iced::widget::Id::new("components-table-header");

fn msg(m: ComponentsTableMsg) -> LibraryMessage {
    LibraryMessage::ComponentsTable(m)
}

/// The width of every visible column, in order, and their sum.
pub(super) fn layout(table: &ComponentsTable) -> (Vec<(ColumnKey, f32)>, f32) {
    let columns: Vec<(ColumnKey, f32)> = table
        .cache
        .setup
        .visible_columns()
        .into_iter()
        .map(|c| {
            let w = table.width_of(&c);
            (c, w)
        })
        .collect();
    let total = columns.iter().map(|(_, w)| w).sum();
    (columns, total)
}

/// The sort mark of `column`: its arrow, and its place once two or more
/// keys sort.
pub(super) fn sort_mark(table: &ComponentsTable, column: &ColumnKey) -> Option<String> {
    let sort = &table.cache.setup.sort;
    let at = sort.iter().position(|k| k.column == *column)?;
    let arrow = if sort[at].descending { "↓" } else { "↑" };
    Some(if sort.len() > 1 {
        format!("{arrow}{}", at + 1)
    } else {
        arrow.to_owned()
    })
}

pub(super) fn view<'a>(
    table: &'a ComponentsTable,
    previews: bool,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let (columns, total) = layout(table);
    let ink = ti(tokens.text);
    let r = resolve(tokens);
    let (muted, line, chrome) = (r.ink3, ti(tokens.border), ti(tokens.toolbar_bg));
    let mut cells: Row<Element<'a, LibraryMessage>> = row![].spacing(0);
    // The Previews thumbnail column has no title.
    if previews {
        cells = cells.push(
            iced::widget::Space::new()
                .width(thumb_cell_width())
                .height(HEADER_HEIGHT)
                .boxed(),
        );
    }
    for (column, width) in columns {
        let mut label = row![
            text(user_text(caption(&column, i18n)))
                .size(HINT_TEXT_SIZE)
                .color(ink)
                .wrapping(iced::widget::text::Wrapping::None)
                .width(Length::Fill)
        ]
        .spacing(2)
        .align_y(Alignment::Center);
        if let Some(mark) = sort_mark(table, &column) {
            label = label.push(
                text(user_text(mark))
                    .size(HINT_TEXT_SIZE)
                    .color(muted)
                    .boxed(),
            );
        }
        let label = container(label)
            .padding([0.0, 6.0])
            .width(Length::Fill)
            .height(HEADER_HEIGHT)
            .center_y(HEADER_HEIGHT)
            .align_x(if is_right_aligned(&column) {
                Alignment::End
            } else {
                Alignment::Start
            });
        let press = mouse_area(label.clip(true))
            .on_press(msg(ComponentsTableMsg::HeaderPressed(column.clone())))
            .on_right_press(msg(ComponentsTableMsg::HeaderRightClicked(column.clone())));
        let handle = mouse_area(
            iced::widget::Space::new()
                .width(HANDLE_WIDTH)
                .height(HEADER_HEIGHT),
        )
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .on_press(msg(ComponentsTableMsg::ResizeStarted(column.clone())));
        let separator = container(iced::widget::Space::new())
            .width(1.0)
            .height(HEADER_HEIGHT)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(line)),
                ..container::Style::default()
            });
        cells = cells.push(
            row![press, handle, separator]
                .spacing(0)
                .width(Length::Fixed(width + HANDLE_WIDTH + 1.0))
                .boxed(),
        );
    }
    let strip =
        scrollable(container(cells).width(Length::Fixed(total_width(total, table, previews))))
            .id(HEADER_SCROLL)
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::hidden(),
            ))
            .width(Length::Fill);
    container(strip)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(chrome)),
            ..container::Style::default()
        })
        .boxed()
}

/// The full width of a line: the columns, their handles and separators.
pub(super) fn total_width(columns_total: f32, table: &ComponentsTable, previews: bool) -> f32 {
    let count = table.cache.setup.visible_columns().len() as f32;
    let thumbs = if previews { thumb_cell_width() } else { 0.0 };
    thumbs + columns_total + count * (HANDLE_WIDTH + 1.0)
}

/// The width a body cell takes for a column `width` wide: the column and
/// the header's handle and separator, so cells line up under headers.
pub(super) fn cell_width(width: f32) -> f32 {
    width + HANDLE_WIDTH + 1.0
}
