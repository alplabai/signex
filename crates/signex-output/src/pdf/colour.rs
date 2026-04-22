//! Colour mode transformations for PDF export.
//!
//! Maps RGB colours through a transformation based on `ColourMode`:
//! - `Colour` — pass-through (identity)
//! - `Grayscale` — convert to luminance via `0.299·R + 0.587·G + 0.114·B`
//! - `BlackAndWhite` — strokes → black, fills → white

use super::ColourMode;

/// Transforms an RGB colour according to the export colour mode.
#[derive(Debug, Clone)]
pub struct ColourMap {
    pub mode: ColourMode,
}

impl ColourMap {
    pub fn new(mode: ColourMode) -> Self {
        Self { mode }
    }

    /// Transform an RGB triple (each in range 0.0–1.0) to the target colour mode.
    pub fn map_rgb(&self, r: f32, g: f32, b: f32) -> (f32, f32, f32) {
        match self.mode {
            ColourMode::Colour => (r, g, b),
            ColourMode::Grayscale => {
                let luminance = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
                let lum_f32 = luminance.clamp(0.0, 1.0) as f32;
                (lum_f32, lum_f32, lum_f32)
            }
            ColourMode::BlackAndWhite => {
                // For B&W: strokes are black (0,0,0), fills are white (1,1,1).
                // This is applied per-context in the caller; here we just signal
                // the transformation rule by returning either black or white.
                // The caller decides whether this is a stroke or fill operation.
                if is_approximately_white(r, g, b) {
                    (1.0, 1.0, 1.0)
                } else {
                    (0.0, 0.0, 0.0)
                }
            }
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

    /// Map a fill colour for B&W mode — white for any colour.
    pub fn map_fill_bw(&self, r: f32, g: f32, b: f32) -> (f32, f32, f32) {
        if matches!(self.mode, ColourMode::BlackAndWhite) {
            if is_approximately_white(r, g, b) {
                (1.0, 1.0, 1.0)
            } else {
                (1.0, 1.0, 1.0) // Non-white fills also become white in B&W
            }
        } else {
            self.map_rgb(r, g, b)
        }
    }
}

/// Check if an RGB value is approximately white (all channels > 0.9).
fn is_approximately_white(r: f32, g: f32, b: f32) -> bool {
    r > 0.9 && g > 0.9 && b > 0.9
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_mode_colour_preserves_rgb() {
        let map = ColourMap::new(ColourMode::Colour);
        let (r, g, b) = map.map_rgb(0.5, 0.3, 0.7);
        assert!((r - 0.5).abs() < 1e-5);
        assert!((g - 0.3).abs() < 1e-5);
        assert!((b - 0.7).abs() < 1e-5);
    }

    #[test]
    fn colour_mode_grayscale_maps_red_to_0_299() {
        let map = ColourMap::new(ColourMode::Grayscale);
        let (r, g, b) = map.map_rgb(1.0, 0.0, 0.0); // Pure red
        let expected = 0.299f32;
        assert!((r - expected).abs() < 0.01);
        assert!((g - expected).abs() < 0.01);
        assert!((b - expected).abs() < 0.01);
    }

    #[test]
    fn colour_mode_grayscale_maps_green_to_0_587() {
        let map = ColourMap::new(ColourMode::Grayscale);
        let (r, g, b) = map.map_rgb(0.0, 1.0, 0.0); // Pure green
        let expected = 0.587f32;
        assert!((r - expected).abs() < 0.01);
        assert!((g - expected).abs() < 0.01);
        assert!((b - expected).abs() < 0.01);
    }

    #[test]
    fn colour_mode_grayscale_maps_blue_to_0_114() {
        let map = ColourMap::new(ColourMode::Grayscale);
        let (r, g, b) = map.map_rgb(0.0, 0.0, 1.0); // Pure blue
        let expected = 0.114f32;
        assert!((r - expected).abs() < 0.01);
        assert!((g - expected).abs() < 0.01);
        assert!((b - expected).abs() < 0.01);
    }

    #[test]
    fn colour_mode_bw_pushes_strokes_to_black() {
        let map = ColourMap::new(ColourMode::BlackAndWhite);
        let (r, g, b) = map.map_stroke_bw(1.0, 0.0, 0.0); // Red stroke
        assert!((r - 0.0).abs() < 1e-5);
        assert!((g - 0.0).abs() < 1e-5);
        assert!((b - 0.0).abs() < 1e-5);
    }

    #[test]
    fn colour_mode_bw_fills_with_white() {
        let map = ColourMap::new(ColourMode::BlackAndWhite);
        // Non-white fill becomes white
        let (r, g, b) = map.map_fill_bw(0.5, 0.5, 0.5);
        assert!((r - 1.0).abs() < 1e-5);
        assert!((g - 1.0).abs() < 1e-5);
        assert!((b - 1.0).abs() < 1e-5);
    }

    #[test]
    fn colour_mode_bw_keeps_existing_white_white() {
        let map = ColourMap::new(ColourMode::BlackAndWhite);
        let (r, g, b) = map.map_fill_bw(1.0, 1.0, 1.0); // Existing white
        assert!((r - 1.0).abs() < 1e-5);
        assert!((g - 1.0).abs() < 1e-5);
        assert!((b - 1.0).abs() < 1e-5);
    }

    #[test]
    fn colour_mode_grayscale_white_stays_white() {
        let map = ColourMap::new(ColourMode::Grayscale);
        let (r, g, b) = map.map_rgb(1.0, 1.0, 1.0);
        assert!((r - 1.0).abs() < 1e-5);
        assert!((g - 1.0).abs() < 1e-5);
        assert!((b - 1.0).abs() < 1e-5);
    }

    #[test]
    fn colour_mode_grayscale_black_stays_black() {
        let map = ColourMap::new(ColourMode::Grayscale);
        let (r, g, b) = map.map_rgb(0.0, 0.0, 0.0);
        assert!((r - 0.0).abs() < 1e-5);
        assert!((g - 0.0).abs() < 1e-5);
        assert!((b - 0.0).abs() < 1e-5);
    }
}
