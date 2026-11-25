//! The constraint grid (#2530): two header rows (column groups with their
//! findings, then columns with their units) over the row hierarchy (All,
//! net classes and nets, layers, scoped rules). The grid's view state is
//! the `ConstraintGrid` the tab holds; every press is a `GridActed`.

use iced::alignment::Vertical;
use iced::widget::{Column, Row, Space, container, mouse_area, scrollable};
use iced::{Background, Border, Element, Length, Padding, Theme, Widget as _};
use signex_drc_dsl::table::catalog::{CatalogGroup, ColumnUnit};
use signex_drc_dsl::table::scope_text;
use signex_i18n::{Localized, MessageId, ids, t};
use signex_widgets::chevron::chevron;
use signex_widgets::constraint_grid::{
    CellRef, ColumnId, GridMessage, GridMetrics, GridStyle, GroupId, RowId, VisibleRow,
};
use signex_widgets::{Label, button, text, tooltip};

use super::grid_icons::{Icon, icon};
use super::{Frame, SP, TS, up, user};
use crate::app::Message;
use crate::app::rules_tab::RulesTabMsg;
use crate::app::rules_tab::grid::{RowGroup, RowRef, RulesGrid, sets_on};
use crate::app::rules_tab::search::{column_label, group_label, type_label};
use crate::styles::tokens::resolve;
use crate::styles::{HAIRLINE, Radii, ti};

/// The object column is this much wider than the widget's name column:
/// it carries kind tags and set chips.
const NAME_SCALE: f32 = 1.4;

pub(super) fn act(frame: &Frame, msg: GridMessage) -> Message {
    up(RulesTabMsg::GridActed {
        project_id: frame.id,
        msg,
    })
}

fn name_width(m: GridMetrics) -> f32 {
    m.name_width * NAME_SCALE
}

fn group_width(grid: &RulesGrid, group: GroupId, m: GridMetrics) -> f32 {
    if grid.view.is_group_collapsed(group) {
        return m.collapsed_width;
    }
    let columns = grid.group(group).map_or(0, |g| g.columns.len());
    m.cell_width * columns as f32
}

fn grid_width(grid: &RulesGrid, m: GridMetrics) -> f32 {
    name_width(m)
        + (0..grid.groups().len())
            .map(|g| group_width(grid, GroupId(g as u32), m))
            .sum::<f32>()
}

/// Where `column` starts, from the grid's left edge.
pub(super) fn column_left(grid: &RulesGrid, column: ColumnId, m: GridMetrics) -> f32 {
    let mut x = name_width(m);
    let mut first = 0u32;
    for (g, group) in grid.groups().iter().enumerate() {
        let id = GroupId(g as u32);
        let end = first + group.columns.len() as u32;
        if (first..end).contains(&column.0) {
            if grid.view.is_group_collapsed(id) {
                return x;
            }
            return x + m.cell_width * (column.0 - first) as f32;
        }
        x += group_width(grid, id, m);
        first = end;
    }
    x
}

pub(super) fn view<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let style = frame.grid_style();
    let m = frame.metrics();
    let grid = frame.grid();
    let mut table: Column<Element<'a, Message>> = Column::new()
        .push(group_headers(frame, style, m))
        .push(column_headers(frame, style, m));
    for visible in grid.view.visible_rows() {
        let line = match grid.row(visible.id) {
            Some(RowRef::Heading(group)) => heading_row(frame, visible, *group, style, m),
            Some(row) => body_row(frame, visible, row, style, m),
            None => continue,
        };
        table = table.push(line);
    }
    let body = scrollable(table.width(Length::Fixed(grid_width(grid, m))))
        .id(super::grid_scroll_id())
        .direction(crate::styles::thin_both())
        .style(crate::styles::scrollbar_style(frame.tokens))
        .width(Length::Fill)
        .height(Length::Fill);
    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::styles::card(frame.tokens))
        .boxed()
}

