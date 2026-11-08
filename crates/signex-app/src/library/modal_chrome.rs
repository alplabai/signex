//! Shared, theme-token-driven chrome for the Library modals: the header
//! close button and the footer secondary / primary action buttons.
//!
//! Every Library modal (picker, primitive picker, recovery, updates,
//! close-library prompt, edit-row) used to carry its own copy of these
//! with hard-coded `from_rgba` colours and pixel sizes. They now share
//! one definition built on [`crate::styles`] tokens, so a theme change
//! reaches all of them.

use iced::widget::{Space, button, container, row, text};
use iced::{Element, Length, Widget};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::LibraryMessage;
use crate::styles::{DialogButtonKind, Spacing, TypeScale, dialog_button, dialog_footer_split};

/// Fixed widths of the Library modals that size themselves (the others
/// take the picker / dialog default). One home so a resize is one edit.
pub(crate) const CLOSE_PROMPT_W: f32 = 520.0;
pub(crate) const CREATE_OPTIONS_W: f32 = 480.0;
pub(crate) const DOCUMENT_OPTIONS_W: f32 = 460.0;
pub(crate) const EDIT_ROW_W: f32 = 720.0;
pub(crate) const EDIT_ROW_H: f32 = 640.0;

/// Header close ("x") button emitting `message` when pressed.
pub(crate) fn close_x<'a>(
    message: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    let text_c = theme_ext::text_secondary(tokens);
    button(
        container(text("\u{00D7}".to_string()).size(ts.heading).color(text_c)).padding([0.0, sp.s]),
    )
    .on_press(message)
    .style(crate::styles::outline_button(tokens))
    .boxed()
}

/// What sits at the right end of a dialog header.
enum HeaderEnd {
    Nothing,
    Close(LibraryMessage),
    Note(String),
}

fn header_strip<'a>(
    title: &str,
    end: HeaderEnd,
    windowed: bool,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    // In its own OS window the window's title bar carries the title and the
    // close: the slot stays empty (docs/DETACHED_MODALS.md).
    let ts = TypeScale::default();
    if windowed {
        // A note (the library or file the dialog acts on) is not in the
        // window title, so it stays, as a muted line above the body.
        return match end {
            HeaderEnd::Note(note) => container(
                text(note)
                    .size(ts.body)
                    .color(theme_ext::text_secondary(tokens)),
            )
            .padding([6, 14])
            .boxed(),
            HeaderEnd::Nothing | HeaderEnd::Close(_) => Space::new().height(0).boxed(),
        };
    }
    if crate::styles::is_graphite(tokens) {
        let (subtitle, close) = match end {
            HeaderEnd::Nothing => (None, None),
            HeaderEnd::Close(message) => (None, Some(message)),
            HeaderEnd::Note(note) => (Some(note), None),
        };
        return crate::app::view::dialogs::graphite_header_closing(
            title.to_string(),
            subtitle,
            None,
            close,
            crate::app::view::dialogs::DIALOG_HEADER_H,
            tokens,
        );
    }
    let mut line = row![
        text(title.to_string())
            .size(ts.heading)
            .color(theme_ext::text_primary(tokens)),
        Space::new().width(Length::Fill),
    ]
    .align_y(iced::Alignment::Center);
    match end {
        HeaderEnd::Nothing => {}
        HeaderEnd::Close(message) => line = line.push(close_x(message, tokens)),
        HeaderEnd::Note(note) => {
            line = line.push(
                text(note)
                    .size(ts.body)
                    .color(theme_ext::text_secondary(tokens))
                    .boxed(),
            );
        }
    }
    container(line)
        .padding([10, 14])
        .style(crate::styles::modal_header_strip(tokens))
        .boxed()
}

/// Header of a Library dialog: title and a close button, or the empty slot
/// when `windowed` (the OS window's title bar carries both). Every Library
/// dialog builds its header here.
pub(crate) fn header<'a>(
    title: &str,
    on_close: LibraryMessage,
    windowed: bool,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    header_strip(title, HeaderEnd::Close(on_close), windowed, tokens)
}

/// Header with a muted note at the right end instead of a close button.
pub(crate) fn header_with_note<'a>(
    title: &str,
    note: String,
    windowed: bool,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    header_strip(title, HeaderEnd::Note(note), windowed, tokens)
}

/// Header with the title alone (no close button).
pub(crate) fn header_title_only<'a>(
    title: &str,
    windowed: bool,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    header_strip(title, HeaderEnd::Nothing, windowed, tokens)
}

/// Bordered secondary footer action (Cancel, Skip, ...).
pub(crate) fn secondary_btn<'a>(
    label: &str,
    message: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    dialog_button(label, DialogButtonKind::Secondary, Some(message), tokens)
}

/// The dismissing footer action (Cancel): Ghost on Graphite, bordered
/// elsewhere. It goes first in the right-aligned button row.
pub(crate) fn cancel_btn<'a>(
    label: &str,
    message: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    dialog_button(label, DialogButtonKind::Cancel, Some(message), tokens)
}

/// Accent-filled primary footer action.
pub(crate) fn primary_btn<'a>(
    label: &str,
    message: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    primary_btn_if(label, message, true, tokens)
}

/// Primary footer action that is inert (and drawn disabled) while
/// `enabled` is false.
pub(crate) fn primary_btn_if<'a>(
    label: &str,
    message: LibraryMessage,
    enabled: bool,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    dialog_button(
        label,
        DialogButtonKind::Primary,
        enabled.then_some(message),
        tokens,
    )
}

/// Footer of a Library dialog on the flat footer strip: `extras` at the
/// left end, `buttons` right-aligned with the standard gaps.
pub(crate) fn footer_strip<'a>(
    extras: Vec<Element<'a, LibraryMessage>>,
    buttons: Vec<Element<'a, LibraryMessage>>,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    container(dialog_footer_split(extras, buttons))
        .style(crate::styles::modal_footer_strip(tokens))
        .boxed()
}

/// Destructive footer action (Delete, Discard): error-coloured outline.
pub(crate) fn danger_btn<'a>(
    label: &str,
    message: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    dialog_button(label, DialogButtonKind::Danger, Some(message), tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::ThemeId;

    /// The chrome builds from tokens alone for every built-in theme; no
    /// hard-coded colour path remains to diverge per theme.
    #[test]
    fn chrome_builds_for_every_theme() {
        for id in ThemeId::BUILTINS {
            let tokens = signex_types::theme::theme_tokens(*id);
            let _ = close_x(LibraryMessage::ClosePicker, &tokens);
            for windowed in [false, true] {
                let _ = header("Title", LibraryMessage::ClosePicker, windowed, &tokens);
                let _ = header_with_note("Title", "note".into(), windowed, &tokens);
                let _ = header_title_only("Title", windowed, &tokens);
            }
            let _ = secondary_btn("Skip", LibraryMessage::ClosePicker, &tokens);
            let _ = cancel_btn("Cancel", LibraryMessage::ClosePicker, &tokens);
            let _ = primary_btn("OK", LibraryMessage::ClosePicker, &tokens);
            let _ = primary_btn_if("OK", LibraryMessage::ClosePicker, false, &tokens);
            let _ = danger_btn("Delete", LibraryMessage::ClosePicker, &tokens);
        }
    }
}
