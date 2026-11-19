//! The form above the results: the Find field (match pill and toggles), the
//! Replace field and the scope switch.

use iced::widget::{Row, column, container, row, text_input as iced_text_input};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, button, segmented, text, text_input};

use super::{LABEL_W, SP, TS, segmented_style};
use crate::find_replace::{FindReplaceMsg, FindReplaceState, FindScope};
use crate::styles::{ChromeMetrics, PillTone, Radii, ti, tokens};

const METRICS: ChromeMetrics = ChromeMetrics::DEFAULT;

pub(super) fn form<'a>(
    state: &'a FindReplaceState,
    theme_tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindReplaceMsg> {
    let mut form = column![labelled(
        t!(i18n, ids::FIND_LABEL),
        find_field(state, theme_tokens, i18n),
        theme_tokens,
    )]
    .spacing(SP.m);
    if state.replace_mode {
        let input = text_input(
            t!(i18n, ids::PLACEHOLDER_FIND_REPLACEMENT),
            &state.replacement,
        )
        .style(bare_input(theme_tokens))
        .on_input(FindReplaceMsg::ReplacementChanged)
        .padding([0.0, SP.m])
        .size(TS.body)
        .width(Length::Fill);
        let field = framed(input.boxed(), false, theme_tokens);
        form = form.push(labelled(
            t!(i18n, ids::FIND_REPLACE_LABEL),
            field,
            theme_tokens,
        ));
    }
    form = form.push(scope_row(state, theme_tokens, i18n));
    form.boxed()
}

/// A form row: the muted label in the fixed label column, then `control`.
fn labelled<'a>(
    label: Localized,
    control: Element<'a, FindReplaceMsg>,
    theme_tokens: &ThemeTokens,
) -> Element<'a, FindReplaceMsg> {
    row![
        text(label)
            .size(TS.body)
            .color(tokens::muted_text(theme_tokens))
            .width(Length::Fixed(LABEL_W)),
        control,
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .boxed()
}

/// The Graphite field frame around a borderless input and its extras. An
/// invalid pattern draws the frame in the error colour.
fn framed<'a>(
    content: Element<'a, FindReplaceMsg>,
    invalid: bool,
    theme_tokens: &ThemeTokens,
) -> Element<'a, FindReplaceMsg> {
    let error = ti(theme_tokens.error);
    let base = crate::styles::field(theme_tokens, false);
    container(content)
        .height(Length::Fixed(METRICS.field))
        .width(Length::Fill)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |theme: &Theme| {
            let mut style = base(theme);
            if invalid {
                style.border.color = error;
            }
            style
        })
        .boxed()
}

/// A text input without its own frame: the field around it draws one.
fn bare_input(
    theme_tokens: &ThemeTokens,
) -> impl Fn(&Theme, iced_text_input::Status) -> iced_text_input::Style + 'static {
    let placeholder = tokens::resolve(theme_tokens).ink4;
    let value = ti(theme_tokens.text);
    let selection = tokens::selection_tint(theme_tokens);
    move |_, _| iced_text_input::Style {
        background: Background::Color(iced::Color::TRANSPARENT),
        border: Border::default(),
        placeholder,
        value,
        selection,
    }
}

/// The Find field: search glyph, the query, the match pill and the three
/// match toggles. Board results are matched as before #2525 (a substring,
/// any case), so a board shows no toggles.
fn find_field<'a>(
    state: &'a FindReplaceState,
    theme_tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindReplaceMsg> {
    let invalid = state.board.is_none() && state.find_query().is_err();
    let ink4 = tokens::resolve(theme_tokens).ink4;
    let glyph = iced::widget::svg(crate::icons::icon_chrome_search(
        signex_types::theme::ThemeId::GraphiteDark,
    ))
    .width(TS.body)
    .height(TS.body)
    .style(move |_: &Theme, _| iced::widget::svg::Style {
        color: Some(ink4),
        ..Default::default()
    });
    let input = text_input(t!(i18n, ids::PLACEHOLDER_FIND_QUERY), &state.query)
        .style(bare_input(theme_tokens))
        .on_input(FindReplaceMsg::QueryChanged)
        .padding([0.0, SP.s])
        .size(TS.body)
        .width(Length::Fill);
    let mut line: Row<Element<'a, FindReplaceMsg>> = row![glyph, input]
        .spacing(SP.xs)
        .align_y(Alignment::Center)
        .padding([0.0, SP.m]);
    if let Some((label, tone)) = pill_text(state, i18n) {
        line = line.push(
            container(text(label).size(TS.caption))
                .padding([SP.xs, SP.s])
                .style(crate::styles::pill(theme_tokens, tone))
                .boxed(),
        );
    }
    if state.board.is_none() {
        let options = state.options;
        line = line.push(
            row![
                toggle(
                    ids::FIND_MATCH_CASE_FACE,
                    ids::FIND_MATCH_CASE,
                    options.match_case,
                    FindReplaceMsg::MatchCaseToggled,
                    theme_tokens,
                    i18n,
                ),
                toggle(
                    ids::FIND_WHOLE_WORD_FACE,
                    ids::FIND_WHOLE_WORD,
                    options.whole_word,
                    FindReplaceMsg::WholeWordToggled,
                    theme_tokens,
                    i18n,
                ),
                toggle(
                    ids::FIND_REGEX_FACE,
                    ids::FIND_REGEX,
                    options.regex,
                    FindReplaceMsg::RegexToggled,
                    theme_tokens,
                    i18n,
                ),
            ]
            .spacing(SP.xs)
            .align_y(Alignment::Center)
            .boxed(),
        );
    }
    framed(line.boxed(), invalid, theme_tokens)
}

