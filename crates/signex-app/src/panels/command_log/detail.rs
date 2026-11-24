//! The detail pane of the picked row: a failed row's full error first, then
//! time, client, duration and undo state, the arguments as JSON, the result
//! and the row as a CLI line, scrolling; Replay and Jump to stay pinned
//! under them, so a short dock never hides them (mockup row 161).

use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::{Column, Space, container, row, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::{
    CLI_CLIENT, CommandLogMsg, CommandLogState, CopyTarget, GLYPH_THEME, JumpTarget, LogSource,
    Look, PanelContext, PanelMsg, SP, TS, icon, icon_button, labelled_button, message, mono,
    source_pill, took_text, weighted,
};
use crate::styles::{self, HAIRLINE, Radii, ti, tokens::resolve};

/// Width of the key column of the facts grid (mock: 76 px).
const KEY_W: f32 = 76.0;
/// Mono size of the error, the arguments and the times (11.5 px).
const MONO_SIZE: f32 = TS.body * 11.5 / 12.0;
/// Mono size of the CLI line (11 px).
const CLI_SIZE: f32 = TS.body * 11.0 / 12.0;
/// Space above a section label (the mock's 10 px and 12 px).
const SECTION_GAP: f32 = SP.m + SP.xs;

/// The pane for `row`: the scrolling body over the pinned actions.
pub(super) fn view<'a>(
    ctx: &'a PanelContext,
    state: &'a CommandLogState,
    row: &'a super::LogRow,
    width: f32,
) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let scrolled = crate::panels::panel_scrollable(
        container(body(ctx, state, row))
            .padding([SP.l, SP.l + SP.xs])
            .width(Length::Fill)
            .boxed(),
    );
    let edge = ti(tokens.border);
    let pinned = container(actions(ctx, row))
        .padding([SP.m, SP.l + SP.xs])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border {
                top: iced::border::Side::default().color(edge).width(HAIRLINE),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let pane: Column<Element<'a, PanelMsg>> = Column::new()
        .push(scrolled.boxed())
        .push(pinned.boxed())
        .width(Length::Fill)
        .height(Length::Fill);
    container(pane)
        .width(width)
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border {
                left: iced::border::Side::default().color(edge).width(HAIRLINE),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// What scrolls: the heading, a failed row's error, the facts, the
/// arguments, the result and the CLI line.
fn body<'a>(
    ctx: &'a PanelContext,
    state: &'a CommandLogState,
    row: &'a super::LogRow,
) -> Column<Element<'a, PanelMsg>> {
    let tokens = &ctx.tokens;
    let i18n: &I18n = &ctx.i18n;
    let r = resolve(tokens);
    let failed = row.outcome.is_failed();
    let mut col = heading(ctx, row);
    if failed {
        col = col.push(error_section(ctx, state, row));
    }
    col = col.push(gap(SP.m));
    col = col.push(facts(ctx, row));
    col = col.push(gap(SP.m));
    col = col.push(section(tokens, i18n, ids::COMMAND_LOG_DETAIL_ARGUMENTS));
    col = col.push(gap(SP.xs * 2.0));
    col = col.push(match row.arguments_json() {
        Some(json) => code_block(tokens, json),
        None => text(t!(i18n, ids::COMMAND_LOG_NO_ARGUMENTS))
            .size(TS.caption)
            .color(r.ink4)
            .boxed(),
    });
    if !failed {
        col = col.push(gap(SECTION_GAP));
        col = col.push(section(tokens, i18n, ids::COMMAND_LOG_DETAIL_RESULT));
        col = col.push(gap(SP.xs * 2.0));
        let result = match row.outcome.text() {
            Some(said) => text(UserText::new(said.to_string())),
            None => text(t!(i18n, ids::COMMAND_LOG_DONE)),
        };
        col = col.push(result.size(TS.body).color(ti(tokens.text)).boxed());
    }
    col = col.push(gap(SECTION_GAP + SP.xs));
    col = col.push(section(tokens, i18n, ids::COMMAND_LOG_COPY_AS_CLI));
    col = col.push(gap(SP.xs * 2.0));
    col = col.push(cli_box(ctx, state, row));
    if failed {
        col = col.push(gap(SP.s + SP.xs));
        col = col.push(
            text(t!(i18n, ids::COMMAND_LOG_REPLAY_FAILED_HINT))
                .size(TS.caption)
                .color(r.ink4)
                .boxed(),
        );
    }
    col
}

