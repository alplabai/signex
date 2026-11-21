//! The filter bar (search, source chips, result switch, Follow newest) with
//! the panel's actions at its end (Pause, Save log as…, Clear, More), and
//! the More strip.

use iced::widget::{Row, Space, container, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::chips::{ChipMetrics, ChipStyle, chip};
use signex_widgets::{Label, ToggleSwitchStyle, segmented, text, text_input, toggle_switch};

use super::{
    CM, CommandLogMsg, CommandLogState, GLYPH_THEME, ICON, LogCounts, PanelContext, PanelMsg,
    ResultFilter, SP, SourceKind, TS, icon, icon_button, message,
};
use crate::styles::{self, HAIRLINE, ti, tokens::resolve};

/// Width of the search field (mockup row 161).
const SEARCH_WIDTH: f32 = 250.0;

/// The filter bar. In an empty session every filter reads 0 and is greyed.
pub(super) fn bar<'a>(
    ctx: &'a PanelContext,
    state: &'a CommandLogState,
    counts: &LogCounts,
    empty: bool,
) -> Element<'a, PanelMsg> {
    let i18n: &I18n = &ctx.i18n;
    let tokens = &ctx.tokens;
    let ink3 = resolve(tokens).ink3;
    let mut filters: Row<Element<'a, PanelMsg>> =
        Row::new().spacing(SP.s + SP.xs).align_y(Alignment::Center);
    filters = filters.push(search_field(tokens, i18n, &state.log.filters.query));
    filters = filters.push(vsep(tokens));
    filters = filters.push(
        text(t!(i18n, ids::COMMAND_LOG_SOURCE))
            .size(TS.caption)
            .color(resolve(tokens).ink4)
            .boxed(),
    );
    let (style, metrics) = chip_look(tokens);
    for (kind, label) in [
        (SourceKind::Gui, ids::COMMAND_LOG_SOURCE_GUI),
        (SourceKind::Cli, ids::COMMAND_LOG_SOURCE_CLI),
        (SourceKind::Agent, ids::COMMAND_LOG_SOURCE_AGENT),
    ] {
        let count = u32::try_from(counts.of_source(kind)).unwrap_or(u32::MAX);
        filters = filters.push(chip(
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
    let filters: Element<'a, PanelMsg> = if empty {
        signex_widgets::inert::unless_read_only_veiled(true, veil(tokens), filters.boxed())
    } else {
        filters.boxed()
    };
    let follow = row![
        toggle_switch(
            state.follow_newest,
            toggle_look(tokens),
            Some(message(CommandLogMsg::FollowNewestToggled)),
        ),
        text(t!(i18n, ids::COMMAND_LOG_FOLLOW_NEWEST))
            .size(TS.body)
            .color(ink3)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    let line = row![
        filters,
        Space::new().width(Length::Fill),
        follow,
        vsep(tokens),
        actions(ctx, state),
    ]
    .spacing(SP.s + SP.xs)
    .align_y(Alignment::Center);
    container(line)
        .padding([SP.s + SP.xs, SP.m + SP.xs])
        .width(Length::Fill)
        .clip(true)
        .boxed()
}

/// The panel's actions: Pause (Resume while paused), Save log as…, Clear
/// and More.
fn actions<'a>(ctx: &'a PanelContext, state: &'a CommandLogState) -> Element<'a, PanelMsg> {
    let i18n: &I18n = &ctx.i18n;
    let tokens = &ctx.tokens;
    let (pause_glyph, pause_tip) = if state.log.paused {
        (
            crate::icons::icon_log_play(GLYPH_THEME),
            t!(i18n, ids::COMMAND_LOG_RESUME),
        )
    } else {
        (
            crate::icons::icon_log_pause(GLYPH_THEME),
            t!(i18n, ids::COMMAND_LOG_PAUSE),
        )
    };
    row![
        icon_button(
            tokens,
            pause_glyph,
            pause_tip,
            state.log.paused,
            message(CommandLogMsg::PauseToggled),
        ),
        icon_button(
            tokens,
            crate::icons::icon_log_save(GLYPH_THEME),
            t!(i18n, ids::COMMAND_LOG_SAVE),
            false,
            message(CommandLogMsg::SaveRequested),
        ),
        icon_button(
            tokens,
            crate::icons::icon_log_trash(GLYPH_THEME),
            t!(i18n, ids::COMMAND_LOG_CLEAR),
            false,
            message(CommandLogMsg::Cleared),
        ),
        icon_button(
            tokens,
            crate::icons::icon_log_more(GLYPH_THEME),
            t!(i18n, ids::COMMAND_LOG_MORE),
            state.more_open,
            message(CommandLogMsg::MoreToggled),
        ),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center)
    .boxed()
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
fn search_field<'a>(tokens: &ThemeTokens, i18n: &I18n, query: &'a str) -> Element<'a, PanelMsg> {
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
    .width(SEARCH_WIDTH)
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
