//! Shared dialog kit for the Component Preview tabs.
//!
//! One place for section headers, cards, tables, buttons and empty
//! states so the tabs stop hand-rolling them. Everything is generic over
//! the caller's `Message`, takes `&ThemeTokens`, and draws only with
//! theme tokens ([`Overlay`] steps over `tokens.text`), so it reads on
//! light and dark themes alike.
// The tabs adopt the kit in follow-up units of #1957; until then only
// the Alternate symbols buttons use it.
#![allow(dead_code)]

use iced::widget::{Space, button, column, container, row, text};
use iced::{Background, Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use crate::styles::tokens::{self, Overlay};
use crate::styles::{self, HAIRLINE, Spacing, TypeScale};

/// Type scale shared by every Component Preview tab.
pub(crate) const TS: TypeScale = TypeScale::DEFAULT;
/// Spacing scale shared by every Component Preview tab.
pub(crate) const SP: Spacing = Spacing::DEFAULT;

const RADIUS: f32 = 3.0;
const CARD_RADIUS: f32 = 4.0;

// ─── Style functions (pure; unit-tested) ─────────────────────

/// Card surface: faint overlay over the panel, 1 px token border.
pub(crate) fn card_style(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let fill = tokens::overlay(tokens, Overlay::Faint);
    let border = theme_ext::border_color(tokens);
    move |_| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            width: HAIRLINE,
            radius: CARD_RADIUS.into(),
            color: border,
        },
        ..container::Style::default()
    }
}

/// 1 px rule in the border colour (header underline).
pub(crate) fn rule_style(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let border = theme_ext::border_color(tokens);
    move |_| container::Style {
        background: Some(Background::Color(border)),
        ..container::Style::default()
    }
}

/// Body-row fill: odd rows take the faint overlay (zebra); a hovered or
/// pressed row takes the hover / pressed overlay on any stripe.
pub(crate) fn row_background(
    tokens: &ThemeTokens,
    index: usize,
    status: button::Status,
) -> Option<Background> {
    let color = match status {
        button::Status::Hovered => tokens::overlay(tokens, Overlay::Hover),
        button::Status::Pressed => tokens::overlay(tokens, Overlay::Pressed),
        _ if index % 2 == 1 => tokens::overlay(tokens, Overlay::Faint),
        _ => return None,
    };
    Some(Background::Color(color))
}

fn row_style(
    tokens: &ThemeTokens,
    index: usize,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let tokens = *tokens;
    let text_c = theme_ext::text_primary(&tokens);
    move |_: &Theme, status| button::Style {
        background: row_background(&tokens, index, status),
        text_color: text_c,
        border: Border::default(),
        ..button::Style::default()
    }
}

/// Destructive button: outline shape with `tokens.error` text and border.
pub(crate) fn danger_button_style(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let danger = tokens::danger(tokens);
    let disabled = tokens::disabled_text(tokens);
    let hover = tokens::overlay(tokens, Overlay::Hover);
    let pressed = tokens::overlay(tokens, Overlay::Pressed);
    move |_: &Theme, status| {
        let ink = if status == button::Status::Disabled {
            disabled
        } else {
            danger
        };
        button::Style {
            background: match status {
                button::Status::Hovered => Some(Background::Color(hover)),
                button::Status::Pressed => Some(Background::Color(pressed)),
                _ => None,
            },
            text_color: ink,
            border: Border {
                width: HAIRLINE,
                radius: RADIUS.into(),
                color: ink,
            },
            ..button::Style::default()
        }
    }
}

// ─── Layout pieces ───────────────────────────────────────────

fn rule<'a, M: 'a>(tokens: &ThemeTokens) -> Element<'a, M> {
    container(Space::new())
        .width(Length::Fill)
        .height(HAIRLINE)
        .style(rule_style(tokens))
        .into()
}

/// Section title with an optional trailing element (a button, a count)
/// pushed to the right edge, over a header rule.
pub(crate) fn section_header<'a, M: 'a>(
    title: &'a str,
    trailing: Option<Element<'a, M>>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let mut line = row![
        text(title)
            .size(TS.title)
            .color(theme_ext::text_primary(tokens)),
        Space::new().width(Length::Fill),
    ]
    .align_y(iced::Alignment::Center);
    if let Some(t) = trailing {
        line = line.push(t);
    }
    column![container(line).padding([SP.s, 0.0]), rule(tokens)]
        .width(Length::Fill)
        .into()
}

/// Bordered card around `content`.
pub(crate) fn card<'a, M: 'a>(
    content: impl Into<Element<'a, M>>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    container(content)
        .padding(SP.l)
        .width(Length::Fill)
        .style(card_style(tokens))
        .into()
}