/// The command's label with its source pill, and its catalog id.
fn heading<'a>(ctx: &'a PanelContext, row: &'a super::LogRow) -> Column<Element<'a, PanelMsg>> {
    let tokens = &ctx.tokens;
    let i18n: &I18n = &ctx.i18n;
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().width(Length::Fill);
    col = col.push(
        row![
            text(UserText::new(row.label.clone()))
                .size(TS.title)
                .font(weighted(iced::font::Weight::Semibold))
                .color(ti(tokens.text)),
            source_pill(tokens, i18n, &row.source),
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .boxed(),
    );
    col = col.push(
        text(UserText::new(row.command.clone()))
            .size(TS.caption)
            .font(mono())
            .color(resolve(tokens).ink4)
            .boxed(),
    );
    col
}

/// A failed row's full error, first, with Copy error.
fn error_section<'a>(
    ctx: &'a PanelContext,
    state: &'a CommandLogState,
    row: &'a super::LogRow,
) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let i18n: &I18n = &ctx.i18n;
    let copied = matches!(state.copied, Some((CopyTarget::Error, _)));
    let (glyph, label) = if copied {
        (
            crate::icons::icon_log_check(GLYPH_THEME),
            t!(i18n, ids::COMMAND_LOG_COPIED),
        )
    } else {
        (
            crate::icons::icon_log_copy(GLYPH_THEME),
            t!(i18n, ids::COMMAND_LOG_COPY_ERROR),
        )
    };
    Column::<Element<'a, PanelMsg>>::new()
        .width(Length::Fill)
        .push(gap(SECTION_GAP))
        .push(section(tokens, i18n, ids::COMMAND_LOG_DETAIL_RESULT))
        .push(gap(SP.s + SP.xs))
        .push(error_banner(tokens, row.outcome.text().unwrap_or_default()))
        .push(gap(SP.s + SP.xs))
        .push(labelled_button(
            tokens,
            Look::Field,
            true,
            glyph,
            label,
            Some(message(CommandLogMsg::CopyErrorRequested(row.seq))),
        ))
        .boxed()
}

fn gap<'a>(height: f32) -> Element<'a, PanelMsg> {
    Space::new().height(height).boxed()
}

/// An uppercase section label in the language's capitals.
fn section<'a>(tokens: &ThemeTokens, i18n: &I18n, id: MessageId) -> Element<'a, PanelMsg> {
    let words = t!(i18n, id).into_string();
    text(UserText::new(i18n.upper_for_display(&words)))
        .size(TS.section)
        .font(weighted(iced::font::Weight::Medium))
        .style(styles::section_label(tokens))
        .boxed()
}

/// The full error, wrapped and verbatim, on the error tint.
fn error_banner<'a>(tokens: &ThemeTokens, error: &str) -> Element<'a, PanelMsg> {
    let fill = resolve(tokens).err_soft;
    container(
        row![
            icon(
                crate::icons::icon_log_error(GLYPH_THEME),
                TS.body * 13.0 / 12.0,
                ti(tokens.error)
            ),
            text(UserText::new(error.to_string()))
                .size(MONO_SIZE)
                .font(mono())
                .color(ti(tokens.text))
                .width(Length::Fill),
        ]
        .spacing(SP.m)
        .align_y(Alignment::Start),
    )
    .padding([SP.m, SP.m + SP.xs])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// Time, Client, Took and Undo, keys in a 76 px column.
fn facts<'a>(ctx: &'a PanelContext, row: &'a super::LogRow) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let i18n: &I18n = &ctx.i18n;
    let r = resolve(tokens);
    let ink = ti(tokens.text);
    let plain = |label: Localized| -> Element<'a, PanelMsg> {
        text(label).size(TS.body).color(ink).boxed()
    };
    let code = |value: String| -> Element<'a, PanelMsg> {
        text(UserText::new(value))
            .size(MONO_SIZE)
            .font(mono())
            .color(ink)
            .boxed()
    };
    let client = match &row.source {
        LogSource::Gui { window: None } => plain(t!(i18n, ids::COMMAND_LOG_CLIENT_WINDOW)),
        LogSource::Gui {
            window: Some(document),
        } => plain(t!(
            i18n, ids::COMMAND_LOG_CLIENT_UNDOCKED,
            "document" => document.as_str()
        )),
        LogSource::Cli { .. } => code(CLI_CLIENT.to_string()),
        LogSource::Agent {
            client: Some(client),
            ..
        } => code(client.clone()),
        LogSource::Agent {
            client: None,
            pid: Some(pid),
        } => plain(t!(i18n, ids::COMMAND_LOG_CLIENT_PID, "pid" => *pid)),
        LogSource::Agent {
            client: None,
            pid: None,
        } => plain(t!(i18n, ids::COMMAND_LOG_NO_VALUE)),
    };
    let undo = if row.outcome.is_failed() {
        t!(i18n, ids::COMMAND_LOG_UNDO_NOTHING_CHANGED)
    } else if let Some(at) = row.undone_at {
        t!(
            i18n, ids::COMMAND_LOG_UNDO_UNDONE_AT,
            "time" => at.format("%H:%M:%S").to_string()
        )
    } else if row.undoable {
        t!(i18n, ids::COMMAND_LOG_UNDO_UNDOABLE)
    } else {
        t!(i18n, ids::COMMAND_LOG_UNDO_READ_ONLY)
    };
    let key = |id: MessageId| -> Element<'a, PanelMsg> {
        container(text(t!(i18n, id)).size(TS.body).color(r.ink4))
            .width(KEY_W)
            .boxed()
    };
    let line = |id: MessageId, value: Element<'a, PanelMsg>| -> Element<'a, PanelMsg> {
        row![key(id), container(value).width(Length::Fill)]
            .spacing(SP.m + SP.xs)
            .align_y(Alignment::Center)
            .boxed()
    };
    let mut grid: Column<Element<'a, PanelMsg>> = Column::new().spacing(SP.xs * 2.0);
    grid = grid.push(line(
        ids::COMMAND_LOG_DETAIL_TIME,
        code(row.at.format("%Y-%m-%d %H:%M:%S%.3f").to_string()),
    ));
    grid = grid.push(line(ids::COMMAND_LOG_DETAIL_CLIENT, client));
    grid = grid.push(line(
        ids::COMMAND_LOG_DETAIL_TOOK,
        text(took_text(i18n, row.took))
            .size(MONO_SIZE)
            .font(mono())
            .color(ink)
            .boxed(),
    ));
    grid = grid.push(line(ids::COMMAND_LOG_DETAIL_UNDO, plain(undo)));
    grid.boxed()
}

