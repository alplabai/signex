//! Graphite button looks: `primary` fill, copper accent fill, ghost. All use
//! the button radius; the dialog kit picks them for Graphite themes only.

use iced::widget::button;
use iced::{Background, Border, Color, Theme};
use signex_types::theme::ThemeTokens;

use super::radii::Radii;
use super::tokens::{self, Overlay, composite_over, resolve};
use super::{accent_button, ti};

fn fill(c: Color) -> Option<Background> {
    Some(Background::Color(c))
}

fn rounded() -> Border {
    Border {
        radius: Radii::GRAPHITE.button.into(),
        ..Border::default()
    }
}

/// The theme's `primary` / `primary_ink` button. Hover and press darken the
/// fill toward black.
pub fn primary_button(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let r = resolve(tokens);
    let (base, ink) = (r.primary, r.primary_ink);
    let black = |a: f32| composite_over(Color { a, ..Color::BLACK }, base);
    let (hover, pressed) = (black(0.08), black(0.16));
    let disabled = composite_over(Color { a: 0.5, ..base }, ti(tokens.panel_bg));
    move |_: &Theme, status: button::Status| button::Style {
        background: fill(match status {
            button::Status::Hovered => hover,
            button::Status::Pressed => pressed,
            button::Status::Disabled => disabled,
            button::Status::Active => base,
        }),
        text_color: ink,
        border: rounded(),
        ..button::Style::default()
    }
}

/// [`accent_button`] with the Graphite button radius.
pub fn accent_button_rounded(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let base = accent_button(tokens);
    move |theme: &Theme, status: button::Status| {
        let mut style = base(theme, status);
        style.border.radius = Radii::GRAPHITE.button.into();
        style
    }
}

/// Quiet action ("Decide later"): no fill or border at rest, a hover overlay.
pub fn ghost_button(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let ink = ti(tokens.text);
    let disabled = resolve(tokens).ink4;
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
            ink
        },
        border: rounded(),
        ..button::Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ghost button (the dialogs' neutral close) fills only on hover and
    /// press, in every built-in theme, and hover differs from press.
    #[test]
    fn ghost_button_fills_only_on_hover_and_press() {
        for &id in signex_types::theme::ThemeId::BUILTINS {
            let tokens = signex_types::theme::theme_tokens(id);
            let style = ghost_button(&tokens);
            let theme = Theme::Dark;
            let rest = style(&theme, button::Status::Active);
            let hover = style(&theme, button::Status::Hovered);
            let pressed = style(&theme, button::Status::Pressed);
            assert!(rest.background.is_none(), "{id:?}");
            assert!(hover.background.is_some(), "{id:?}");
            assert!(pressed.background.is_some(), "{id:?}");
            assert_ne!(hover.background, pressed.background, "{id:?}");
        }
    }
}
