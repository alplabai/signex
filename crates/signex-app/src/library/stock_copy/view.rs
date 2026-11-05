//! The "Copy to Library" dialog. A pure read of [`StockCopyState`] and the
//! open libraries' names; every colour comes from theme tokens.

use iced::widget::{Space, Widget, column, container, scrollable, text};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::{StockCopyMsg, StockCopyState};
use crate::library::messages::LibraryMessage;
use crate::library::modal_chrome::{header, primary_btn, secondary_btn};
use crate::library::state::LibraryState;
use crate::styles::{Spacing, TypeScale, dialog_footer_split};

/// Width of the dialog card in the main window.
pub(crate) const CARD_W: f32 = 520.0;

pub fn view<'a>(
    library: &'a LibraryState,
    state: &'a StockCopyState,
    tokens: &'a ThemeTokens,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let cancel = LibraryMessage::StockCopy(StockCopyMsg::Cancelled);

    let header = header("Copy to Library", cancel.clone(), windowed, tokens);

    let intro = text(format!(
        "{} is part of the stock library, which is read-only. Pick the library to copy \
         the {} into; the copy opens for editing and the stock original is not changed.",
        state.label,
        state.kind.noun()
    ))
    .size(ts.body)
    .color(text_c);

    let mut body = column![intro].spacing(sp.m);
    if let Some(previous) = &state.previous_unavailable {
        body = body.push(
            text(format!(
                "The library used before ({}) is no longer open or writable.",
                previous.display()
            ))
            .size(ts.caption)
            .color(muted)
            .boxed(),
        );
    }
    if let Some(error) = &state.error {
        body = body.push(
            text(error.as_str())
                .size(ts.caption)
                .color(theme_ext::error_color(tokens))
                .boxed(),
        );
    }

    let writable = library.writable_libraries();
    let list: Element<'a, LibraryMessage> = if writable.is_empty() {
        text("No writable library is open. Create a new one to copy into.")
            .size(ts.body)
            .color(muted)
            .boxed()
    } else {
        let rows = writable.into_iter().map(|(root, name)| {
            secondary_btn(
                name,
                LibraryMessage::StockCopy(StockCopyMsg::TargetPicked(root.to_path_buf())),
                tokens,
            )
        });
        scrollable(column(rows).spacing(sp.s))
            .height(Length::Fixed(180.0))
            .width(Length::Fill)
            .boxed()
    };
    body = body.push(list);

    let footer = container(dialog_footer_split(
        vec![primary_btn(
            "New Library\u{2026}",
            LibraryMessage::StockCopy(StockCopyMsg::NewLibraryRequested),
            tokens,
        )],
        vec![secondary_btn("Cancel", cancel, tokens)],
    ))
    .style(crate::styles::modal_footer_strip(tokens));

    container(
        column![
            header,
            container(body).padding(sp.l),
            Space::new().height(Length::Fill),
            footer
        ]
        .width(crate::library::card_len(windowed, CARD_W))
        .height(crate::library::card_len(windowed, 380.0)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}