/// The arguments as indented JSON on the sunken surface.
fn code_block<'a>(tokens: &ThemeTokens, json: String) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let (fill, line) = (r.sunken, ti(tokens.border));
    let body = text(UserText::new(json))
        .size(MONO_SIZE)
        .line_height(1.5)
        .font(mono())
        .color(ti(tokens.text))
        .wrapping(iced::widget::text::Wrapping::None);
    container(
        scrollable(container(body).padding([SP.m, SP.m + SP.xs]))
            .direction(Direction::Horizontal(
                Scrollbar::new().width(SP.xs).scroller_width(SP.xs),
            ))
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            width: HAIRLINE,
            color: line,
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// The row as a `signex-cli run` line, and its copy button.
fn cli_box<'a>(
    ctx: &'a PanelContext,
    state: &'a CommandLogState,
    row: &'a super::LogRow,
) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let i18n: &I18n = &ctx.i18n;
    let r = resolve(tokens);
    let placeholder = t!(i18n, ids::COMMAND_LOG_CLI_NO_PROJECT).into_string();
    let line = text(UserText::new(row.cli_line(&placeholder)))
        .size(CLI_SIZE)
        .font(mono())
        .color(ti(tokens.text))
        .wrapping(iced::widget::text::Wrapping::None);
    let copied = matches!(state.copied, Some((CopyTarget::Cli, _)));
    let glyph = if copied {
        crate::icons::icon_log_check(GLYPH_THEME)
    } else {
        crate::icons::icon_log_copy(GLYPH_THEME)
    };
    let tip = if copied {
        t!(i18n, ids::COMMAND_LOG_COPIED)
    } else {
        t!(i18n, ids::COMMAND_LOG_COPY)
    };
    let (fill, edge) = (r.sunken, ti(tokens.border));
    container(
        row![
            scrollable(line)
                .direction(Direction::Horizontal(
                    Scrollbar::new().width(SP.xs).scroller_width(SP.xs)
                ))
                .width(Length::Fill),
            icon_button(
                tokens,
                glyph,
                tip,
                false,
                message(CommandLogMsg::CopyCliRequested(row.seq))
            ),
        ]
        .spacing(SP.s + SP.xs)
        .align_y(Alignment::Center),
    )
    .padding(iced::Padding {
        left: SP.m + SP.xs,
        ..iced::Padding::from(SP.xs)
    })
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            width: HAIRLINE,
            color: edge,
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// Replay, and Jump to the object the arguments name (greyed when they
/// name none).
fn actions<'a>(ctx: &'a PanelContext, row: &'a super::LogRow) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let i18n: &I18n = &ctx.i18n;
    let target = row.jump_target();
    let jump_label = match &target {
        Some(JumpTarget::Designator(name) | JumpTarget::Net(name)) => {
            t!(i18n, ids::COMMAND_LOG_JUMP_TO, "target" => name.as_str())
        }
        Some(JumpTarget::Point(_)) | None => t!(i18n, ids::COMMAND_LOG_JUMP_TO_OBJECT),
    };
    row![
        labelled_button(
            tokens,
            Look::Primary,
            false,
            crate::icons::icon_log_play(GLYPH_THEME),
            t!(i18n, ids::COMMAND_LOG_REPLAY),
            Some(message(CommandLogMsg::ReplayRequested(row.seq))),
        ),
        labelled_button(
            tokens,
            Look::Field,
            false,
            crate::icons::icon_log_jump(GLYPH_THEME),
            jump_label,
            target
                .is_some()
                .then(|| message(CommandLogMsg::JumpRequested(row.seq))),
        ),
    ]
    .spacing(SP.s + SP.xs)
    .align_y(Alignment::Center)
    .boxed()
}
