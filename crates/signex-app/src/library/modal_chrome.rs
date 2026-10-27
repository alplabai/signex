//! Shared, theme-token-driven chrome for the Library modals: the header
//! close button and the footer secondary / primary action buttons.
//!
//! Every Library modal (picker, primitive picker, recovery, updates,
//! close-library prompt, edit-row) used to carry its own copy of these
//! with hard-coded `from_rgba` colours and pixel sizes. They now share
//! one definition built on [`crate::styles`] tokens, so a theme change
//! reaches all of them.

use iced::Element;
use iced::widget::{button, container, text};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::LibraryMessage;
use crate::styles::{Spacing, TypeScale};

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
    .into()
}

/// Bordered secondary footer action (Cancel, Skip, ...).
pub(crate) fn secondary_btn<'a>(
    label: &str,
    message: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    let text_color = theme_ext::text_primary(tokens);
    button(container(text(label.to_string()).size(ts.body).color(text_color)).padding([sp.s, sp.l]))
        .on_press(message)
        .style(crate::styles::outline_button(tokens))
        .into()
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
    let ts = TypeScale::default();
    let sp = Spacing::default();
    let fg = crate::styles::tokens::on_accent(tokens);
    let btn =
        button(container(text(label.to_string()).size(ts.body).color(fg)).padding([sp.s, sp.l]));
    let btn = if enabled { btn.on_press(message) } else { btn };
    btn.style(crate::styles::accent_button(tokens)).into()
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
            let _ = secondary_btn("Cancel", LibraryMessage::ClosePicker, &tokens);
            let _ = primary_btn("OK", LibraryMessage::ClosePicker, &tokens);
            let _ = primary_btn_if("OK", LibraryMessage::ClosePicker, false, &tokens);
        }
    }
}
