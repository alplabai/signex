//! Shared style / text / button helpers for the Preferences modal.
//!
//! Theme-token-resolved `text::Style` / `button::Style` closures plus
//! the section-title and horizontal-separator builders. Reached by the
//! shell (`mod.rs`) and every section submodule via `use super::*` —
//! re-exported from `mod.rs` at `pub(in crate::preferences)`. Pure code
//! motion out of the former single-file `preferences` module.

use super::*;
use iced::widget::{Space, button, column, container, text};
use iced::{Background, Border, Element, Length, Theme};

// ─── Theme-aware text styles ──────────────────────────────────
// The whole modal shell + text now follow the active theme via
// `theme.extended_palette()` tokens — the same palette source the keymap
// widget islands (further down) already use — so every section reads
// correctly on dark AND light palettes. Text colours are resolved at
// render time through these tiny `.style(|theme| …)` helpers rather than
// threading a `&Theme` parameter through every builder signature.

/// Primary body text — the theme's guaranteed-readable text colour on the
/// base background surface.
pub(super) fn text_primary(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.extended_palette().background.base.text),
    }
}

/// Muted / secondary text — the theme's secondary accent, still legible on
/// the base and weak background surfaces used across the modal.
pub(super) fn text_muted(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.extended_palette().secondary.base.color),
    }
}

/// Warning text (unsaved-changes marker, invalid / conflicting bindings).
/// iced's `ExtendedPalette` carries no warning token, but the base
/// `Palette` exposes a caution hue that is semantic and reads correctly on
/// both dark and light backgrounds — so we resolve it from there.
pub(super) fn text_warning(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.palette().warning),
    }
}

/// Primary action button (Import / Export / Add / Reset-ERC) — theme accent
/// fill with its guaranteed-contrast text colour.
pub(super) fn primary_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => palette.primary.strong.color,
        _ => palette.primary.base.color,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: palette.primary.base.text,
        border: Border {
            radius: 3.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// Destructive button (Discard / Delete / Remove / Stop) — theme danger
/// fill with its guaranteed-contrast text colour.
pub(super) fn danger_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
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

/// Confirm / commit button (Save) — theme success fill with its
/// guaranteed-contrast text colour.
pub(super) fn success_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => palette.success.strong.color,
        _ => palette.success.base.color,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: palette.success.base.text,
        border: Border {
            radius: 3.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

pub(super) fn section_title<'a>(title: &str) -> Element<'a, PrefMsg> {
    column![
        text(title.to_owned()).size(13).style(text_primary),
        container(Space::new())
            .width(Length::Fill)
            .height(1)
            .style(move |theme: &Theme| container::Style {
                background: Some(Background::Color(theme.extended_palette().background.strong.color)),
                ..container::Style::default()
            }),
    ]
    .spacing(6)
    .into()
}

pub(super) fn h_sep<'a>() -> Element<'a, PrefMsg> {
    container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(move |theme: &Theme| container::Style {
            background: Some(Background::Color(theme.extended_palette().background.strong.color)),
            ..container::Style::default()
        })
        .into()
}

/// Neutral secondary button (Import / Export / Edit / Cancel / Clear) —
/// theme-derived so it reads on both light and dark palettes.
pub(super) fn secondary_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => palette.background.strong.color,
        _ => palette.background.weak.color,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: palette.background.base.text,
        border: Border {
            width: 1.0,
            color: palette.background.strong.color,
            radius: 3.0.into(),
        },
        ..button::Style::default()
    }
}
