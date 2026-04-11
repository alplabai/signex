//! Theme bridge layer — converts `signex_types::theme::ThemeTokens` to Iced styles.
//!
//! All colors in the widget crate flow through this module so that
//! no hardcoded color values leak into widget code.

use iced::widget::container;
use iced::{Border, Color};
use signex_types::theme::{Color as SxColor, ThemeTokens};

// ---------------------------------------------------------------------------
// Core color conversion
// ---------------------------------------------------------------------------

/// Convert a signex `Color` (u8 components) to an Iced `Color` (f32 0..1).
pub fn to_color(c: &SxColor) -> Color {
    Color::from_rgba8(c.r, c.g, c.b, c.a as f32 / 255.0)
}

// ---------------------------------------------------------------------------
// Text color helpers
// ---------------------------------------------------------------------------

/// Primary text color from theme tokens.
pub fn text_primary(tokens: &ThemeTokens) -> Color {
    to_color(&tokens.text)
}

/// Secondary / muted text color.
pub fn text_secondary(tokens: &ThemeTokens) -> Color {
    to_color(&tokens.text_secondary)
}

/// Accent color (for highlights, active elements).
pub fn accent(tokens: &ThemeTokens) -> Color {
    to_color(&tokens.accent)
}

/// Error color.
pub fn error_color(tokens: &ThemeTokens) -> Color {
    to_color(&tokens.error)
}

/// Warning color.
pub fn warning_color(tokens: &ThemeTokens) -> Color {
    to_color(&tokens.warning)
}

/// Success / "on" indicator color.
pub fn success_color(tokens: &ThemeTokens) -> Color {
    to_color(&tokens.success)
}

/// Border color.
pub fn border_color(tokens: &ThemeTokens) -> Color {
    to_color(&tokens.border)
}

/// Selection highlight background.
pub fn selection_color(tokens: &ThemeTokens) -> Color {
    to_color(&tokens.selection)
}

/// Hover highlight color.
pub fn hover_color(tokens: &ThemeTokens) -> Color {
    to_color(&tokens.hover)
}

// ---------------------------------------------------------------------------
// Container style factories
// ---------------------------------------------------------------------------

/// Panel background container style (side panels, docks).
pub fn panel_bg(tokens: &ThemeTokens) -> container::Style {
    container::Style {
        background: Some(to_color(&tokens.panel_bg).into()),
        text_color: Some(to_color(&tokens.text)),
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: to_color(&tokens.border),
        },
        ..container::Style::default()
    }
}

/// Toolbar background container style.
pub fn toolbar_bg(tokens: &ThemeTokens) -> container::Style {
    container::Style {
        background: Some(to_color(&tokens.toolbar_bg).into()),
        text_color: Some(to_color(&tokens.text)),
        border: Border {
            width: 0.0,
            radius: 0.0.into(),
            color: to_color(&tokens.border),
        },
        ..container::Style::default()
    }
}

/// Status bar background container style (1px top border).
pub fn status_bar_bg(tokens: &ThemeTokens) -> container::Style {
    container::Style {
        background: Some(to_color(&tokens.statusbar_bg).into()),
        text_color: Some(to_color(&tokens.text)),
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: to_color(&tokens.border),
        },
        ..container::Style::default()
    }
}

/// General application background.
pub fn app_bg(tokens: &ThemeTokens) -> container::Style {
    container::Style {
        background: Some(to_color(&tokens.bg).into()),
        text_color: Some(to_color(&tokens.text)),
        border: Border::default(),
        ..container::Style::default()
    }
}

/// Paper / content area background.
pub fn paper_bg(tokens: &ThemeTokens) -> container::Style {
    container::Style {
        background: Some(to_color(&tokens.paper).into()),
        text_color: Some(to_color(&tokens.text)),
        border: Border::default(),
        ..container::Style::default()
    }
}

/// Accent-colored container (for selected / active items).
pub fn accent_bg(tokens: &ThemeTokens) -> container::Style {
    container::Style {
        background: Some(to_color(&tokens.accent).into()),
        text_color: Some(to_color(&tokens.text)),
        border: Border::default(),
        ..container::Style::default()
    }
}
