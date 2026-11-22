//! Shared style / text / button helpers for the Preferences modal.
//!
//! Theme-token-resolved `text::Style` / `button::Style` closures plus
//! the horizontal-separator builder. Reached by the
//! shell (`mod.rs`) and every section submodule via `use super::*` —
//! re-exported from `mod.rs` at `pub(in crate::preferences)`. Pure code
//! motion out of the former single-file `preferences` module.

use iced::{Background, Border, Theme};
use signex_widgets::{button, text};

// ─── Theme-aware text styles ──────────────────────────────────
// The whole modal shell + text now follow the active theme via
// `theme.palette()` tokens — the same palette source the keymap
// widget islands (further down) already use — so every section reads
// correctly on dark AND light palettes. Text colours are resolved at
// render time through these tiny `.style(|theme| …)` helpers rather than
// threading a `&Theme` parameter through every builder signature.

/// Primary body text — the theme's guaranteed-readable text colour on the
/// base background surface.
pub(super) fn text_primary(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.palette().background.base.text),
        selection: None,
    }
}

/// Muted / secondary text — a neutral grey between the text and the page,
/// legible on the base and weak surfaces used across the modal.
pub(super) fn text_muted(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(muted_color(theme)),
        selection: None,
    }
}

/// Warning text (unsaved-changes marker, invalid / conflicting bindings).
/// iced's `ExtendedPalette` carries no warning token, but the base
/// `Palette` exposes a caution hue that is semantic and reads correctly on
/// both dark and light backgrounds — so we resolve it from there.
pub(super) fn text_warning(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.palette().warning.base.color),
        selection: None,
    }
}

/// Destructive button (Discard / Delete / Remove / Stop) — theme danger
/// fill with its guaranteed-contrast text colour.
pub(super) fn danger_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.palette();
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => palette.danger.strong.color,
        _ => palette.danger.base.color,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: palette.danger.base.text,
        border: Border {
            radius: 3.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// `a` moved `t` of the way to `b`, channel by channel.
fn toward(a: iced::Color, b: iced::Color, t: f32) -> iced::Color {
    iced::Color::from_rgb(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
    )
}

/// The neutral hint ink: the text colour toward the page, so it stays grey
/// rather than the palette's warm secondary hue.
pub(super) fn muted_color(theme: &Theme) -> iced::Color {
    let palette = theme.palette();
    toward(
        palette.background.base.text,
        palette.background.base.color,
        MUTED_TOWARD_PAGE,
    )
}

/// How far the hint ink sits from the text toward the page.
const MUTED_TOWARD_PAGE: f32 = 0.45;

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::ThemeId;

    /// Spread between the strongest and weakest channel: 0 is a pure grey.
    fn tint(c: iced::Color) -> f32 {
        c.r.max(c.g).max(c.b) - c.r.min(c.g).min(c.b)
    }

    /// #2262: hint text is a neutral grey, not the warm secondary (brown) of
    /// the iced palette.
    #[test]
    fn hint_ink_is_neutral_in_both_graphite_themes() {
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let theme = crate::app::Signex::id_to_iced_theme(id);
            let palette = theme.palette();
            let hint = muted_color(&theme);
            assert!(tint(hint) < 0.03, "{id:?} hint is tinted: {hint:?}");
            assert_ne!(muted_color(&theme), palette.secondary.base.color);
            assert_eq!(text_muted(&theme).color, Some(muted_color(&theme)));
        }
    }
}
