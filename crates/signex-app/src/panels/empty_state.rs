//! Shared Graphite empty-state card: a title, a line of explanation and an
//! optional muted caption, on the card surface, with optional action buttons
//! under the text. Used by panels with nothing to list yet (Selection Filter,
//! Navigator, Net Inspector).

use iced::widget::{Column, Row, column, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::Localized;
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use crate::styles::{self, Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// A button of [`empty_state_card_with_actions`]: its label, what a press
/// emits, and whether it is the card's primary action.
#[derive(Debug, Clone)]
pub struct EmptyStateAction<M> {
    pub label: Localized,
    pub on_press: M,
    pub primary: bool,
}

/// The card, 12 px padded, filling the panel width.
pub fn empty_state_card<'a, M: 'a>(
    tokens: &ThemeTokens,
    title: Localized,
    body: Localized,
    caption: Option<Localized>,
) -> Element<'a, M> {
    card(tokens, lines(tokens, title, body, caption))
}

/// [`empty_state_card`] with `actions` as a row of buttons under the text:
/// the primary one in the Graphite primary look, the others secondary. No
/// actions draws the plain card.
pub fn empty_state_card_with_actions<'a, M: Clone + 'a>(
    tokens: &ThemeTokens,
    title: Localized,
    body: Localized,
    caption: Option<Localized>,
    actions: Vec<EmptyStateAction<M>>,
) -> Element<'a, M> {
    let mut content = lines(tokens, title, body, caption);
    if !actions.is_empty() {
        let mut buttons: Row<Element<'a, M>> = Row::new().spacing(SP.s);
        for action in actions {
            buttons = buttons.push(action_button(tokens, action));
        }
        content = content.push(
            container(buttons)
                .padding(iced::Padding {
                    top: SP.xs,
                    ..iced::Padding::ZERO
                })
                .boxed(),
        );
    }
    card(tokens, content)
}

fn lines<'a, M: 'a>(
    tokens: &ThemeTokens,
    title: Localized,
    body: Localized,
    caption: Option<Localized>,
) -> Column<Element<'a, M>> {
    let mut lines = column![
        text(title).size(TS.label).color(ti(tokens.text)),
        text(body).size(TS.dense).color(ti(tokens.text_secondary)),
    ]
    .spacing(6);
    if let Some(caption) = caption {
        lines = lines.push(
            text(caption)
                .size(TS.caption)
                .color(resolve(tokens).ink4)
                .boxed(),
        );
    }
    lines
}

fn card<'a, M: 'a>(tokens: &ThemeTokens, content: Column<Element<'a, M>>) -> Element<'a, M> {
    container(content)
        .padding(12)
        .width(Length::Fill)
        .style(styles::card(tokens))
        .boxed()
}

fn action_button<'a, M: Clone + 'a>(
    tokens: &ThemeTokens,
    action: EmptyStateAction<M>,
) -> Element<'a, M> {
    let pressed = button(text(action.label).size(TS.body))
        .padding([SP.xs, SP.m])
        .on_press(action.on_press);
    if action.primary {
        pressed.style(styles::primary_button(tokens)).boxed()
    } else {
        pressed.style(styles::outline_button(tokens)).boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_i18n::{I18n, ids, t};
    use signex_types::theme::{ThemeId, theme_tokens};

    /// The texts of a card with `actions` buttons, each labelled Open
    /// Project…, that read as that label.
    fn buttons_of(actions: usize) -> usize {
        let i18n = I18n::en_us();
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let actions: Vec<EmptyStateAction<usize>> = (0..actions)
            .map(|n| EmptyStateAction {
                label: t!(i18n, ids::PANELS_NAVIGATOR_OPEN_PROJECT),
                on_press: n,
                primary: n == 0,
            })
            .collect();
        let card = empty_state_card_with_actions(
            &tokens,
            t!(i18n, ids::PANELS_NAVIGATOR_NO_PROJECT),
            t!(i18n, ids::PANELS_NAVIGATOR_NO_PROJECT_BODY),
            None,
            actions,
        );
        let want = i18n.message_plain(ids::PANELS_NAVIGATOR_OPEN_PROJECT, &[]);
        crate::layout_probe::lay_out_texts(card, iced::Size::new(300.0, 400.0))
            .into_iter()
            .filter(|t| t.text == want)
            .count()
    }

    #[test]
    fn the_card_builds_with_zero_one_and_two_actions() {
        assert_eq!(buttons_of(0), 0);
        assert_eq!(buttons_of(1), 1);
        assert_eq!(buttons_of(2), 2);
    }
}