/// The pill inside the Find field: a replace or scan in progress, an
/// invalid pattern, else the match count. `None` while the field is empty.
pub(super) fn pill_text(state: &FindReplaceState, i18n: &I18n) -> Option<(Localized, PillTone)> {
    if state.replace_job.is_some() {
        return Some((t!(i18n, ids::FIND_REPLACING), PillTone::Neutral));
    }
    if state.scanning {
        return Some((t!(i18n, ids::FIND_SEARCHING), PillTone::Neutral));
    }
    if state.query.trim().is_empty() {
        return None;
    }
    if state.board.is_none() && state.find_query().is_err() {
        return Some((t!(i18n, ids::FIND_INVALID_PATTERN), PillTone::Err));
    }
    let count = state.result_count();
    Some((
        t!(i18n, ids::FIND_MATCHES, "count" => count),
        PillTone::Neutral,
    ))
}

/// One match toggle: a short face ("Aa", "ab", ".*"), the accent-soft fill
/// when on, its name as the tooltip.
fn toggle<'a>(
    face: MessageId,
    name: MessageId,
    on: bool,
    message: FindReplaceMsg,
    theme_tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindReplaceMsg> {
    let r = tokens::resolve(theme_tokens);
    let (fill, ink) = if on {
        (Some(Background::Color(r.accent_soft)), r.accent_soft_ink)
    } else {
        (None, r.ink3)
    };
    let hover = tokens::hover_overlay(theme_tokens);
    let side = TS.body * 1.75;
    let face = text(t!(i18n, face))
        .size(TS.caption)
        .color(ink)
        .font(super::results::mono())
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center);
    let toggle = button(face)
        .padding(0)
        .width(Length::Fixed(side))
        .height(Length::Fixed(side))
        .on_press(message)
        .style(move |_: &Theme, status: button::Status| button::Style {
            background: match status {
                button::Status::Hovered if !on => Some(Background::Color(hover)),
                _ => fill,
            },
            text_color: ink,
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..button::Style::default()
        });
    let tip = container(
        text(t!(i18n, name))
            .size(TS.caption)
            .color(ti(theme_tokens.text)),
    )
    .padding([SP.xs, SP.s])
    .style(crate::styles::context_menu(theme_tokens));
    signex_widgets::tooltip(toggle, tip, iced::widget::tooltip::Position::Bottom)
        .gap(SP.xs)
        .boxed()
}

/// Scope: a Sheet / Project switch with the help text beside it. A board is
/// searched whole (#2403): no switch, its own help text.
fn scope_row<'a>(
    state: &FindReplaceState,
    theme_tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, FindReplaceMsg> {
    let muted = tokens::muted_text(theme_tokens);
    if state.board.is_some() {
        return text(t!(i18n, ids::BOARD_FIND_TARGETS))
            .size(TS.caption)
            .color(muted)
            .boxed();
    }
    let options: [(FindScope, Label<'a>); 2] = [
        (FindScope::Sheet, t!(i18n, ids::FIND_SCOPE_SHEET).into()),
        (FindScope::Project, t!(i18n, ids::FIND_SCOPE_PROJECT).into()),
    ];
    let switch = container(segmented(
        options,
        state.scope,
        segmented_style(theme_tokens, TS.body).fill_width(true),
        FindReplaceMsg::ScopeChanged,
    ))
    .width(Length::Fixed(TS.body * 15.0));
    labelled(
        t!(i18n, ids::FIND_SCOPE),
        row![
            switch,
            text(t!(i18n, ids::FIND_TARGETS))
                .size(TS.caption)
                .color(muted),
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .boxed(),
        theme_tokens,
    )
}
