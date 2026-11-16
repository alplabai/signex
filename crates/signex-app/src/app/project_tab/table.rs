//! The pieces the Project tab's table sections share (#1689-#1693): the
//! section frame (title, help), a heading row, a table row, a muted note and
//! the Move up / Move down pair. Pure view helpers in the Graphite look.

use iced::widget::{Column, Row, Space, column, container, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use crate::app::Message;
use crate::styles::{Spacing, TypeScale, tokens as ink};

pub(super) const TS: TypeScale = TypeScale::DEFAULT;
pub(super) const SP: Spacing = Spacing::DEFAULT;

/// Widest a section grows, in `body`-size characters.
const SECTION_MAX_EMS: f32 = 64.0;

/// Width of a [`labelled`] row's caption column, in `body`-size characters.
const LABEL_EMS: f32 = 12.0;

/// A section: its title, a help line, then `body`, padded like the others.
pub(super) fn section<'a>(
    title: MessageId,
    help: MessageId,
    body: Vec<Element<'a, Message>>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut col: Column<'a, Message> = column![
        text(t!(i18n, title))
            .size(TS.heading)
            .color(crate::styles::ti(tokens.text)),
        note(t!(i18n, help), tokens),
    ]
    .spacing(SP.l)
    .width(Length::Fill.max(TS.body * SECTION_MAX_EMS));
    for part in body {
        col = col.push(part);
    }
    container(col).padding([SP.xl, SP.xl]).boxed()
}

/// A sub-heading inside a section.
pub(super) fn heading<'a>(words: Localized, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(words)
        .size(TS.title)
        .color(crate::styles::ti(tokens.text))
        .boxed()
}

/// Secondary text: help, a state, a reason.
pub(super) fn note<'a>(words: Localized, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(words)
        .size(TS.caption)
        .color(ink::muted_text(tokens))
        .boxed()
}

/// A refusal reason, in the danger colour.
pub(super) fn problem<'a>(words: Localized, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(words)
        .size(TS.caption)
        .color(ink::danger(tokens))
        .boxed()
}

/// The heading row of a table: one caption per column with its width, then
/// an uncaptioned column of `actions` width (the row buttons), if any.
pub(super) fn heading_row<'a>(
    columns: &[(MessageId, Length)],
    actions: Option<Length>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut head: Row<'a, Message> = Row::new().spacing(SP.s).padding([SP.xs, SP.m]);
    for (caption, width) in columns {
        head = head.push(
            text(t!(i18n, *caption))
                .size(TS.caption)
                .color(ink::muted_text(tokens))
                .width(*width),
        );
    }
    if let Some(width) = actions {
        head = head.push(Space::new().width(width));
    }
    head.boxed()
}

/// One table row: each cell in its column's width.
pub(super) fn table_row<'a>(cells: Vec<(Element<'a, Message>, Length)>) -> Element<'a, Message> {
    let mut line: Row<'a, Message> = Row::new()
        .spacing(SP.s)
        .padding([SP.xs, SP.m])
        .align_y(Alignment::Center);
    for (cell, width) in cells {
        line = line.push(container(cell).width(width));
    }
    line.boxed()
}

/// A plain cell of user text (a name, a file, a path).
pub(super) fn cell<'a>(value: impl Into<String>, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(signex_i18n::UserText::new(value.into()))
        .size(TS.body)
        .color(crate::styles::ti(tokens.text))
        .boxed()
}

/// A cell of catalog words.
pub(super) fn word_cell<'a>(words: Localized, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(words)
        .size(TS.body)
        .color(crate::styles::ti(tokens.text))
        .boxed()
}

/// A labelled row outside a table: a caption column, then `field`.
pub(super) fn labelled<'a>(
    caption: Localized,
    field: Element<'a, Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    row![
        container(text(caption).size(TS.body).color(ink::muted_text(tokens)))
            .width(Length::Fixed(TS.body * LABEL_EMS)),
        field,
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .boxed()
}

/// A small outlined action button; `None` greys it.
pub(super) fn action<'a>(
    words: Localized,
    on_press: Option<Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    button(text(words).size(TS.body))
        .padding([SP.xs, SP.m])
        .style(crate::styles::outline_button(tokens))
        .on_press_maybe(on_press)
        .boxed()
}

/// Move up / Move down for the row at `index` of `count`; the button at the
/// list's end is greyed.
pub(super) fn move_buttons<'a>(
    index: usize,
    count: usize,
    moved: impl Fn(signex_types::project::ListMove) -> Message,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    use signex_types::project::ListMove;
    let up = (index > 0).then(|| moved(ListMove::Up));
    let down = (index + 1 < count).then(|| moved(ListMove::Down));
    row![
        action(t!(i18n, ids::PROJECT_MOVE_UP), up, tokens),
        action(t!(i18n, ids::PROJECT_MOVE_DOWN), down, tokens),
    ]
    .spacing(SP.xs)
    .boxed()
}
