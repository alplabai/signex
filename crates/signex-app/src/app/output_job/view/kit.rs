//! The small parts every page of the Export Job tab draws with: section
//! heads, quiet lines, tick boxes, text fields and segmented choices.

use iced::widget::{Row, container};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{Localized, MessageId, UserText};
use signex_types::theme::ThemeTokens;
use signex_widgets::{CheckboxLabel as _, Label, button, checkbox, text, text_input};

use super::{Look, SP, TS};
use crate::app::Message;
use crate::panels::form_look::{checkbox_style, text_input_style};
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, ti};

/// An uppercase-styled section head.
pub(super) fn head<'a>(look: Look<'a>, id: MessageId) -> Element<'a, Message> {
    text(look.t(id))
        .size(TS.section)
        .style(crate::styles::section_label(look.tokens))
        .boxed()
}

/// A line of running text.
pub(super) fn line<'a>(look: Look<'a>, words: Localized) -> Element<'a, Message> {
    text(words)
        .size(TS.body)
        .color(ti(look.tokens.text))
        .boxed()
}

/// A muted line under a control.
pub(super) fn quiet<'a>(look: Look<'a>, words: impl Into<Label<'a>>) -> Element<'a, Message> {
    text(words)
        .size(TS.caption)
        .color(resolve(look.tokens).ink3)
        .boxed()
}

/// A name the design gave (a file, an output, a profile).
pub(super) fn user<'a>(look: Look<'a>, words: &str) -> Element<'a, Message> {
    text(UserText::new(words.to_string()))
        .size(TS.body)
        .color(ti(look.tokens.text))
        .boxed()
}

/// A tick box with its words; without `msg` it is drawn disabled.
pub(super) fn tick<'a>(
    on: bool,
    words: impl Into<Label<'a>>,
    msg: Option<Message>,
) -> Element<'a, Message> {
    let mut check = checkbox(on)
        .style(checkbox_style)
        .labelled(words)
        .text_size(TS.body)
        .spacing(SP.s);
    if let Some(msg) = msg {
        check = check.on_toggle(move |_| msg.clone());
    }
    check.boxed()
}

/// A one-line text field.
pub(super) fn field<'a>(
    hint: Localized,
    value: &str,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    text_input(hint, value.to_string())
        .on_input(on_input)
        .size(TS.body)
        .padding(SP.xs)
        .width(Length::Fill)
        .style(text_input_style)
        .boxed()
}

/// A quiet button.
pub(super) fn ghost<'a>(
    tokens: &ThemeTokens,
    words: impl Into<Label<'a>>,
    msg: Option<Message>,
) -> Element<'a, Message> {
    button(text(words).size(TS.dense))
        .padding([SP.xs, SP.s])
        .style(crate::styles::ghost_button(tokens))
        .on_press_maybe(msg)
        .boxed()
}

/// The primary action of a card.
pub(super) fn primary<'a>(
    tokens: &ThemeTokens,
    words: impl Into<Label<'a>>,
    msg: Option<Message>,
) -> Element<'a, Message> {
    button(text(words).size(TS.dense))
        .padding([SP.xs, SP.m])
        .style(crate::styles::primary_button(tokens))
        .on_press_maybe(msg)
        .boxed()
}

/// A segmented choice: one button a choice, the picked one in the primary
/// look.
pub(super) fn choice<'a, T: Copy + PartialEq + 'a>(
    tokens: &ThemeTokens,
    picked: T,
    choices: Vec<(T, Localized)>,
    on_pick: impl Fn(T) -> Message + 'a,
) -> Element<'a, Message> {
    let mut row: Row<Element<'a, Message>> = Row::new().spacing(SP.xs);
    for (value, words) in choices {
        let cell = button(text(words).size(TS.dense)).padding([SP.xs, SP.s]);
        let cell = match value == picked {
            true => cell.style(crate::styles::primary_button(tokens)),
            false => cell.style(crate::styles::ghost_button(tokens)),
        };
        row = row.push(cell.on_press(on_pick(value)).boxed());
    }
    row.align_y(Alignment::Center).boxed()
}

/// A small coloured chip: a badge or a status.
pub(super) fn chip<'a>(
    tokens: &ThemeTokens,
    tone: PillTone,
    words: impl Into<Label<'a>>,
) -> Element<'a, Message> {
    let (_, ink) = crate::styles::pill_colors(tokens, tone);
    container(text(words).size(TS.caption).color(ink))
        .padding([0.0, SP.s])
        .style(crate::styles::pill(tokens, tone))
        .boxed()
}

/// A card around a page's part.
pub(super) fn card<'a>(
    tokens: &ThemeTokens,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    container(content)
        .padding(SP.m)
        .width(Length::Fill)
        .style(crate::styles::card(tokens))
        .boxed()
}
