//! Resolves the ink tokens of an illustration from the theme.

use iced::Color;
use signex_types::theme::{Color as SxColor, ThemeTokens};

use super::style::{Ink, Paint, Tint};
use crate::theme_ext::to_color;

/// Alpha of the faint ink when the theme sets no fourth text step.
const FAINT_FALLBACK_ALPHA: f32 = 0.6;
/// Alpha of the third ink when the theme sets no third text step.
const TERTIARY_FALLBACK_ALPHA: f32 = 0.8;

/// The resolved colours of every [`Ink`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    secondary: Color,
    tertiary: Color,
    faint: Color,
    rule: Color,
    accent: Color,
    panel: Color,
    error: Color,
}

impl Palette {
    /// The palette of `tokens`. An extended token the theme leaves unset
    /// falls back to a base token.
    pub fn from_tokens(tokens: &ThemeTokens) -> Self {
        let ext = &tokens.extended;
        let secondary = to_color(&tokens.text_secondary);
        let pick = |set: Option<SxColor>, fallback: Color| set.map_or(fallback, |c| to_color(&c));
        Self {
            secondary,
            tertiary: pick(
                ext.ink3,
                with_alpha(secondary, secondary.a * TERTIARY_FALLBACK_ALPHA),
            ),
            faint: pick(
                ext.ink4,
                with_alpha(secondary, secondary.a * FAINT_FALLBACK_ALPHA),
            ),
            rule: pick(ext.line_strong, to_color(&tokens.border)),
            accent: to_color(&tokens.accent),
            panel: to_color(&tokens.panel_bg),
            error: to_color(&tokens.error),
        }
    }

    pub fn ink(&self, ink: Ink) -> Color {
        match ink {
            Ink::Secondary => self.secondary,
            Ink::Tertiary => self.tertiary,
            Ink::Faint => self.faint,
            Ink::Rule => self.rule,
            Ink::Accent => self.accent,
            Ink::Panel => self.panel,
            Ink::Error => self.error,
        }
    }

    pub fn paint(&self, paint: Paint) -> Color {
        match paint {
            Paint::Ink(ink) => self.ink(ink),
            Paint::Document(color) => color,
        }
    }

    /// `paint` at the alpha of `tint`, times the paint's own alpha.
    pub fn tinted(&self, paint: Paint, tint: Tint) -> Color {
        let color = self.paint(paint);
        with_alpha(color, color.a * tint.alpha())
    }
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    Color { a: alpha, ..color }
}
