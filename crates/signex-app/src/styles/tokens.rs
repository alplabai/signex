//! Theme-derived colours for chrome surfaces.
//!
//! These replace hard-coded literals that only read on dark themes
//! (`Color::from_rgba(1, 1, 1, 0.04)` and friends). Every colour is
//! derived from existing [`ThemeTokens`] — no new token fields, no
//! theme-file change — so custom themes get the right behaviour for free.
//!
//! The key trick is that translucent "state" fills are tinted with
//! `tokens.text`: text is light on dark themes and dark on light themes,
//! so the same fill lightens a dark panel and darkens a light one.

use iced::Color;
use signex_types::theme::ThemeTokens;

use super::ti;

/// Strength of a translucent state fill, as a step of alpha over
/// `tokens.text`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    /// Resting control fill (replaces white at 0.03 / 0.04).
    Faint,
    /// Pointer-over fill (replaces white at 0.06).
    Hover,
    /// Pressed / active fill (replaces white at 0.10).
    Pressed,
}

impl Overlay {
    /// Alpha applied to `tokens.text` for this step.
    pub const fn alpha(self) -> f32 {
        match self {
            Overlay::Faint => 0.04,
            Overlay::Hover => 0.06,
            Overlay::Pressed => 0.10,
        }
    }
}

/// Translucent state fill for `step`: `tokens.text` at the step's alpha.
/// Draw it over a surface; it lightens dark themes and darkens light ones.
pub fn overlay(tokens: &ThemeTokens, step: Overlay) -> Color {
    Color {
        a: step.alpha(),
        ..ti(tokens.text)
    }
}

/// Pointer-over fill — shorthand for `overlay(tokens, Overlay::Hover)`.
pub fn hover_overlay(tokens: &ThemeTokens) -> Color {
    overlay(tokens, Overlay::Hover)
}

/// Translucent selected-row / selection tint: the accent at 40 % alpha
/// (replaces `rgba(0.4, 0.6, 1.0, 0.4)`).
pub fn selection_tint(tokens: &ThemeTokens) -> Color {
    Color {
        a: 0.4,
        ..ti(tokens.accent)
    }
}

/// Opaque accent colour (replaces `rgb(0.00, 0.47, 0.84)`).
pub fn accent(tokens: &ThemeTokens) -> Color {
    ti(tokens.accent)
}

/// Text / icon colour that reads on top of [`accent`]; see [`readable_on`].
pub fn on_accent(tokens: &ThemeTokens) -> Color {
    readable_on(ti(tokens.accent))
}

/// Minimum WCAG contrast for normal-size text.
pub const MIN_TEXT_CONTRAST: f32 = 4.5;

/// Text colour for an opaque fill `bg`. White is preferred whenever it
/// reaches [`MIN_TEXT_CONTRAST`] (the long-standing look on dark and
/// mid-tone fills); only when white fails does the higher-contrast of
/// black / white win.
pub fn readable_on(bg: Color) -> Color {
    let white = contrast_ratio(Color::WHITE, bg);
    if white >= MIN_TEXT_CONTRAST || white >= contrast_ratio(Color::BLACK, bg) {
        Color::WHITE
    } else {
        Color::BLACK
    }
}

/// Destructive / error colour (replaces `rgb(0.85, 0.3, 0.3)`).
pub fn danger(tokens: &ThemeTokens) -> Color {
    ti(tokens.error)
}

/// Modal backdrop. Black at `alpha` on every theme — a scrim dims the
/// content behind it, which is theme-neutral by design.
pub fn scrim(alpha: f32) -> Color {
    Color::from_rgba(0.0, 0.0, 0.0, alpha)
}

/// Secondary / hint text.
pub fn muted_text(tokens: &ThemeTokens) -> Color {
    ti(tokens.text_secondary)
}

/// Text of a disabled control: secondary text at half strength.
pub fn disabled_text(tokens: &ThemeTokens) -> Color {
    let c = ti(tokens.text_secondary);
    Color { a: c.a * 0.5, ..c }
}

/// Opaque text-input background: the faint overlay composited over the
/// panel, so inputs sit slightly off the surface on any theme.
pub fn input_bg(tokens: &ThemeTokens) -> Color {
    composite_over(overlay(tokens, Overlay::Faint), ti(tokens.panel_bg))
}

/// Text-input border.
pub fn input_border(tokens: &ThemeTokens) -> Color {
    ti(tokens.border)
}

/// Alpha-composite `fg` over an opaque `bg` (result is opaque).
pub fn composite_over(fg: Color, bg: Color) -> Color {
    let mix = |f: f32, b: f32| f * fg.a + b * (1.0 - fg.a);
    Color::from_rgb(mix(fg.r, bg.r), mix(fg.g, bg.g), mix(fg.b, bg.b))
}

/// WCAG relative luminance (linearised sRGB).
pub fn relative_luminance(c: Color) -> f32 {
    let lin = |v: f32| {
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

/// WCAG contrast ratio between two opaque colours (1.0 ..= 21.0).
pub fn contrast_ratio(a: Color, b: Color) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}