/// Signal integrity: planned with the simulation band.
pub(super) fn later<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let (i18n, tokens) = (frame.i18n, frame.tokens);
    let r = resolve(tokens);
    let warn = ti(tokens.warning);
    container(
        iced::widget::row![
            icon(Icon::Info, TS.body, warn),
            text(t!(i18n, ids::RULES_GRID_SIGNAL_INTEGRITY_LATER))
                .size(TS.body)
                .width(Length::Fill),
        ]
        .spacing(SP.s)
        .align_y(Vertical::Center),
    )
    .padding([SP.s, SP.m])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(r.warn_soft)),
        border: Border {
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

fn header_box(style: GridStyle, is_marked: bool) -> impl Fn(&Theme) -> container::Style {
    move |_: &Theme| container::Style {
        background: Some(Background::Color(if is_marked {
            style.accent_soft
        } else {
            style.header_fill
        })),
        border: Border {
            color: style.line,
            width: HAIRLINE,
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// The group header row: a collapse chevron, the group's name and its
/// findings in the last run.
fn group_headers<'a>(frame: &Frame<'a>, style: GridStyle, m: GridMetrics) -> Element<'a, Message> {
    let grid = frame.grid();
    let mut line: Row<Element<'a, Message>> = Row::new().push(
        Space::new()
            .width(name_width(m))
            .height(m.row_height)
            .boxed(),
    );
    for (g, group) in grid.groups().iter().enumerate() {
        line = line.push(group_header(frame, GroupId(g as u32), group, style, m));
    }
    line.boxed()
}

fn group_header<'a>(
    frame: &Frame<'a>,
    id: GroupId,
    group: &'static CatalogGroup,
    style: GridStyle,
    m: GridMetrics,
) -> Element<'a, Message> {
    let (i18n, tokens) = (frame.i18n, frame.tokens);
    let is_open = !frame.grid().view.is_group_collapsed(id);
    let mut face: Row<Element<'a, Message>> = Row::new()
        .spacing(m.pad / 2.0)
        .align_y(Vertical::Center)
        .push(chevron(is_open, TS.body, style.muted_ink).boxed());
    if is_open {
        face = face.push(
            text(t!(i18n, group_label(group.id)))
                .size(TS.body)
                .color(style.ink)
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
        );
        let findings = frame.group_findings(group).unwrap_or(0);
        if findings > 0 {
            face = face.push(badge(findings, ti(tokens.error), ti(tokens.paper)));
        }
    }
    let press = button(face)
        .padding(Padding::from([0.0, m.pad / 2.0]))
        .width(group_width(frame.grid(), id, m))
        .height(m.row_height)
        .on_press(act(frame, GridMessage::GroupToggled(id)))
        .style(move |_: &Theme, _| button::Style {
            background: Some(Background::Color(style.header_fill)),
            text_color: style.ink,
            border: Border {
                color: style.line,
                width: HAIRLINE,
                ..Border::default()
            },
            ..button::Style::default()
        });
    let tip = if is_open {
        ids::RULES_GROUP_COLLAPSE
    } else {
        ids::RULES_GROUP_EXPAND
    };
    tooltip(
        press,
        container(
            text(t!(
                i18n,
                tip,
                "type" => t!(i18n, type_label(group.id)).into_string()
            ))
            .size(TS.caption),
        )
        .style(crate::styles::card(tokens)),
        tooltip::Position::Bottom,
    )
    .boxed()
}

