//! Graphite look of the Active Bar and its dropdown panel.
//!
//! Paint only: the slot widths, padding and
//! spacing come from [`super::Metrics`], which the app's hit-test reads too.

use iced::{Color, Shadow, Vector};
use signex_types::theme::ThemeTokens;

use crate::theme_ext::{border_color, text_primary, text_secondary, to_color};

/// Radius of the bar and of a dropdown panel.
const FLOATING_RADIUS: f32 = 10.0;
/// Radius of a bar button.
const BUTTON_RADIUS: f32 = 7.0;
/// Alpha of the hover wash (the text colour over the bar fill, so it
/// reads on light themes as well as dark).
const HOVER_ALPHA: f32 = 0.08;
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
    /// Fill of the armed tool: the ink token, as in the mock.
    pub selected: Color,
    /// Tint of every icon (the ink token).
    pub ink: Color,
    /// Icon tint on the armed tool's fill (the bar surface colour).
    pub on_selected: Color,
    /// Separator line inside a dropdown card (the soft line token).
    pub line_soft: Color,
    /// Tint of a dropdown row icon (the third ink token).
    pub icon_ink: Color,
}

impl Skin {
    /// The Graphite skin for `tokens`.
    pub fn of(tokens: &ThemeTokens) -> Self {
        let e = &tokens.extended;
        let ink = text_primary(tokens);
        Self {
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
            selected: ink,
            ink,
            on_selected: e
                .raised
                .as_ref()
                .map_or_else(|| to_color(&tokens.panel_bg), to_color),
            line_soft: e
                .line_soft
                .as_ref()
                .map_or_else(|| border_color(tokens), to_color),
            icon_ink: e
                .ink3
                .as_ref()
                .map_or_else(|| text_secondary(tokens), to_color),
        }
    }
}
