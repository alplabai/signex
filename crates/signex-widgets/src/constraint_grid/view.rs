//! The grid view: find box, group headers, column headers and the row
//! hierarchy. Pure: `&ConstraintGrid -> Element<GridMessage>`; the caller
//! maps it into its own message (`.map(Message::Rules)`).

use iced::alignment::Vertical;
use iced::widget::{Column, Row, Space, container, mouse_area, scrollable};
use iced::{Background, Border, Element, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, UserText};

use super::cell;
use super::model::{CellRef, ColumnGroup, ConstraintGrid, VisibleRow};
use super::style::{GridMetrics, GridStyle, GridText};
use super::update::GridMessage;
use crate::chevron::chevron;
use crate::{Label, button, text, text_input};

/// Width of one column group as laid out.
fn group_width(grid: &ConstraintGrid, group: &ColumnGroup, m: GridMetrics) -> f32 {
    if grid.is_group_collapsed(group.id) {
        m.collapsed_width
    } else {
        m.cell_width * group.columns.len() as f32
    }
}

/// Total width of the grid, so a both-ways scrollable knows its content.
pub fn grid_width(grid: &ConstraintGrid, m: GridMetrics) -> f32 {
    m.name_width
        + grid
            .groups()
            .iter()
            .map(|g| group_width(grid, g, m))
            .sum::<f32>()
}

fn header_cell<'a>(
    label: Label<'a>,
    width: f32,
    is_marked: bool,
    style: GridStyle,
) -> Element<'a, GridMessage> {
    let m = style.metrics();
    let ink = if is_marked { style.accent } else { style.ink };
    container(
        text(label)
            .size(style.text_size)
            .color(ink)
            .font(cell::strong_font())
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding(Padding::from([0.0, m.pad]))
    .width(width)
    .height(m.row_height)
    .align_y(Vertical::Center)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(if is_marked {
            style.accent_soft
        } else {
            style.header_fill
        })),
        border: Border {
            color: style.line,
            width: 0.5,
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

fn group_header<'a>(
    grid: &'a ConstraintGrid,
    group: &'a ColumnGroup,
    style: GridStyle,
) -> Element<'a, GridMessage> {
    let m = style.metrics();
    let is_open = !grid.is_group_collapsed(group.id);
    let mut line: Row<Element<'a, GridMessage>> = Row::new()
        .spacing(m.pad / 2.0)
        .align_y(Vertical::Center)
        .push(chevron(is_open, style.text_size, style.muted_ink).boxed());
    // A collapsed group is too narrow for its title; the chevron stands in.
    if is_open {
        line = line.push(
            text(group.title.clone())
                .size(style.text_size)
                .color(style.ink)
                .font(cell::strong_font())
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
        );
    }
    button(line)
        .padding(Padding::from([0.0, m.pad / 2.0]))
        .width(group_width(grid, group, m))
        .height(m.row_height)
        .on_press(GridMessage::GroupToggled(group.id))
        .style(move |_: &Theme, _| button::Style {
            background: Some(Background::Color(style.header_fill)),
            text_color: style.ink,
            border: Border {
                color: style.line,
                width: 0.5,
                ..Border::default()
            },
            ..button::Style::default()
        })
        .boxed()
}

fn headers<'a>(
    grid: &'a ConstraintGrid,
    words: &GridText<'a>,
    style: GridStyle,
) -> [Element<'a, GridMessage>; 2] {
    let m = style.metrics();
    let found = grid.found_column();
    let mut groups: Row<Element<'a, GridMessage>> = Row::new().push(
        Space::new()
            .width(m.name_width)
            .height(m.row_height)
            .boxed(),
    );
    let mut columns: Row<Element<'a, GridMessage>> = Row::new().push(header_cell(
        words.object_header.clone(),
        m.name_width,
        false,
        style,
    ));
    for group in grid.groups() {
        groups = groups.push(group_header(grid, group, style));
        if grid.is_group_collapsed(group.id) {
            columns = columns.push(header_cell(
                UserText::new(String::new()).into(),
                m.collapsed_width,
                false,
                style,
            ));
            continue;
        }
        for column in &group.columns {
            columns = columns.push(header_cell(
                column.title.clone(),
                m.cell_width,
                found == Some(column.id),
                style,
            ));
        }
    }
    [groups.boxed(), columns.boxed()]
}

