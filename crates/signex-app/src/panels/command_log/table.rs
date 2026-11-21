//! The log table: Time, Source, Command, Arguments, Result and Took, one row
//! per listed command, oldest at the top. A failed row is tinted, the picked
//! row takes the soft accent and an undone row is struck through.

use iced::widget::{Column, Row, container, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, rich_text, span, text};

use super::{
    CM, CommandLogMsg, CommandLogState, LogRow, PanelContext, PanelMsg, SP, TS, message, mono,
    result_glyph, source_pill, text_tooltip, took_text, weighted,
};
use crate::styles::{
    HAIRLINE, ti,
    tokens::{composite_over, hover_overlay, resolve},
};

/// Column widths of the mock (Arguments takes the rest).
const TIME_W: f32 = 100.0;
const SOURCE_W: f32 = 82.0;
const COMMAND_W: f32 = 206.0;
const RESULT_W: f32 = 196.0;
const TOOK_W: f32 = 64.0;
/// Side padding of a cell (the mock's 8 px).
const CELL_PAD: f32 = SP.m;
/// Strength of the error tint of a failed row over the panel.
const FAILED_TINT: f32 = 0.6;
/// Mono size of the time, arguments and took cells (11.5 px).
const MONO_SIZE: f32 = TS.body * 11.5 / 12.0;
/// Mono size of the id under a label (10.5 px).
const ID_SIZE: f32 = TS.body * 10.5 / 12.0;
/// Edge of a result glyph (13 px).
const RESULT_ICON: f32 = TS.body * 13.0 / 12.0;

/// The header and the rows in a scroller that keeps the newest row in view
/// while Follow newest is on.
pub(super) fn view<'a>(
    ctx: &'a PanelContext,
    state: &'a CommandLogState,
    visible: &[&'a LogRow],
    shown: Option<u64>,
) -> Element<'a, PanelMsg> {
    let mut rows: Column<Element<'a, PanelMsg>> = Column::new().width(Length::Fill);
    for &row in visible {
        rows = rows.push(table_row(ctx, row, shown == Some(row.seq)));
    }
    let scroller = crate::panels::panel_scrollable(rows.boxed());
    let scroller = if state.follow_newest {
        scroller.anchor_bottom()
    } else {
        scroller
    };
    let mut col: Column<Element<'a, PanelMsg>> =
        Column::new().width(Length::Fill).height(Length::Fill);
    col = col.push(header(ctx));
    col = col.push(scroller.boxed());
    col.boxed()
}

/// One fixed-width (or filling) cell, its content centred vertically.
fn cell<'a>(content: Element<'a, PanelMsg>, width: Length, right: bool) -> Element<'a, PanelMsg> {
    let c = container(content)
        .padding([0.0, CELL_PAD])
        .width(width)
        .height(Length::Fill)
        .align_y(iced::alignment::Vertical::Center)
        .clip(true);
    if right {
        c.align_x(iced::alignment::Horizontal::Right).boxed()
    } else {
        c.boxed()
    }
}

fn header<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let i18n: &I18n = &ctx.i18n;
    let ink3 = resolve(tokens).ink3;
    let caption = |id: MessageId| -> Element<'a, PanelMsg> {
        text(t!(i18n, id))
            .size(TS.body)
            .font(weighted(iced::font::Weight::Medium))
            .color(ink3)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed()
    };
    let line = row![
        cell(
            caption(ids::COMMAND_LOG_COLUMN_TIME),
            Length::Fixed(TIME_W),
            false
        ),
        cell(
            caption(ids::COMMAND_LOG_COLUMN_SOURCE),
            Length::Fixed(SOURCE_W),
            false
        ),
        cell(
            caption(ids::COMMAND_LOG_COLUMN_COMMAND),
            Length::Fixed(COMMAND_W),
            false
        ),
        cell(
            caption(ids::COMMAND_LOG_COLUMN_ARGUMENTS),
            Length::Fill,
            false
        ),
        cell(
            caption(ids::COMMAND_LOG_COLUMN_RESULT),
            Length::Fixed(RESULT_W),
            false
        ),
        cell(
            caption(ids::COMMAND_LOG_COLUMN_TOOK),
            Length::Fixed(TOOK_W),
            true
        ),
    ]
    .width(Length::Fill);
    let (chrome, edge) = (ti(tokens.toolbar_bg), ti(tokens.border));
    container(line)
        .height(CM.row)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(chrome)),
            border: bottom_edge(edge),
            ..container::Style::default()
        })
        .boxed()
}

/// A 1 px line along the bottom only.
fn bottom_edge(color: Color) -> Border {
    Border {
        bottom: iced::border::Side::default().color(color).width(HAIRLINE),
        ..Border::default()
    }
}

