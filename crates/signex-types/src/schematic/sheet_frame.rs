//! Per-sheet frame settings (#1134): whether the sheet draws its border
//! (with zone markers) and its title block, and how many zones the border
//! is divided into. The canvas and the PDF both read them, so a sheet looks
//! the same on screen and on paper.

use serde::{Deserialize, Serialize};

/// Most zones along one edge; letters past Z run AA, AB, ….
pub const MAX_ZONES: u8 = 52;

/// How a sheet's frame is drawn. The default is what every sheet drew before
/// these settings existed: border and title block on, 8 × 6 zones. A sheet at
/// the default stores nothing, so older files re-save byte-identical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SheetFrameSettings {
    /// Border and zone markers.
    #[serde(default = "yes")]
    pub show_border: bool,
    #[serde(default = "yes")]
    pub show_title_block: bool,
    /// Zones numbered 1, 2, 3… along the top and bottom; 0 draws none.
    #[serde(default = "default_horizontal_zones")]
    pub horizontal_zones: u8,
    /// Zones lettered A, B, C… down the sides; 0 draws none.
    #[serde(default = "default_vertical_zones")]
    pub vertical_zones: u8,
}

fn yes() -> bool {
    true
}

fn default_horizontal_zones() -> u8 {
    8
}

fn default_vertical_zones() -> u8 {
    6
}

impl Default for SheetFrameSettings {
    fn default() -> Self {
        Self {
            show_border: true,
            show_title_block: true,
            horizontal_zones: default_horizontal_zones(),
            vertical_zones: default_vertical_zones(),
        }
    }
}

impl SheetFrameSettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// These settings with both zone counts clamped to `0..=MAX_ZONES`.
    pub fn clamped(self) -> Self {
        Self {
            horizontal_zones: self.horizontal_zones.min(MAX_ZONES),
            vertical_zones: self.vertical_zones.min(MAX_ZONES),
            ..self
        }
    }
}