/// A count pill in `fill`.
pub(super) fn badge<'a>(count: usize, fill: iced::Color, ink: iced::Color) -> Element<'a, Message> {
    let m = signex_widgets::chips::ChipMetrics::from_text_size(TS.body);
    container(text(user(count.to_string())).size(m.badge_text).color(ink))
        .height(m.badge_height)
        .padding(Padding::from([0.0, m.badge_pad_h]))
        .align_y(Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: Radii::GRAPHITE.pill.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

fn unit_label(unit: ColumnUnit) -> Option<MessageId> {
    match unit {
        ColumnUnit::Mm => Some(ids::RULES_GRID_UNIT_MM),
        ColumnUnit::Percent => Some(ids::RULES_GRID_UNIT_PERCENT),
        ColumnUnit::Degrees => Some(ids::RULES_GRID_UNIT_DEGREES),
        ColumnUnit::Count | ColumnUnit::Choice => None,
    }
}

/// The column header row: Object, then each column with its unit. The
/// column the find box landed on is marked.
fn column_headers<'a>(frame: &Frame<'a>, style: GridStyle, m: GridMetrics) -> Element<'a, Message> {
    let (i18n, grid) = (frame.i18n, frame.grid());
    let found = frame.state.find.current().map(|hit| hit.column.id);
    let mut line: Row<Element<'a, Message>> = Row::new().push(
        container(
            text(t!(i18n, ids::RULES_GRID_OBJECT))
                .size(TS.body)
                .color(style.muted_ink),
        )
        .padding(Padding::from([0.0, m.pad]))
        .width(name_width(m))
        .height(m.row_height)
        .align_y(Vertical::Center)
        .style(header_box(style, false))
        .boxed(),
    );
    for (g, group) in grid.groups().iter().enumerate() {
        if grid.view.is_group_collapsed(GroupId(g as u32)) {
            line = line.push(
                container(
                    text(t!(i18n, ids::RULES_GRID_COLLAPSED))
                        .size(TS.caption)
                        .color(style.muted_ink),
                )
                .padding(Padding::from([0.0, m.pad / 2.0]))
                .width(m.collapsed_width)
                .height(m.row_height)
                .align_y(Vertical::Center)
                .style(header_box(style, false))
                .boxed(),
            );
            continue;
        }
        for column in group.columns {
            let is_found = found == Some(column.id);
            let ink = if is_found { style.accent } else { style.ink };
            let mut face: Row<Element<'a, Message>> =
                Row::new().spacing(SP.xs).align_y(Vertical::Center).push(
                    text(t!(i18n, column_label(column.id)))
                        .size(TS.body)
                        .color(ink)
                        .wrapping(iced::widget::text::Wrapping::None)
                        .boxed(),
                );
            if let Some(unit) = unit_label(column.unit) {
                face = face.push(
                    text(t!(i18n, unit))
                        .size(TS.caption)
                        .color(style.muted_ink)
                        .boxed(),
                );
            }
            line = line.push(
                container(face)
                    .padding(Padding::from([0.0, m.pad]))
                    .width(m.cell_width)
                    .height(m.row_height)
                    .align_y(Vertical::Center)
                    .clip(true)
                    .style(header_box(style, is_found))
                    .boxed(),
            );
        }
    }
    line.boxed()
}

/// A heading over a group of rows: Net Classes, Layers, Scoped rules.
fn heading_row<'a>(
    frame: &Frame<'a>,
    visible: VisibleRow,
    group: RowGroup,
    style: GridStyle,
    m: GridMetrics,
) -> Element<'a, Message> {
    let label = match group {
        RowGroup::NetClasses => ids::RULES_GRID_ROWS_NET_CLASSES,
        RowGroup::Layers => ids::RULES_GRID_ROWS_LAYERS,
        RowGroup::Scoped => ids::RULES_GRID_ROWS_SCOPED,
    };
    let face = Row::new()
        .spacing(m.pad / 2.0)
        .align_y(Vertical::Center)
        .push(chevron(visible.is_open, TS.body, style.muted_ink).boxed())
        .push(
            text(t!(frame.i18n, label))
                .size(TS.body)
                .color(style.muted_ink)
                .boxed(),
        );
    mouse_area(
        container(face)
            .padding(Padding::from([0.0, m.pad]))
            .width(Length::Fill)
            .height(m.row_height)
            .align_y(Vertical::Center)
            .style(header_box(style, false)),
    )
    .on_press(act(frame, GridMessage::RowToggled(visible.id)))
    .interaction(iced::mouse::Interaction::Pointer)
    .boxed()
}

