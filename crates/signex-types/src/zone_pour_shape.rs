//! A polygon pour's corners (#2578) and fill mode (#2579): how the fill
//! shapes the corners of the pour's outline, and whether it is solid copper
//! or a hatch. Both are applied by `pcb_geom::zone_fills`; the rules are in
//! `docs/PCB_TOOLS.md`, "Polygon Pour". Every key is written only when it
//! differs from the default, so a board with sharp, solid pours saves as
//! before.

use serde::{Deserialize, Serialize};

use crate::pcb::Zone;

/// How the fill shapes the corners of a pour's outline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PourCorners {
    /// The corners as drawn.
    #[default]
    Sharp,
    /// An arc of `Zone::corner_size` radius, tangent to both edges.
    Rounded,
    /// A straight cut `Zone::corner_size` back from the corner along each
    /// edge.
    Chamfer,
}

impl PourCorners {
    /// The tokens a command reads, in the panel's order.
    pub const ALL: [PourCorners; 3] = [Self::Sharp, Self::Rounded, Self::Chamfer];

    pub fn token(self) -> &'static str {
        match self {
            Self::Sharp => "sharp",
            Self::Rounded => "rounded",
            Self::Chamfer => "chamfer",
        }
    }

    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.token() == token)
    }

    pub(crate) fn is_sharp(&self) -> bool {
        *self == Self::Sharp
    }
}

/// Whether a pour's fill is solid copper or a hatch. There is no None: a
/// pour that keeps only its outline is shelved (`Zone::shelved`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PourFillMode {
    #[default]
    Solid,
    /// A square grid of copper lines inside a copper border, both
    /// `Zone::hatch_width` wide, with `Zone::hatch_gap` between lines.
    Hatched,
}

impl PourFillMode {
    pub const ALL: [PourFillMode; 2] = [Self::Solid, Self::Hatched];

    pub fn token(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Hatched => "hatched",
        }
    }

    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.token() == token)
    }

    pub(crate) fn is_solid(&self) -> bool {
        *self == Self::Solid
    }
}

/// The hatch line width a hatched pour uses while its own is 0, in mm.
pub const HATCH_WIDTH_DEFAULT_MM: f64 = 0.25;
/// The gap between hatch lines a hatched pour uses while its own is 0, in
/// mm.
pub const HATCH_GAP_DEFAULT_MM: f64 = 0.5;

/// The hatch a hatched pour's fill draws, in mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PourHatch {
    /// The width of each line and of the border.
    pub width_mm: f64,
    /// The open space between two lines.
    pub gap_mm: f64,
}

/// `value` when it is a positive number.
fn positive(value: f64) -> Option<f64> {
    (value.is_finite() && value > 0.0).then_some(value)
}

impl Zone {
    /// The corner shape and size, in mm, a pour's fill gives its outline;
    /// `None` for sharp corners, a size that is not a positive number, or a
    /// zone that is no pour.
    pub fn pour_corner_shape(&self) -> Option<(PourCorners, f64)> {
        if !self.is_pour() || self.corners == PourCorners::Sharp {
            return None;
        }
        positive(self.corner_size).map(|size| (self.corners, size))
    }

    /// The hatch of a hatched pour, a width or gap of 0 (or one that is not
    /// a positive number) read as its default; `None` for a solid pour or a
    /// zone that is no pour.
    pub fn pour_hatch(&self) -> Option<PourHatch> {
        (self.is_pour() && self.fill_mode == PourFillMode::Hatched).then(|| self.hatch_settings())
    }

    /// The hatch this zone's width and gap stand for, whatever its fill
    /// mode: each stored value, or its default for a 0 (or a value that is
    /// not a positive number). The pour panel shows these.
    pub fn hatch_settings(&self) -> PourHatch {
        PourHatch {
            width_mm: positive(self.hatch_width).unwrap_or(HATCH_WIDTH_DEFAULT_MM),
            gap_mm: positive(self.hatch_gap).unwrap_or(HATCH_GAP_DEFAULT_MM),
        }
    }

    /// Whether this pour fills with shaped corners or a hatch: what a build
    /// that does not know the keys would fill differently
    /// (`format::snxpcb_token`). Rounded or Chamfer with no size fills as
    /// Sharp, so it does not count.
    pub fn has_pour_shape(&self) -> bool {
        self.pour_corner_shape().is_some() || self.pour_hatch().is_some()
    }
}

#[cfg(test)]
#[path = "zone_pour_shape_tests.rs"]
mod tests;
