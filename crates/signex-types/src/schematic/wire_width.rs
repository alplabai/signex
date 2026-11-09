//! The four wire widths the Tab form and the wire Properties offer.
//! Values and rationale: docs/DECISIONS.md, "Wire width presets".

use super::{SCHEMATIC_DEFAULT_WIRE_STROKE_MM, SCHEMATIC_RENDER_MIN_STROKE_MM};

/// A named wire width. `Small` stores `0.0` ("default"), so a project or net
/// class width still applies to it; the others store their own width in mm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WireWidthPreset {
    Smallest,
    #[default]
    Small,
    Medium,
    Large,
}

impl WireWidthPreset {
    pub const ALL: [Self; 4] = [Self::Smallest, Self::Small, Self::Medium, Self::Large];

    pub fn label(self) -> &'static str {
        match self {
            Self::Smallest => "Smallest",
            Self::Small => "Small",
            Self::Medium => "Medium",
            Self::Large => "Large",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.label() == label)
    }

    /// The `Wire::stroke_width` this preset stores, in mm.
    pub fn stroke_mm(self) -> f64 {
        match self {
            Self::Smallest => SCHEMATIC_RENDER_MIN_STROKE_MM,
            Self::Small => 0.0,
            Self::Medium => 0.5,
            Self::Large => 1.0,
        }
    }

    /// The preset a stored width belongs to: the nearest by drawn width
    /// (an unset width draws at the built-in default, so it is `Small`).
    pub fn from_stroke_mm(stroke_mm: f64) -> Self {
        let drawn = if super::is_set_width(stroke_mm) {
            stroke_mm
        } else {
            SCHEMATIC_DEFAULT_WIRE_STROKE_MM
        };
        let drawn_of = |p: Self| match p {
            Self::Small => SCHEMATIC_DEFAULT_WIRE_STROKE_MM,
            other => other.stroke_mm(),
        };
        Self::ALL
            .into_iter()
            .min_by(|a, b| {
                (drawn_of(*a) - drawn)
                    .abs()
                    .total_cmp(&(drawn_of(*b) - drawn).abs())
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_choice_stores_unset_and_round_trips() {
        assert_eq!(WireWidthPreset::default().stroke_mm(), 0.0);
        for p in WireWidthPreset::ALL {
            assert_eq!(WireWidthPreset::from_stroke_mm(p.stroke_mm()), p);
            assert_eq!(WireWidthPreset::from_label(p.label()), Some(p));
        }
    }
}