fn body_row<'a>(
    frame: &Frame<'a>,
    visible: VisibleRow,
    row: &RowRef,
    style: GridStyle,
    m: GridMetrics,
) -> Element<'a, Message> {
    let grid = frame.grid();
    let mut line: Row<Element<'a, Message>> =
        Row::new().push(name_cell(frame, visible, row, style, m));
    let mut first = 0u32;
    for (g, group) in grid.groups().iter().enumerate() {
        let count = group.columns.len() as u32;
        if grid.view.is_group_collapsed(GroupId(g as u32)) {
            line = line.push(
                container(Space::new())
                    .width(m.collapsed_width)
                    .height(m.row_height)
                    .style(header_box(style, false))
                    .boxed(),
            );
        } else {
            for c in first..first + count {
                let at = CellRef::new(visible.id, ColumnId(c));
                line = line.push(super::grid_cell::view(frame, at, style, m));
            }
        }
        first += count;
    }
    line.boxed()
}

/// The cell a click on a row's name selects: the selected column when it
/// can be set on the row, else the first column that can.
fn name_target(frame: &Frame, row: RowId) -> Option<CellRef> {
    let grid = frame.grid();
    let cells = frame.cells();
    let editable = |c: ColumnId| cells.show(CellRef::new(row, c)).is_editable();
    let selected = grid
        .view
        .selected()
        .map(|c| c.column)
        .filter(|c| editable(*c));
    let total = grid.groups().iter().map(|g| g.columns.len()).sum::<usize>() as u32;
    selected
        .or_else(|| (0..total).map(ColumnId).find(|c| editable(*c)))
        .map(|c| CellRef::new(row, c))
}

/// The row's name cell: indent, chevron, kind tag, name and what it
/// carries (net count, scope, constraint sets).
fn name_cell<'a>(
    frame: &Frame<'a>,
    visible: VisibleRow,
    row: &RowRef,
    style: GridStyle,
    m: GridMetrics,
) -> Element<'a, Message> {
    let (i18n, tokens) = (frame.i18n, frame.tokens);
    let toggle: Element<'a, Message> = if visible.has_children {
        mouse_area(chevron(visible.is_open, TS.body, style.muted_ink))
            .on_press(act(frame, GridMessage::RowToggled(visible.id)))
            .interaction(iced::mouse::Interaction::Pointer)
            .boxed()
    } else {
        Space::new().width(m.glyph).boxed()
    };
    let mut face: Row<Element<'a, Message>> = Row::new()
        .spacing(m.pad / 2.0)
        .align_y(Vertical::Center)
        .push(Space::new().width(m.indent * visible.depth as f32).boxed())
        .push(toggle)
        .push(kind_tag(frame, row));
    let mut name = text(row_name(frame, row))
        .size(TS.body)
        .color(style.ink)
        .wrapping(iced::widget::text::Wrapping::None);
    if matches!(row, RowRef::Net(_) | RowRef::Scoped(_)) {
        name = name.font(crate::fonts::iced_font_for_family(
            crate::fonts::CHROME_MONO_FONT,
        ));
    }
    face = face.push(name.boxed());
    if let Some(extra) = row_extra(frame, visible.id, row) {
        face = face.push(
            text(extra)
                .size(TS.caption)
                .color(style.muted_ink)
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
        );
    }
    let r = resolve(tokens);
    for set in sets_on(row, &frame.sets) {
        face = face.push(
            container(
                text(t!(i18n, ids::RULES_GRID_SET_CHIP, "set" => set.as_str()))
                    .size(TS.caption)
                    .color(r.accent_text),
            )
            .padding(Padding::from([0.0, SP.xs]))
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(r.accent_soft)),
                border: Border {
                    radius: Radii::GRAPHITE.small.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed(),
        );
    }
    let is_selected = frame
        .grid()
        .view
        .selected()
        .is_some_and(|c| c.row == visible.id);
    let cell = container(face)
        .padding(Padding::from([0.0, m.pad]))
        .width(name_width(m))
        .height(m.row_height)
        .align_y(Vertical::Center)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            background: is_selected.then_some(Background::Color(style.accent_soft)),
            border: Border {
                color: style.line,
                width: HAIRLINE,
                ..Border::default()
            },
            ..container::Style::default()
        });
    match name_target(frame, visible.id) {
        Some(target) => mouse_area(cell)
            .on_press(act(frame, GridMessage::CellPressed(target)))
            .boxed(),
        None => cell.boxed(),
    }
}

