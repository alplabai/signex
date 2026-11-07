//! Graphite look of the Active Bar and its dropdown panel.
//!
//! A theme that sets any extended token is a Graphite theme (the same
//! test as `signex-app`'s `is_graphite`); every other theme gets `None`
//! and keeps the legacy paint. Only paint changes here: the slot widths,
//! padding and spacing stay the legacy constants, so the hit rects the
//! app derives from them are unchanged.

use iced::{Color, Shadow, Vector};
use signex_types::theme::ThemeTokens;

use crate::theme_ext::{border_color, text_primary, to_color};

/// Radius of the bar and of a dropdown panel.
const FLOATING_RADIUS: f32 = 10.0;
/// Radius of a bar button.
const BUTTON_RADIUS: f32 = 7.0;
/// Alpha of the hover wash (the text colour over the bar fill, so it
/// reads on light themes as well as dark).
const HOVER_ALPHA: f32 = 0.08;
/// Alpha of the selected-button tint when the theme has no `accent_soft`.
const SELECTED_FALLBACK_ALPHA: f32 = 0.15;
/// Shadow strength when the theme sets none.
const SHADOW_FALLBACK_ALPHA: f32 = 0.35;

/// Colours and radii of one Graphite floating surface.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Skin {
    pub fill: Color,
    pub border: Color,
    pub shadow: Shadow,
    pub radius: f32,
    pub button_radius: f32,
    pub hover: Color,
    pub selected: Color,
}

impl Skin {
    /// The Graphite skin for `tokens`, or `None` for a legacy theme.
    pub fn of(tokens: &ThemeTokens) -> Option<Self> {
        let e = &tokens.extended;
        if e.is_empty() {
            return None;
        }
        let ink = text_primary(tokens);
        let accent = to_color(&tokens.accent);
        Some(Self {
            fill: e
                .raised
                .as_ref()
                .map_or_else(|| to_color(&tokens.panel_bg), to_color),
            border: border_color(tokens),
            shadow: Shadow {
                color: e.shadow.as_ref().map_or(
                    Color::from_rgba(0.0, 0.0, 0.0, SHADOW_FALLBACK_ALPHA),
                    to_color,
                ),
                offset: Vector::new(0.0, 6.0),
                blur_radius: 20.0,
            },
            radius: FLOATING_RADIUS,
            button_radius: BUTTON_RADIUS,
            hover: Color {
                a: HOVER_ALPHA,
                ..ink
            },
            selected: e.accent_soft.as_ref().map_or(
                Color {
                    a: SELECTED_FALLBACK_ALPHA,
                    ..accent
                },
                to_color,
            ),
        })
    }
}
