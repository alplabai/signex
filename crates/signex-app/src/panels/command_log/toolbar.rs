//! The filter row (search, source chips, result switch, Follow newest), the
//! panel's title-bar actions (Pause, Save log as…, Clear, More), and the
//! More strip.

use iced::widget::{Row, Space, column, container, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::chips::{ChipMetrics, ChipStyle, icon_chip};
use signex_widgets::{Label, ToggleSwitchStyle, segmented, text, text_input, toggle_switch};

use super::{
    CM, CommandLogMsg, CommandLogState, GLYPH_THEME, ICON, LogCounts, PanelContext, PanelMsg,
    ResultFilter, SP, SourceKind, TS, icon, layout, message,
};
use crate::panels::TitleAction;
use crate::styles::{self, HAIRLINE, ti, tokens::resolve};

/// The filter row, `width` wide (mockup row 161): the filters, then Follow
/// newest at the right end. The search field gives up width first; in a
/// side dock the filters wrap and Follow newest takes its own line.
pub(super) fn bar<'a>(
    ctx: &'a PanelContext,
    state: &'a CommandLogState,
    counts: &LogCounts,
    empty: bool,
    width: f32,
) -> Element<'a, PanelMsg> {
    let one_row = layout::filters_on_one_row(width);
    let filters = filter_group(ctx, state, counts, empty, width);
    let follow = follow_switch(ctx, state);
    let line: Element<'a, PanelMsg> = if one_row {
        row![filters, follow]
            .spacing(SP.m + SP.xs)
            .align_y(Alignment::Center)
            .boxed()
    } else {
        column![filters, container(follow).align_right(Length::Fill)]
            .spacing(SP.s + SP.xs)
            .width(Length::Fill)
            .boxed()
    };
    container(line)
        .padding([SP.s + SP.xs, SP.m + SP.xs])
        .width(Length::Fill)
        .clip(true)
        .boxed()
}

/// Search, source chips and the result switch, wrapping when they do not fit
/// on one line. In an empty session every filter reads 0 and is greyed.
fn filter_group<'a>(
    ctx: &'a PanelContext,
    state: &'a CommandLogState,
    counts: &LogCounts,
    empty: bool,
    width: f32,
) -> Element<'a, PanelMsg> {
    let i18n: &I18n = &ctx.i18n;
    let tokens = &ctx.tokens;
    let mut filters: Row<Element<'a, PanelMsg>> = Row::new()
        .width(Length::Fill)
        .spacing(SP.s + SP.xs)
        .align_y(Alignment::Center);
    filters = filters.push(search_field(
        tokens,
        i18n,
        &state.log.filters.query,
        layout::search_width(width),
    ));
    filters = filters.push(vsep(tokens));
    filters = filters.push(
        text(t!(i18n, ids::COMMAND_LOG_SOURCE))
            .size(TS.caption)
            .color(resolve(tokens).ink4)
            .boxed(),
    );
    let (style, metrics) = chip_look(tokens);
    for (kind, label, glyph) in [
        (
            SourceKind::Gui,
            ids::COMMAND_LOG_SOURCE_GUI,
            crate::icons::icon_log_mouse(GLYPH_THEME),
        ),
        (
            SourceKind::Cli,
            ids::COMMAND_LOG_SOURCE_CLI,
            crate::icons::icon_log_terminal(GLYPH_THEME),
        ),
        (
            SourceKind::Agent,
            ids::COMMAND_LOG_SOURCE_AGENT,
            crate::icons::icon_log_plug(GLYPH_THEME),
        ),
    ] {
        let count = u32::try_from(counts.of_source(kind)).unwrap_or(u32::MAX);
        filters = filters.push(icon_chip(
            glyph,
            t!(i18n, label),
            Some(count),
            state.log.filters.sources.contains(&kind),
            style,
            metrics,
            i18n,
            Some(message(CommandLogMsg::SourceToggled(kind))),
        ));
    }
    filters = filters.push(vsep(tokens));
    filters = filters.push(result_switch(
        tokens,
        i18n,
        state.log.filters.result,
        counts,
    ));
    // Wrapping in both layouts: a longer language takes a second line
    // instead of running under Follow newest.
    let filters = filters.wrap().vertical_spacing(SP.xs).boxed();
    signex_widgets::inert::unless_read_only_veiled(empty, veil(tokens), filters)
}

/// The Follow newest switch and its label.
fn follow_switch<'a>(ctx: &'a PanelContext, state: &'a CommandLogState) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    row![
        toggle_switch(
            state.follow_newest,
            toggle_look(tokens),
            Some(message(CommandLogMsg::FollowNewestToggled)),
        ),
        text(t!(ctx.i18n, ids::COMMAND_LOG_FOLLOW_NEWEST))
            .size(TS.body)
            .color(resolve(tokens).ink3)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .boxed()
}