/// The small kind tag before a row's name.
fn kind_tag<'a>(frame: &Frame<'a>, row: &RowRef) -> Element<'a, Message> {
    let r = resolve(frame.tokens);
    let label = match row {
        RowRef::All | RowRef::Heading(_) => ids::RULES_GRID_KIND_ALL,
        RowRef::Class(_) => ids::RULES_GRID_KIND_CLASS,
        RowRef::Net(_) => ids::RULES_GRID_KIND_NET,
        RowRef::Layer { .. } => ids::RULES_GRID_KIND_LAYER,
        RowRef::Scoped(_) => ids::RULES_GRID_KIND_RULE,
    };
    container(text(t!(frame.i18n, label)).size(TS.caption).color(r.ink3))
        .padding(Padding::from([0.0, SP.xs]))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(r.sunken)),
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A row's name: user content, or the worded All row and the layer by
/// its position (L1 is Top, the last is Bottom); a layer the board lacks
/// by the token the rules name it with.
pub(super) fn row_name(frame: &Frame, row: &RowRef) -> Label<'static> {
    let i18n = frame.i18n;
    match row {
        RowRef::All | RowRef::Heading(_) => t!(i18n, ids::RULES_GRID_ROW_ALL).into(),
        RowRef::Layer {
            number: 0, token, ..
        } => user(token.clone()).into(),
        RowRef::Layer { number: 1, .. } => t!(i18n, ids::RULES_GRID_LAYER_TOP, "n" => 1).into(),
        RowRef::Layer { number, count, .. } if number == count => {
            t!(i18n, ids::RULES_GRID_LAYER_BOTTOM, "n" => *number).into()
        }
        RowRef::Layer { number, .. } => {
            t!(i18n, ids::RULES_GRID_LAYER_INNER, "n" => *number).into()
        }
        other => user(other.name().to_string()).into(),
    }
}

/// What a row carries after its name: a class's net count, a scoped
/// rule's scope.
fn row_extra(frame: &Frame, id: RowId, row: &RowRef) -> Option<Localized> {
    let i18n = frame.i18n;
    match row {
        RowRef::Class(_) => {
            let nets = frame
                .grid()
                .view
                .rows()
                .iter()
                .filter(|r| r.parent == Some(id))
                .count();
            Some(t!(i18n, ids::RULES_GRID_ROW_NETS, "count" => nets))
        }
        RowRef::Scoped(name) => {
            let rule = frame.table.row(name)?;
            let mut scope = scope_text(&rule.scope);
            if let Some(layer) = &rule.layer {
                scope = format!("{scope} {layer}").trim().to_string();
            }
            if let Some([a, b]) = &rule.pair {
                scope = format!("{scope} {a}-{b}").trim().to_string();
            }
            if scope.is_empty() {
                return Some(t!(i18n, ids::RULES_SCOPE_ALL));
            }
            Some(t!(i18n, ids::RULES_GRID_ROW_SCOPE, "scope" => scope))
        }
        _ => None,
    }
}
