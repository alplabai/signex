//! Graphite look of the footprint canvas: paper (light) or charcoal (dark)
//! ground, a dot grid like the schematic's, and the canvas layer palette.
//! The look travels as the canvas background colour, so the shared
//! backdrop and layer renderers need no extra field and every other theme
//! keeps the black Altium-style board.

use iced::Color;
use signex_types::theme::ThemeTokens;

use super::super::super::layers::LayerPalette;

/// Spacing the dot grid aims for on screen, in px.
const DOT_STEP_PX: f32 = 16.0;

/// Colours of one Graphite canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GraphiteLook {
    pub bg: Color,
    pub dot: Color,
    pub origin: Color,
    pub dark: bool,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

const LIGHT: GraphiteLook = GraphiteLook {
    bg: rgb(0xFD, 0xFC, 0xFA),
    dot: rgb(0xE2, 0xDF, 0xD8),
    origin: rgb(0xB5, 0x47, 0x8A),
    dark: false,
};

const DARK: GraphiteLook = GraphiteLook {
    bg: rgb(0x14, 0x16, 0x1A),
    dot: rgb(0x2A, 0x2D, 0x33),
    origin: rgb(0xD1, 0x6B, 0xA5),
    dark: true,
};

/// The Graphite canvas look for `tokens`; `None` on every other theme.
pub(crate) fn look_for(tokens: &ThemeTokens) -> Option<GraphiteLook> {
    if !crate::styles::is_graphite(tokens) {
        return None;
    }
    let paper = crate::styles::ti(tokens.paper);
    let luma = 0.299 * paper.r + 0.587 * paper.g + 0.114 * paper.b;
    Some(if luma > 0.5 { LIGHT } else { DARK })
}

/// The look whose ground is exactly `bg`, if any.
pub(crate) fn look_from_bg(bg: Color) -> Option<GraphiteLook> {
    [LIGHT, DARK].into_iter().find(|l| l.bg == bg)
}

impl GraphiteLook {
    /// Layer colours that read on this ground.
    pub(crate) fn palette(self) -> LayerPalette {
        if self.dark {
            LayerPalette::CanvasDark
        } else {
            LayerPalette::CanvasLight
        }
    }
}

/// Dot-grid spacing in px: the grid step, widened by x5 until dots are
/// about [`DOT_STEP_PX`] apart so they never turn into a haze.
pub(crate) fn dot_step_px(grid_step_px: f32) -> Option<f32> {
    if !grid_step_px.is_finite() || grid_step_px <= 0.0 {
        return None;
    }
    let mut step = grid_step_px;
    while step < DOT_STEP_PX * 0.75 {
        step *= 5.0;
    }
    Some(step)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn look_round_trips_through_the_background() {
        assert_eq!(look_from_bg(LIGHT.bg), Some(LIGHT));
        assert_eq!(look_from_bg(DARK.bg), Some(DARK));
        assert_eq!(look_from_bg(Color::BLACK), None);
    }

    #[test]
    fn dot_step_widens_a_tight_grid() {
        assert_eq!(dot_step_px(0.0), None);
        assert_eq!(dot_step_px(20.0), Some(20.0));
        let s = dot_step_px(2.0).unwrap();
        assert!((12.0..=60.0).contains(&s));
    }
}