fn name_cell<'a>(
    grid: &'a ConstraintGrid,
    visible: VisibleRow,
    style: GridStyle,
) -> Element<'a, GridMessage> {
    let m = style.metrics();
    let name = grid.row(visible.id).map_or("", |r| r.name.as_str());
    let toggle: Element<'a, GridMessage> = if visible.has_children {
        mouse_area(chevron(visible.is_open, style.text_size, style.muted_ink))
            .on_press(GridMessage::RowToggled(visible.id))
            .interaction(iced::mouse::Interaction::Pointer)
            .boxed()
    } else {
        Space::new().width(m.glyph).boxed()
    };
    let is_selected_row = grid.selected().is_some_and(|c| c.row == visible.id);
    container(
        Row::new()
            .spacing(m.pad / 2.0)
            .align_y(Vertical::Center)
            .push(Space::new().width(m.indent * visible.depth as f32).boxed())
            .push(toggle)
            .push(
                text(UserText::new(name))
                    .size(style.text_size)
                    .color(style.ink)
                    .wrapping(iced::widget::text::Wrapping::None)
                    .boxed(),
            ),
    )
    .padding(Padding::from([0.0, m.pad]))
    .width(m.name_width)
    .height(m.row_height)
    .align_y(Vertical::Center)
    .style(move |_: &Theme| container::Style {
        background: is_selected_row.then_some(Background::Color(style.accent_soft)),
        border: Border {
            color: style.line,
            width: 0.5,
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

fn body_row<'a>(
    grid: &'a ConstraintGrid,
    visible: VisibleRow,
    i18n: &I18n,
    words: &GridText<'a>,
    style: GridStyle,
) -> Element<'a, GridMessage> {
    let m = style.metrics();
    let mut line: Row<Element<'a, GridMessage>> = Row::new().push(name_cell(grid, visible, style));
    for group in grid.groups() {
        if grid.is_group_collapsed(group.id) {
            line = line.push(
                Space::new()
                    .width(m.collapsed_width)
                    .height(m.row_height)
                    .boxed(),
            );
            continue;
        }
        for column in &group.columns {
            let at = CellRef::new(visible.id, column.id);
            line = line.push(cell::view(grid, column, at, i18n, words, style));
        }
    }
    line.boxed()
}

/// The find-a-column box. Enter steps to the next match.
pub fn find_box<'a>(
    grid: &'a ConstraintGrid,
    words: &GridText<'a>,
    style: GridStyle,
) -> Element<'a, GridMessage> {
    let m = style.metrics();
    text_input(words.find_placeholder.clone(), grid.column_query())
        .on_input(GridMessage::ColumnQueryChanged)
        .on_submit(GridMessage::ColumnQuerySubmitted)
        .size(style.text_size)
        .padding(Padding::from([m.pad / 2.0, m.pad]))
        .width(m.name_width)
        .boxed()
}

/// The constraint grid: the find box over a both-ways scrollable table.
pub fn view<'a>(
    grid: &'a ConstraintGrid,
    i18n: &I18n,
    words: &GridText<'a>,
    style: GridStyle,
) -> Element<'a, GridMessage> {
    let m = style.metrics();
    let [group_row, column_row] = headers(grid, words, style);
    let mut table: Column<Element<'a, GridMessage>> =
        Column::new().push(group_row).push(column_row);
    for visible in grid.visible_rows() {
        table = table.push(body_row(grid, visible, i18n, words, style));
    }
    let table = table.width(Length::Fixed(grid_width(grid, m)));
    let body = scrollable(table)
        .direction(scrollable::Direction::Both {
            vertical: scrollable::Scrollbar::default(),
            horizontal: scrollable::Scrollbar::default(),
        })
        .width(Length::Fill)
        .height(Length::Fill);
    Column::new()
        .spacing(m.pad)
        .push(find_box(grid, words, style))
        .push(body.boxed())
        .boxed()
}
