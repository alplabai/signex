//! Graphite look of the footprint canvas: the dark board ground (the same
//! in Graphite Light and Dark), a dot grid like the schematic's, and the canvas layer palette.
//! The look travels as the canvas background colour, so the shared
//! backdrop and layer renderers need no extra field and every other theme
//! keeps the black Altium-style board.

use iced::Color;
use signex_types::theme::ThemeTokens;

use super::super::super::layers::LayerPalette;

/// Spacing the dot grid aims for on screen, in px.
const DOT_STEP_PX: f32 = 16.0;

/// Colours of the Graphite board canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GraphiteLook {
    pub bg: Color,
    pub dot: Color,
    pub origin: Color,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

/// The board look every Graphite theme paints a footprint with: the dark
/// board palette ([`crate::styles::board_canvas_colors`]), light or dark app.
pub(crate) fn board_look() -> GraphiteLook {
    let board = crate::styles::board_canvas_colors();
    GraphiteLook {
        bg: crate::render_config::to_iced(&board.background),
        dot: crate::render_config::to_iced(&board.grid),
        origin: rgb(0xD1, 0x6B, 0xA5),
    }
}

/// The board look for `tokens`: every theme draws the same board.
pub(crate) fn look_for(_tokens: &ThemeTokens) -> GraphiteLook {
    board_look()
}

/// The look whose ground is exactly `bg`, if any.
pub(crate) fn look_from_bg(bg: Color) -> Option<GraphiteLook> {
    Some(board_look()).filter(|l| l.bg == bg)
}

impl GraphiteLook {
    /// Layer colours that read on the dark board ground.
    pub(crate) fn palette(self) -> LayerPalette {
        LayerPalette::CanvasDark
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
        assert_eq!(look_from_bg(board_look().bg), Some(board_look()));
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
