//! The passive calculator's look for the catalog sample. This crate cannot
//! reach the app's token resolution, so an optional token the theme leaves
//! unset falls back to a base token here and may drift from the app.

use iced::{Color, Font};
use signex_types::theme::{Color as ThemeColor, ThemeTokens};
use signex_widgets::passive_calculator::{CalculatorLook, CalculatorPalette};
use signex_widgets::segmented::SegmentedStyle;
use signex_widgets::theme_ext::TypeScale;

use crate::theme;

fn ext(token: Option<ThemeColor>, fallback: ThemeColor) -> Color {
    theme::color(token.unwrap_or(fallback))
}

/// The calculator look on `tokens`.
pub(crate) fn look(tokens: &ThemeTokens) -> CalculatorLook<'static> {
    let x = &tokens.extended;
    let palette = CalculatorPalette {
        ink: theme::color(tokens.text),
        ink2: theme::color(tokens.text_secondary),
        ink3: ext(x.ink3, tokens.text_secondary),
        ink4: ext(x.ink4, tokens.text_secondary),
        accent: theme::color(tokens.accent),
        accent_text: ext(x.accent_text, tokens.accent),
        border: theme::color(tokens.border),
        line_soft: ext(x.line_soft, tokens.border),
        line_strong: ext(x.line_strong, tokens.border),
        surface: theme::color(tokens.paper),
        panel: theme::color(tokens.panel_bg),
        field: ext(x.field, tokens.panel_bg),
        sunken: ext(x.sunken, tokens.bg),
        primary: ext(x.primary, tokens.accent),
        primary_ink: ext(x.primary_ink, tokens.paper),
        hover: theme::color(tokens.hover),
        ok: theme::color(tokens.success),
        ok_soft: ext(x.ok_soft, tokens.selection),
        warn: theme::color(tokens.warning),
        warn_soft: ext(x.warn_soft, tokens.selection),
        error: theme::color(tokens.error),
    };
    CalculatorLook {
        palette,
        mono: Font::MONOSPACE,
        segmented: SegmentedStyle::new(
            palette.sunken,
            ext(x.raised, tokens.paper),
            ext(x.shadow, tokens.border),
            palette.ink3,
            palette.ink,
            TypeScale::DEFAULT.body,
        ),
        enable_section: crate::enable_section::style(tokens),
    }
}
