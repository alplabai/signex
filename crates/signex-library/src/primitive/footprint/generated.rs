//! Generated footprint features (mockup row 124): the switches and values
//! the Properties panel edits, and the strokes the last bake made from them.
//! The strokes stay out of the hand-drawn silk and fab lists, so a re-bake
//! never touches those. See `docs/FOOTPRINT_SKETCH.md`, "Generated features".

use serde::{Deserialize, Serialize};

use super::{FpGraphic, default_true};

/// Silkscreen outline line width a footprint starts with, in mm.
pub const DEFAULT_SILK_LINE_WIDTH_MM: f64 = 0.12;
/// Silkscreen clearance to the pads a footprint starts with, in mm.
pub const DEFAULT_PAD_CLEARANCE_MM: f64 = 0.20;

fn default_silk_line_width() -> f64 {
    DEFAULT_SILK_LINE_WIDTH_MM
}

fn default_pad_clearance() -> f64 {
    DEFAULT_PAD_CLEARANCE_MM
}

fn is_off(value: &bool) -> bool {
    !*value
}

/// What a footprint generates. All off at the defaults, which is not written
/// to the file, so such a footprint keeps its lower format token.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeneratedFeatures {
    #[serde(default, skip_serializing_if = "is_off")]
    pub silk_outline: bool,
    #[serde(default = "default_silk_line_width")]
    pub silk_line_width_mm: f64,
    #[serde(default = "default_pad_clearance")]
    pub pad_clearance_mm: f64,
    #[serde(default = "default_true")]
    pub pin_one_marker: bool,
    #[serde(default, skip_serializing_if = "is_off")]
    pub assembly_outline: bool,
    /// Front silkscreen strokes of the last bake.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub silk: Vec<FpGraphic>,
    /// Front assembly strokes of the last bake.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assembly: Vec<FpGraphic>,
}

impl Default for GeneratedFeatures {
    fn default() -> Self {
        Self {
            silk_outline: false,
            silk_line_width_mm: DEFAULT_SILK_LINE_WIDTH_MM,
            pad_clearance_mm: DEFAULT_PAD_CLEARANCE_MM,
            pin_one_marker: true,
            assembly_outline: false,
            silk: Vec::new(),
            assembly: Vec::new(),
        }
    }
}

impl GeneratedFeatures {
    /// Nothing to store: every switch and value is at its default.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}