/// The panel's dock title-bar actions (mockup row 161): Pause (Resume while
/// paused), Save log as…, Clear and More.
pub(crate) fn title_actions(ctx: &PanelContext) -> Vec<TitleAction> {
    let i18n: &I18n = &ctx.i18n;
    let tid = ctx.theme_id;
    let paused = ctx.command_log.log.paused;
    let action = |icon, tip, msg| TitleAction {
        icon,
        tip,
        on_press: Some(message(msg)),
    };
    vec![
        if paused {
            action(
                crate::icons::icon_log_play(tid),
                t!(i18n, ids::COMMAND_LOG_RESUME),
                CommandLogMsg::PauseToggled,
            )
        } else {
            action(
                crate::icons::icon_log_pause(tid),
                t!(i18n, ids::COMMAND_LOG_PAUSE),
                CommandLogMsg::PauseToggled,
            )
        },
        action(
            crate::icons::icon_log_save(tid),
            t!(i18n, ids::COMMAND_LOG_SAVE),
            CommandLogMsg::SaveRequested,
        ),
        action(
            crate::icons::icon_log_trash(tid),
            t!(i18n, ids::COMMAND_LOG_CLEAR),
            CommandLogMsg::Cleared,
        ),
        action(
            crate::icons::icon_log_more(tid),
            t!(i18n, ids::COMMAND_LOG_MORE),
            CommandLogMsg::MoreToggled,
        ),
    ]
}

/// What More opens: the Show view commands switch, under the bar's right end.
pub(super) fn more_strip<'a>(
    ctx: &'a PanelContext,
    state: &'a CommandLogState,
) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let switch = row![
        toggle_switch(
            state.log.show_view_commands,
            toggle_look(tokens),
            Some(message(CommandLogMsg::ViewCommandsToggled)),
        ),
        text(t!(ctx.i18n, ids::COMMAND_LOG_SHOW_VIEW_COMMANDS))
            .size(TS.body)
            .color(ti(tokens.text))
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    let r = resolve(tokens);
    let (raised, line) = (r.raised, ti(tokens.border));
    let card = container(switch)
        .padding([SP.xs * 2.0, SP.m + SP.xs])
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(raised)),
            border: Border {
                width: HAIRLINE,
                color: line,
                radius: styles::Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    container(card)
        .padding(Padding {
            top: 0.0,
            right: SP.m + SP.xs,
            bottom: SP.s + SP.xs,
            left: SP.m + SP.xs,
        })
        .align_right(Length::Fill)
        .boxed()
}

/// The search field: a glyph and a borderless input in a field frame.
fn search_field<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    query: &'a str,
    width: f32,
) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let (ink, ink4, accent) = (ti(tokens.text), r.ink4, ti(tokens.accent));
    let input = text_input(t!(i18n, ids::COMMAND_LOG_FILTER_PLACEHOLDER), query)
        .on_input(|text| message(CommandLogMsg::FilterChanged(text)))
        .padding(Padding::ZERO)
        .size(TS.body)
        .width(Length::Fill)
        .style(move |_: &iced::Theme, _| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder: ink4,
            value: ink,
            selection: Color { a: 0.4, ..accent },
        });
    container(
        row![
            icon(crate::icons::icon_chrome_search(GLYPH_THEME), ICON, ink4),
            input.boxed()
        ]
        .spacing(SP.s + SP.xs / 2.0)
        .align_y(Alignment::Center),
    )
    .padding([0.0, SP.m])
    .width(width)
    .height(CM.field)
    .center_y(CM.field)
    .style(styles::field(tokens, false))
    .boxed()
}

/// All / Failed N / Undone N.
fn result_switch<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    picked: ResultFilter,
    counts: &LogCounts,
) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    segmented(
        [
            (
                ResultFilter::All,
                Label::from(t!(i18n, ids::COMMAND_LOG_RESULT_ALL)),
            ),
            (
                ResultFilter::Failed,
                Label::from(t!(i18n, ids::COMMAND_LOG_RESULT_FAILED, "count" => counts.failed)),
            ),
            (
                ResultFilter::Undone,
                Label::from(t!(i18n, ids::COMMAND_LOG_RESULT_UNDONE, "count" => counts.undone)),
            ),
        ],
        picked,
        r.segmented(TS.body),
        |result| message(CommandLogMsg::ResultFilterPicked(result)),
    )
}

/// The source chips' look: off is a field pill, on the soft accent.
fn chip_look(tokens: &ThemeTokens) -> (ChipStyle, ChipMetrics) {
    let r = resolve(tokens);
    let style = ChipStyle {
        fill: r.field,
        edge: ti(tokens.border),
        hover: styles::tokens::composite_over(styles::tokens::hover_overlay(tokens), r.field),
        ink: r.ink3,
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    };
    (style, ChipMetrics::from_text_size(TS.body))
}

fn toggle_look(tokens: &ThemeTokens) -> ToggleSwitchStyle {
    let r = resolve(tokens);
    ToggleSwitchStyle::new(ti(tokens.accent), r.line_strong, Color::WHITE)
}

/// The fade over the greyed filters of an empty session.
fn veil(tokens: &ThemeTokens) -> Color {
    Color {
        a: 1.0 - styles::DISABLED_OPACITY,
        ..ti(tokens.panel_bg)
    }
}

/// A short vertical rule between groups of the bar.
fn vsep<'a>(tokens: &ThemeTokens) -> Element<'a, PanelMsg> {
    let line = resolve(tokens).line_soft;
    container(Space::new())
        .width(HAIRLINE)
        .height(CM.field - SP.s)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        })
        .boxed()
}