/// Table header: muted column titles over a header rule. Each column is
/// `(title, width)`; pass the same widths to [`table_row`] cells.
pub(crate) fn table_header<'a, M: 'a>(
    columns: &[(&'a str, Length)],
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let muted = theme_ext::text_secondary(tokens);
    let mut line = row![].spacing(SP.m).align_y(iced::Alignment::Center);
    for (title, width) in columns {
        line = line.push(text(*title).size(TS.caption).color(muted).width(*width));
    }
    column![container(line).padding([SP.s, SP.m]), rule(tokens)]
        .width(Length::Fill)
        .into()
}

/// Table body row. `index` drives zebra striping; with `on_press` the row
/// also shows hover / pressed fills (a row without a handler is inert).
pub(crate) fn table_row<'a, M: Clone + 'a>(
    cells: Vec<(Element<'a, M>, Length)>,
    index: usize,
    on_press: Option<M>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let mut line = row![].spacing(SP.m).align_y(iced::Alignment::Center);
    for (cell, width) in cells {
        line = line.push(container(cell).width(width));
    }
    let body = container(line).padding([SP.s, SP.m]).width(Length::Fill);
    if on_press.is_none() {
        let fill = row_background(tokens, index, button::Status::Active);
        return body
            .style(move |_: &Theme| container::Style {
                background: fill,
                ..container::Style::default()
            })
            .into();
    }
    button(body)
        .padding(0)
        .width(Length::Fill)
        .on_press_maybe(on_press)
        .style(row_style(tokens, index))
        .into()
}

// ─── Buttons ─────────────────────────────────────────────────

fn labelled<'a, M: Clone + 'a>(
    label: &'a str,
    on_press: Option<M>,
    style: impl Fn(&Theme, button::Status) -> button::Style + 'a,
) -> Element<'a, M> {
    button(text(label).size(TS.body))
        .padding([SP.xs, SP.m])
        .on_press_maybe(on_press)
        .style(style)
        .into()
}

/// Primary action (accent fill). `None` disables it.
pub(crate) fn primary_button<'a, M: Clone + 'a>(
    label: &'a str,
    on_press: Option<M>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    labelled(label, on_press, styles::accent_button(tokens))
}

/// Secondary action (outline). `None` disables it.
pub(crate) fn secondary_button<'a, M: Clone + 'a>(
    label: &'a str,
    on_press: Option<M>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    labelled(label, on_press, styles::outline_button(tokens))
}

/// Destructive action (error-coloured outline). `None` disables it.
pub(crate) fn danger_button<'a, M: Clone + 'a>(
    label: &'a str,
    on_press: Option<M>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    labelled(label, on_press, danger_button_style(tokens))
}

/// Icon-only button with a tooltip, from `signex-widgets`.
pub(crate) fn icon_button<'a, M: Clone + 'a>(
    icon: &str,
    tooltip: &str,
    on_press: M,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    signex_widgets::icon_button::icon_button(icon, tooltip, on_press, false, tokens)
}

// ─── Empty state ─────────────────────────────────────────────

/// Centred message with an optional call-to-action under it.
pub(crate) fn empty_state<'a, M: 'a>(
    message: &'a str,
    action: Option<Element<'a, M>>,
    tokens: &ThemeTokens,
) -> Element<'a, M> {
    let mut col = column![
        text(message)
            .size(TS.label)
            .color(theme_ext::text_secondary(tokens))
    ]
    .spacing(SP.m)
    .align_x(iced::Alignment::Center);
    if let Some(a) = action {
        col = col.push(a);
    }
    container(col).padding(SP.xl).center_x(Length::Fill).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::{ThemeId, theme_tokens};

    fn bg(b: Option<Background>) -> Option<iced::Color> {
        match b {
            Some(Background::Color(c)) => Some(c),
            _ => None,
        }
    }

    #[test]
    fn zebra_rows_differ_on_every_builtin() {
        for &id in ThemeId::BUILTINS {
            let t = theme_tokens(id);
            let even = bg(row_background(&t, 0, button::Status::Active));
            let odd = bg(row_background(&t, 1, button::Status::Active));
            assert_ne!(even, odd, "{id:?}");
            assert_eq!(odd, bg(row_background(&t, 3, button::Status::Active)));
        }
    }

    #[test]
    fn hover_differs_from_active_on_both_stripes() {
        for &id in ThemeId::BUILTINS {
            let t = theme_tokens(id);
            for index in [0, 1] {
                let active = bg(row_background(&t, index, button::Status::Active));
                let hover = bg(row_background(&t, index, button::Status::Hovered));
                let pressed = bg(row_background(&t, index, button::Status::Pressed));
                assert!(hover.is_some() && hover != active, "{id:?} {index}");
                assert_ne!(hover, pressed, "{id:?} {index}");
            }
        }
    }

    #[test]
    fn card_and_rule_use_the_token_border() {
        let t = theme_tokens(ThemeId::BUILTINS[0]);
        let border = theme_ext::border_color(&t);
        let card = card_style(&t)(&Theme::Dark);
        assert_eq!(card.border.color, border);
        assert_eq!(bg(rule_style(&t)(&Theme::Dark).background), Some(border));
    }

    #[test]
    fn danger_button_is_error_coloured_and_dims_when_disabled() {
        let t = theme_tokens(ThemeId::BUILTINS[0]);
        let style = danger_button_style(&t);
        let active = style(&Theme::Dark, button::Status::Active);
        let disabled = style(&Theme::Dark, button::Status::Disabled);
        assert_eq!(active.text_color, tokens::danger(&t));
        assert_ne!(active.text_color, disabled.text_color);
        assert_ne!(
            bg(style(&Theme::Dark, button::Status::Hovered).background),
            bg(active.background)
        );
    }
}
