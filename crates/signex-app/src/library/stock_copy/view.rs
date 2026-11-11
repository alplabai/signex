//! The "Copy to Library" dialog. A pure read of [`StockCopyState`] and the
//! open libraries' names; every colour comes from theme tokens.

use iced::widget::{Space, Widget, column, container, scrollable};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::{raw, text};

use super::{StockCopyMsg, StockCopyState};
use crate::library::messages::LibraryMessage;
use crate::library::modal_chrome::{cancel_btn, footer_strip, header, primary_btn, secondary_btn};
use crate::library::state::LibraryState;
use crate::styles::{DialogIcon, Spacing, TypeScale, dialog_intro};

/// Width of the dialog card in the main window.
pub(crate) const CARD_W: f32 = 520.0;

/// The intro's title and secondary text; the window height follows their
/// wrapped line counts (`app/handlers/modal_sizing.rs`).
pub(crate) fn intro_texts(label: &str, noun: &str) -> (String, String) {
    (
        format!("Copy {label} to a library"),
        format!(
            "{label} is part of the stock library, which is read-only. Pick the library to copy \
             the {noun} into; the copy opens for editing and the stock original is not changed."
        ),
    )
}

/// The note shown when the library used before can no longer take the copy.
pub(crate) fn previous_note(
    i18n: &signex_i18n::I18n,
    previous: &std::path::Path,
) -> signex_i18n::Localized {
    signex_i18n::t!(
        i18n,
        signex_i18n::ids::LIBRARY_STOCK_COPY_PREVIOUS_GONE,
        "path" => previous.display().to_string()
    )
}

pub fn view<'a>(
    library: &'a LibraryState,
    state: &'a StockCopyState,
    tokens: &'a ThemeTokens,
    i18n: &signex_i18n::I18n,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    let muted = theme_ext::text_secondary(tokens);
    let cancel = LibraryMessage::StockCopy(StockCopyMsg::Cancelled);

    let header = header("Copy to Library", cancel.clone(), windowed, tokens);

    let (intro_title, intro_text) = intro_texts(&state.label, state.kind.noun());
    let intro = dialog_intro(DialogIcon::Library, intro_title, Some(intro_text), tokens);

    let mut body = column![intro].spacing(sp.m);
    if let Some(previous) = &state.previous_unavailable {
        body = body.push(
            text(previous_note(i18n, previous))
                .size(ts.caption)
                .color(muted)
                .boxed(),
        );
    }
    if let Some(error) = &state.error {
        body = body.push(
            text(raw(error.as_str()))
                .size(ts.caption)
                .color(theme_ext::error_color(tokens))
                .boxed(),
        );
    }

    let writable = library.writable_libraries();
    let list: Element<'a, LibraryMessage> = if writable.is_empty() {
        text(signex_i18n::t!(
            i18n,
            signex_i18n::ids::LIBRARY_STOCK_COPY_NO_WRITABLE
        ))
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

    // The one primary action follows the quiet Cancel at the right end.
    let footer = footer_strip(
        Vec::new(),
        vec![
            cancel_btn("Cancel", cancel, tokens),
            primary_btn(
                "New Library\u{2026}",
                LibraryMessage::StockCopy(StockCopyMsg::NewLibraryRequested),
                tokens,
            ),
        ],
        tokens,
    );

    // In the main window the card is as tall as its content; in its own
    // window it fills the window, which is sized to the content.
    let mut card = column![header, container(body).padding(sp.l)];
    if windowed {
        card = card.push(Space::new().height(Length::Fill).boxed());
    }
    let height = if windowed {
        Length::Fill
    } else {
        Length::Shrink
    };
    container(
        card.push(footer)
            .width(crate::library::card_len(windowed, CARD_W))
            .height(height),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}
