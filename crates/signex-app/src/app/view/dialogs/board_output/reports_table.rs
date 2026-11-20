//! Board Reports' table (mockup row 117): a header that stays put over the scrolling
//! rows, click-to-sort headings, 26 px rows, numbers right-aligned in the
//! mono face, long cells on one line ending in an ellipsis with the full
//! text on hover, and a centred empty state.

use std::f32::consts::FRAC_PI_2;

use iced::alignment::Horizontal;
use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Column, Row, Space, column, container, scrollable};
use iced::{Alignment, Background, Element, Length, Radians, Rotation, Theme, Widget as _};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_output::{BoardReportKind, ReportColumn};
use signex_types::coord::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text, tooltip};

use super::reports::cell_text;
use super::{SP, TS, kit, out};
use crate::app::{BoardOutputMsg, BoardReportsState, Message};
use crate::styles::{chrome_px, ti, tokens};

/// Height of a heading and of a row (26 px).
pub(super) const ROW_H: f32 = chrome_px(26.0 * 11.0 / 12.0);
/// Side padding of a cell (8 px).
const CELL_PAD: f32 = SP.m;

pub(super) fn column_label(column: ReportColumn) -> MessageId {
    use ReportColumn as C;
    match column {
        C::Designators => ids::BOARD_REPORT_COL_DESIGNATORS,
        C::Quantity => ids::BOARD_REPORT_COL_QUANTITY,
        C::Value => ids::BOARD_REPORT_COL_VALUE,
        C::Footprint => ids::BOARD_REPORT_COL_FOOTPRINT,
        C::Description => ids::BOARD_REPORT_COL_DESCRIPTION,
        C::Type => ids::BOARD_REPORT_COL_TYPE,
        C::Net => ids::BOARD_REPORT_COL_NET,
        C::NodeCount => ids::BOARD_REPORT_COL_NODE_COUNT,
        C::Nodes => ids::BOARD_REPORT_COL_NODES,
        C::Length => ids::BOARD_REPORT_COL_LENGTH,
        C::Tracks => ids::BOARD_REPORT_COL_TRACKS,
        C::Vias => ids::BOARD_REPORT_COL_VIAS,
        C::Designator => ids::BOARD_REPORT_COL_DESIGNATOR,
        C::X => ids::BOARD_REPORT_COL_X,
        C::Y => ids::BOARD_REPORT_COL_Y,
        C::Rotation => ids::BOARD_REPORT_COL_ROTATION,
        C::Side => ids::BOARD_REPORT_COL_SIDE,
    }
}

/// Whether `column` holds numbers: right-aligned, in the mono face.
pub(super) fn is_numeric(column: ReportColumn) -> bool {
    use ReportColumn as C;
    matches!(
        column,
        C::Quantity | C::NodeCount | C::Length | C::Tracks | C::Vias | C::X | C::Y | C::Rotation
    )
}

/// Whether `column` holds identifiers (net names, designators) set in the
/// mono face like the numbers.
fn is_mono(column: ReportColumn) -> bool {
    is_numeric(column) || matches!(column, ReportColumn::Net | ReportColumn::Designator)
}

/// How wide `column` is in a report of `kind`: a fixed width, or the room
/// left (`Fill`) for the one column a report lets grow.
pub(super) fn column_width(kind: BoardReportKind, column: ReportColumn) -> Length {
    use ReportColumn as C;
    let px = |at_12: f32| Length::Fixed(chrome_px(at_12 * 11.0 / 12.0));
    match column {
        C::Nodes | C::Description => Length::Fill,
        C::Net if kind == BoardReportKind::NetLength => Length::Fill,
        C::Footprint if kind == BoardReportKind::Components => Length::Fill,
        C::Designators => px(150.0),
        C::Footprint => px(140.0),
        C::Value | C::Net | C::Designator => px(110.0),
        C::Type => px(100.0),
        C::Length => px(120.0),
        C::X | C::Y | C::Rotation => px(90.0),
        C::Tracks | C::Side => px(80.0),
        C::NodeCount | C::Vias => px(70.0),
        C::Quantity => px(60.0),
    }
}

