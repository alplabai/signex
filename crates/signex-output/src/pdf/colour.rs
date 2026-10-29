//! Colour mode transformations for PDF export.
//!
//! Maps RGB colours through a transformation based on `ColourMode`:
//! - `Colour` — pass-through (identity)
//! - `Grayscale` — convert to luminance via `0.299·R + 0.587·G + 0.114·B`
//! - `BlackAndWhite` — strokes and text → black; fills → white when they
//!   are close to the theme's paper tone, black otherwise

use super::ColourMode;
use crate::svg::{luma, map_black_and_white};

/// Transforms an RGB colour according to the export colour mode.
#[derive(Debug, Clone)]
pub struct ColourMap {
    pub mode: ColourMode,
    /// The theme's paper colour, which Black & White fills map against.
    pub paper: (f32, f32, f32),
}

impl ColourMap {
    pub fn new(mode: ColourMode, paper: (f32, f32, f32)) -> Self {
        Self { mode, paper }
    }

    /// Transform an RGB triple (each in range 0.0–1.0) to the target colour mode.
    pub fn map_rgb(&self, r: f32, g: f32, b: f32) -> (f32, f32, f32) {
        match self.mode {
            ColourMode::Colour => (r, g, b),
            ColourMode::Grayscale => {
                let y = luma((r, g, b)).clamp(0.0, 1.0);
                (y, y, y)
            }
            ColourMode::BlackAndWhite => map_black_and_white((r, g, b), self.paper),
        }
    }

    /// Map a stroke colour for B&W mode — always returns black.
    pub fn map_stroke_bw(&self, _r: f32, _g: f32, _b: f32) -> (f32, f32, f32) {
        if matches!(self.mode, ColourMode::BlackAndWhite) {
            (0.0, 0.0, 0.0)
        } else {
            self.map_rgb(_r, _g, _b)
        }
    }

    /// Map a fill colour for B&W mode. A fill close to the theme's paper
    /// (the page itself, a symbol's body tint) becomes white and anything
    /// else (junction dots, arrowheads, filled shapes drawn in ink) becomes
    /// black, so a B&W page stays white with solid marks on every theme
    /// (#1342). The preview uses the same mapping. Colour modes map as usual.
    pub fn map_fill_bw(&self, r: f32, g: f32, b: f32) -> (f32, f32, f32) {
        self.map_rgb(r, g, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: (f32, f32, f32) = (1.0, 1.0, 1.0);

    #[test]
    fn colour_mode_colour_preserves_rgb() {
        let map = ColourMap::new(ColourMode::Colour, WHITE);
        let (r, g, b) = map.map_rgb(0.5, 0.3, 0.7);
        assert!((r - 0.5).abs() < 1e-5);
        assert!((g - 0.3).abs() < 1e-5);
        assert!((b - 0.7).abs() < 1e-5);
    }

    #[test]
    fn colour_mode_grayscale_maps_red_to_0_299() {
        let map = ColourMap::new(ColourMode::Grayscale, WHITE);
        let (r, g, b) = map.map_rgb(1.0, 0.0, 0.0); // Pure red
        let expected = 0.299f32;
        assert!((r - expected).abs() < 0.01);
        assert!((g - expected).abs() < 0.01);
        assert!((b - expected).abs() < 0.01);
    }

    #[test]
    fn colour_mode_grayscale_maps_green_to_0_587() {
        let map = ColourMap::new(ColourMode::Grayscale, WHITE);
        let (r, g, b) = map.map_rgb(0.0, 1.0, 0.0); // Pure green
        let expected = 0.587f32;
        assert!((r - expected).abs() < 0.01);
        assert!((g - expected).abs() < 0.01);
        assert!((b - expected).abs() < 0.01);
    }

    #[test]
    fn colour_mode_grayscale_maps_blue_to_0_114() {
        let map = ColourMap::new(ColourMode::Grayscale, WHITE);
        let (r, g, b) = map.map_rgb(0.0, 0.0, 1.0); // Pure blue
        let expected = 0.114f32;
        assert!((r - expected).abs() < 0.01);
        assert!((g - expected).abs() < 0.01);
        assert!((b - expected).abs() < 0.01);
    }

    #[test]
    fn colour_mode_bw_pushes_strokes_to_black() {
        let map = ColourMap::new(ColourMode::BlackAndWhite, WHITE);
        let (r, g, b) = map.map_stroke_bw(1.0, 0.0, 0.0); // Red stroke
        assert!((r - 0.0).abs() < 1e-5);
        assert!((g - 0.0).abs() < 1e-5);
        assert!((b - 0.0).abs() < 1e-5);
    }

    #[test]
    fn colour_mode_bw_fills_a_light_tint_with_white() {
        // A body tint close to white paper prints as paper.
        let map = ColourMap::new(ColourMode::BlackAndWhite, WHITE);
        assert_eq!(map.map_fill_bw(0.95, 0.95, 0.85), (1.0, 1.0, 1.0));
    }

    #[test]
    fn colour_mode_bw_on_dark_paper_whitens_the_paper_and_blackens_light_ink() {
        // #1342: a dark theme's paper must print white and its bright marks
        // (junction dots drawn in a light colour) must print solid black.
        let nord_paper = (
            0x3B as f32 / 255.0,
            0x42 as f32 / 255.0,
            0x52 as f32 / 255.0,
        );
        let map = ColourMap::new(ColourMode::BlackAndWhite, nord_paper);
        assert_eq!(
            map.map_fill_bw(nord_paper.0, nord_paper.1, nord_paper.2),
            (1.0, 1.0, 1.0)
        );
        assert_eq!(map.map_fill_bw(0.9, 0.9, 0.9), (0.0, 0.0, 0.0));
    }

    #[test]
    fn colour_mode_bw_fills_dark_ink_with_black() {
        // A junction dot or arrowhead in dark ink stays a solid mark.
        let map = ColourMap::new(ColourMode::BlackAndWhite, WHITE);
        assert_eq!(map.map_fill_bw(0.1, 0.1, 0.2), (0.0, 0.0, 0.0));
    }

    #[test]
    fn colour_mode_bw_keeps_existing_white_white() {
        let map = ColourMap::new(ColourMode::BlackAndWhite, WHITE);
        let (r, g, b) = map.map_fill_bw(1.0, 1.0, 1.0); // Existing white
        assert!((r - 1.0).abs() < 1e-5);
        assert!((g - 1.0).abs() < 1e-5);
        assert!((b - 1.0).abs() < 1e-5);
    }

    #[test]
    fn colour_mode_grayscale_white_stays_white() {
        let map = ColourMap::new(ColourMode::Grayscale, WHITE);
        let (r, g, b) = map.map_rgb(1.0, 1.0, 1.0);
        assert!((r - 1.0).abs() < 1e-5);
        assert!((g - 1.0).abs() < 1e-5);
        assert!((b - 1.0).abs() < 1e-5);
    }

    #[test]
    fn colour_mode_grayscale_black_stays_black() {
        let map = ColourMap::new(ColourMode::Grayscale, WHITE);
        let (r, g, b) = map.map_rgb(0.0, 0.0, 0.0);
        assert!((r - 0.0).abs() < 1e-5);
        assert!((g - 0.0).abs() < 1e-5);
        assert!((b - 0.0).abs() < 1e-5);
    }
}