/// The fill of a row: picked, failed or neither.
fn row_fill(tokens: &ThemeTokens, row: &LogRow, picked: bool) -> Option<Color> {
    let r = resolve(tokens);
    if picked {
        Some(r.accent_soft)
    } else if row.outcome.is_failed() {
        let tint = Color {
            a: r.err_soft.a * FAILED_TINT,
            ..r.err_soft
        };
        Some(composite_over(tint, ti(tokens.panel_bg)))
    } else {
        None
    }
}

/// One command: a button over its cells that picks it.
fn table_row<'a>(ctx: &'a PanelContext, row: &'a LogRow, picked: bool) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let i18n: &I18n = &ctx.i18n;
    let r = resolve(tokens);
    let undone = row.undone_at.is_some();
    let ink = if undone { r.ink4 } else { ti(tokens.text) };
    let muted = r.ink4;
    let mono_cell = |value: String, color: Color| -> Element<'a, PanelMsg> {
        text(UserText::new(value))
            .size(MONO_SIZE)
            .font(mono())
            .color(color)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed()
    };
    let arguments = row.arguments_text();
    let arguments_cell = if arguments.is_empty() {
        text(t!(i18n, ids::COMMAND_LOG_NO_VALUE))
            .size(MONO_SIZE)
            .font(mono())
            .color(muted)
            .boxed()
    } else {
        text_tooltip(tokens, mono_cell(arguments.clone(), ink), arguments)
    };
    let cells: Row<Element<'a, PanelMsg>> = row![
        cell(
            mono_cell(row.at.format("%H:%M:%S%.3f").to_string(), muted),
            Length::Fixed(TIME_W),
            false
        ),
        cell(
            source_pill(tokens, i18n, &row.source),
            Length::Fixed(SOURCE_W),
            false
        ),
        cell(
            command_cell(row, ink, muted),
            Length::Fixed(COMMAND_W),
            false
        ),
        cell(arguments_cell, Length::Fill, false),
        cell(
            result_cell(tokens, i18n, row),
            Length::Fixed(RESULT_W),
            false
        ),
        cell(
            text(took_text(i18n, row.took))
                .size(MONO_SIZE)
                .font(mono())
                .color(muted)
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
            Length::Fixed(TOOK_W),
            true
        ),
    ]
    .width(Length::Fill)
    .height(Length::Fill);
    let fill = row_fill(tokens, row, picked);
    let (hover, line, text_color) = (hover_overlay(tokens), r.line_soft, ink);
    button(cells)
        .width(Length::Fill)
        .height(CM.row)
        .padding(0.0)
        .on_press(message(CommandLogMsg::RowSelected(row.seq)))
        .style(move |_: &Theme, status: button::Status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            let background = match (fill, hovered) {
                (Some(fill), _) if picked => fill,
                (Some(fill), true) => composite_over(hover, fill),
                (Some(fill), false) => fill,
                (None, true) => hover,
                (None, false) => Color::TRANSPARENT,
            };
            button::Style {
                background: Some(Background::Color(background)),
                text_color,
                border: bottom_edge(line),
                ..button::Style::default()
            }
        })
        .boxed()
}

/// The label (medium weight, struck through once undone) and the catalog id.
fn command_cell<'a>(row: &'a LogRow, ink: Color, id_ink: Color) -> Element<'a, PanelMsg> {
    let undone = row.undone_at.is_some();
    // The label as it was rendered from the catalog when the command ran.
    let label = span::<()>(UserText::new(row.label.clone()))
        .font(weighted(iced::font::Weight::Medium))
        .color(ink)
        .strikethrough(undone);
    row![
        rich_text::<(), PanelMsg, Theme>(vec![label])
            .size(TS.body)
            .wrapping(iced::widget::text::Wrapping::None),
        text(UserText::new(row.command.clone()))
            .size(ID_SIZE)
            .font(mono())
            .color(id_ink)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(SP.s + SP.xs / 2.0)
    .align_y(Alignment::Center)
    .boxed()
}

/// The result glyph and text; a failed row shows its error code with the
/// whole error as the tooltip.
fn result_cell<'a>(tokens: &ThemeTokens, i18n: &I18n, row: &'a LogRow) -> Element<'a, PanelMsg> {
    let (glyph, tint) = result_glyph(tokens, &row.outcome);
    let ink = if row.outcome.is_failed() {
        ti(tokens.error)
    } else if row.undone_at.is_some() {
        resolve(tokens).ink4
    } else {
        ti(tokens.text)
    };
    let label: Element<'a, PanelMsg> = match row.outcome.short_text() {
        Some(short) => text(UserText::new(short.to_string()))
            .size(TS.body)
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed(),
        None => text(t!(i18n, ids::COMMAND_LOG_DONE))
            .size(TS.body)
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed(),
    };
    let line = row![super::icon(glyph, RESULT_ICON, tint), label]
        .spacing(SP.xs * 2.0)
        .align_y(Alignment::Center)
        .boxed();
    match row.outcome.text() {
        Some(full) => text_tooltip(tokens, line, full.to_string()),
        None => line,
    }
}