/// The headings over the rows `shown` (indices into the report's rows),
/// or the centred empty state.
pub(super) fn table<'a>(
    state: &'a BoardReportsState,
    shown: &[usize],
    unit: Unit,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let report = &state.report;
    if shown.is_empty() {
        let words = if report.rows.is_empty() {
            ids::BOARD_REPORTS_EMPTY
        } else {
            ids::BOARD_REPORTS_NO_MATCH
        };
        return container(
            text(t!(i18n, words))
                .size(TS.body)
                .color(tokens::resolve(tokens).ink3),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .boxed();
    }
    let r = tokens::resolve(tokens);
    let mut rows: Column<Element<'a, Message>> = Column::new().width(Length::Fill);
    for &index in shown {
        let Some(cells) = report.rows.get(index) else {
            continue;
        };
        let mut line: Row<Element<'a, Message>> = Row::new().height(Length::Fixed(ROW_H));
        for (column, cell) in report.columns.iter().zip(cells) {
            line = line.push(cell_view(
                *column,
                report.kind,
                cell_text(cell, unit, i18n),
                tokens,
            ));
        }
        rows = rows.push(line.boxed());
        rows = rows.push(kit::rule(r.line_soft));
    }
    column![
        headings(state, tokens, i18n),
        kit::rule(ti(tokens.border)),
        scrollable(rows)
            .direction(crate::styles::thin_vertical())
            .height(Length::Fill),
    ]
    .height(Length::Fill)
    .boxed()
}

/// One cell: one line, cut with an ellipsis, the whole text on hover.
fn cell_view<'a>(
    column: ReportColumn,
    kind: BoardReportKind,
    words: signex_widgets::Label<'static>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let full = words.as_str().to_string();
    let mut cell = text(words)
        .size(if is_mono(column) { TS.caption } else { TS.body })
        .color(ti(tokens.text))
        .wrapping(Wrapping::None)
        .ellipsis(Ellipsis::End)
        .width(Length::Fill);
    if is_numeric(column) {
        cell = cell.align_x(Horizontal::Right);
    }
    if is_mono(column) {
        cell = cell.font(super::super::graphite::mono());
    }
    let boxed = container(cell)
        .padding([0.0, CELL_PAD])
        .width(column_width(kind, column))
        .height(Length::Fill)
        .center_y(Length::Fill);
    let tip =
        container(text(signex_i18n::UserText::new(full)).size(TS.body)).padding([SP.xs, SP.s]);
    tooltip(boxed, tip, tooltip::Position::Bottom)
        .style(crate::styles::context_menu(tokens))
        .boxed()
}

/// The headings: each a button that sorts by its column, the sorted one in
/// the full ink with an arrow for its direction.
fn headings<'a>(
    state: &BoardReportsState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let chrome = r.raised;
    let mut line: Row<Element<'a, Message>> = Row::new().height(Length::Fixed(ROW_H));
    for column in &state.report.columns {
        let sorted = state.sort.filter(|s| s.column == *column);
        let ink = if sorted.is_some() {
            ti(tokens.text)
        } else {
            r.ink3
        };
        let label = text(t!(i18n, column_label(*column)))
            .size(TS.body)
            .font(kit::medium())
            .color(ink)
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End);
        let mut words: Row<Element<'a, Message>> = Row::new()
            .spacing(SP.xs + SP.xs / 2.0)
            .align_y(Alignment::Center);
        if is_numeric(*column) {
            words = words.push(Space::new().width(Length::Fill).boxed());
        }
        words = words.push(label.boxed());
        if let Some(sort) = sorted {
            words = words.push(sort_arrow(sort.descending, r.accent_text));
        }
        line = line.push(
            button(container(words).center_y(Length::Fill))
                .padding([0.0, CELL_PAD])
                .width(column_width(state.report.kind, *column))
                .height(Length::Fill)
                .on_press(out(BoardOutputMsg::ReportSortPicked(*column)))
                .style(crate::styles::ghost_button(tokens))
                .boxed(),
        );
    }
    container(line)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(chrome)),
            ..container::Style::default()
        })
        .boxed()
}

/// The arrow of the sorted heading: down for descending, up for
/// ascending.
fn sort_arrow<'a>(descending: bool, ink: iced::Color) -> Element<'a, Message> {
    let turn = if descending { FRAC_PI_2 } else { -FRAC_PI_2 };
    signex_widgets::chevron::chevron(true, TS.caption, ink)
        .rotation(Rotation::Floating(Radians(turn)))
        .boxed()
}
