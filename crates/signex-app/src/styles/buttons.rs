//! Token-aware button style factories built on [`super::tokens`].
//!
//! Each returns a closure for `button(..).style(..)`; they replace the
//! inline "transparent / hover tint / pressed tint" `button::Style`
//! closures that hard-coded white-alpha fills.

use iced::widget::button;
use iced::{Background, Border, Color, Theme};
use signex_types::theme::ThemeTokens;

use super::ti;
use super::tokens::{self, Overlay, composite_over};

const RADIUS: f32 = 3.0;

fn fill(c: Color) -> Option<Background> {
    Some(Background::Color(c))
}

/// Borderless button: transparent at rest, hover overlay on hover,
/// pressed overlay on press, `text_secondary` text when disabled.
pub fn subtle_button(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let text = ti(tokens.text);
    let disabled = ti(tokens.text_secondary);
    let hover = tokens::overlay(tokens, Overlay::Hover);
    let pressed = tokens::overlay(tokens, Overlay::Pressed);
    move |_: &Theme, status: button::Status| button::Style {
        background: match status {
            button::Status::Hovered => fill(hover),
            button::Status::Pressed => fill(pressed),
            _ => None,
        },
        text_color: if status == button::Status::Disabled {
            disabled
        } else {
            text
        },
        border: Border {
            radius: RADIUS.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// Bordered secondary button (Cancel, cycle-value, close-x): faint fill
/// at rest, hover / pressed overlays, 1 px `tokens.border`.
pub fn outline_button(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let text = ti(tokens.text);
    let disabled = ti(tokens.text_secondary);
    let border = ti(tokens.border);
    let faint = tokens::overlay(tokens, Overlay::Faint);
    let hover = tokens::overlay(tokens, Overlay::Hover);
    let pressed = tokens::overlay(tokens, Overlay::Pressed);
    move |_: &Theme, status: button::Status| button::Style {
        background: fill(match status {
            button::Status::Hovered => hover,
            button::Status::Pressed => pressed,
            _ => faint,
        }),
        text_color: if status == button::Status::Disabled {
            disabled
        } else {
            text
        },
        border: Border {
            width: 1.0,
            radius: RADIUS.into(),
            color: border,
        },
        ..button::Style::default()
    }
}

/// List-row button: transparent, hover overlay; `selected` rows get the
/// selection tint (kept on hover). Full-width, square, no border.
pub fn hover_row(
    tokens: &ThemeTokens,
    selected: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let text = ti(tokens.text);
    let hover = tokens::overlay(tokens, Overlay::Hover);
    let tint = tokens::selection_tint(tokens);
    move |_: &Theme, status: button::Status| button::Style {
        background: if selected {
            fill(tint)
        } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
            fill(hover)
        } else {
            None
        },
        text_color: text,
        border: Border::default(),
        ..button::Style::default()
    }
}

/// Primary action: [`tokens::accent_fill`] with white text
/// ([`tokens::on_accent`]). Hover and press darken the fill, so white
/// keeps its 4.5:1 contrast in every state.
pub fn accent_button(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let accent = tokens::accent_fill(tokens);
    let on = tokens::on_accent(tokens);
    let hover = tokens::accent_fill_hover(tokens);
    let pressed = tokens::accent_fill_pressed(tokens);
    let disabled = composite_over(Color { a: 0.5, ..accent }, ti(tokens.panel_bg));
    move |_: &Theme, status: button::Status| button::Style {
        background: fill(match status {
            button::Status::Hovered => hover,
            button::Status::Pressed => pressed,
            button::Status::Disabled => disabled,
            button::Status::Active => accent,
        }),
        text_color: on,
        border: Border {
            radius: RADIUS.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}
