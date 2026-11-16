//! Board panelisation (#2410): how a board is arrayed into a manufacturing
//! panel with spacing, rails, fiducials, tooling holes and mouse bites or
//! V-score lines. Document data on the board's options
//! ([`crate::pcb_options::BoardOptions::panel`]), saved in the `.snxpcb`
//! `[board_options.panel]` table and changed only through the board engine's
//! `SetBoardPanel` command. The copies are never stored: `pcb_geom::panel`
//! derives the panel from this and the board outline. See
//! `docs/PANELISATION.md`.

mod field;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

pub use field::{PanelField, PanelFieldKind};

/// Most copies along one axis.
pub const MAX_PANEL_COPIES: u32 = 50;

/// Most tabs on one edge of a copy.
pub const MAX_TABS_PER_EDGE: u32 = 20;

/// How the copies come apart after assembly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanelSeparation {
    /// Routed gaps with tabs across them, perforated by a row of small
    /// non-plated holes.
    #[default]
    MouseBites,
    /// Straight score lines across the whole panel along the copies' edges.
    VScore,
}

impl PanelSeparation {
    pub const ALL: [Self; 2] = [Self::MouseBites, Self::VScore];

    /// The word a file and a command operand use.
    pub fn token(self) -> &'static str {
        match self {
            Self::MouseBites => "mouse_bites",
            Self::VScore => "v_score",
        }
    }

    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.token() == token)
    }
}

/// The waste strips round the copies, mm; 0 leaves a side without one.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PanelRails {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

/// The tabs that hold a copy in a routed panel, mm.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MouseBites {
    /// Diameter of each perforation hole.
    pub hole_diameter: f64,
    /// Centre to centre distance of the holes along a tab.
    pub pitch: f64,
    /// Length of a tab along the copy's edge.
    pub tab_width: f64,
    pub tabs_per_edge: u32,
}

/// A board's panel. Lengths in mm; the y axis grows down, as in the board
/// model, so `rails.top` is the rail above the first row.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoardPanel {
    pub columns: u32,
    pub rows: u32,
    /// Gap between neighbouring copies, and between a copy and a rail.
    pub spacing_x: f64,
    pub spacing_y: f64,
    pub rails: PanelRails,
    /// Copper dot of each fiducial on the top and bottom rails; 0 places
    /// none.
    pub fiducial_diameter: f64,
    /// Non-plated tooling hole in each corner of the top and bottom rails;
    /// 0 places none.
    pub tooling_hole_diameter: f64,
    pub separation: PanelSeparation,
    /// Kept while V-scores are chosen, so switching back loses nothing.
    pub bites: MouseBites,
}

impl Default for BoardPanel {
    /// The panel `panelize_board` starts from: two by two, 2 mm gaps, 5 mm
    /// rails top and bottom with fiducials and 2 mm tooling holes, one 3 mm
    /// tab per edge.
    fn default() -> Self {
        Self {
            columns: 2,
            rows: 2,
            spacing_x: 2.0,
            spacing_y: 2.0,
            rails: PanelRails {
                top: 5.0,
                bottom: 5.0,
                left: 0.0,
                right: 0.0,
            },
            fiducial_diameter: 1.0,
            tooling_hole_diameter: 2.0,
            separation: PanelSeparation::MouseBites,
            bites: MouseBites {
                hole_diameter: 0.5,
                pitch: 0.8,
                tab_width: 3.0,
                tabs_per_edge: 1,
            },
        }
    }
}

impl BoardPanel {
    /// Number of copies.
    pub fn copies(&self) -> u32 {
        self.columns * self.rows
    }

    /// Every rule a panel keeps whatever board it arrays; the board-shaped
    /// ones (tabs that fit an edge) are checked by the layout.
    pub fn check(&self) -> Result<(), PanelError> {
        count("columns", self.columns, 1, MAX_PANEL_COPIES)?;
        count("rows", self.rows, 1, MAX_PANEL_COPIES)?;
        let b = &self.bites;
        count("tabs_per_edge", b.tabs_per_edge, 1, MAX_TABS_PER_EDGE)?;
        let lengths = [
            ("spacing_x", self.spacing_x),
            ("spacing_y", self.spacing_y),
            ("rail_top", self.rails.top),
            ("rail_bottom", self.rails.bottom),
            ("rail_left", self.rails.left),
            ("rail_right", self.rails.right),
            ("fiducial_diameter", self.fiducial_diameter),
            ("tooling_hole_diameter", self.tooling_hole_diameter),
            ("bite_hole_diameter", b.hole_diameter),
            ("bite_pitch", b.pitch),
            ("tab_width", b.tab_width),
        ];
        for (field, mm) in lengths {
            if !mm.is_finite() {
                return Err(PanelError::NotFinite { field });
            }
            if mm < 0.0 {
                return Err(PanelError::Negative { field });
            }
        }
        if self.separation != PanelSeparation::MouseBites {
            return Ok(());
        }
        for (field, mm) in [
            ("bite_hole_diameter", b.hole_diameter),
            ("bite_pitch", b.pitch),
            ("tab_width", b.tab_width),
        ] {
            if mm <= 0.0 {
                return Err(PanelError::NotPositive { field });
            }
        }
        if b.pitch <= b.hole_diameter {
            return Err(PanelError::PitchNotAboveHole);
        }
        if self.spacing_x <= 0.0 || self.spacing_y <= 0.0 {
            return Err(PanelError::BitesNeedSpacing);
        }
        Ok(())
    }
}

fn count(field: &'static str, value: u32, min: u32, max: u32) -> Result<(), PanelError> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(PanelError::Count { field, min, max })
    }
}

/// Why a panel cannot be set or laid out.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PanelError {
    #[error("{field} must be a whole number from {min} to {max}")]
    Count {
        field: &'static str,
        min: u32,
        max: u32,
    },
    #[error("{field} is not a number")]
    NotFinite { field: &'static str },
    #[error("{field} cannot be negative")]
    Negative { field: &'static str },
    #[error("{field} must be more than 0")]
    NotPositive { field: &'static str },
    #[error("the bite pitch must be more than the bite hole diameter, or the holes run together")]
    PitchNotAboveHole,
    #[error("mouse bites need a gap: set spacing_x and spacing_y above 0, or use V-scores")]
    BitesNeedSpacing,
    #[error("`{value}` is not a value for {field}")]
    BadValue { field: &'static str, value: String },
    #[error("the board has no outline to array")]
    NoOutline,
    #[error("{tabs} tabs of {tab_width_mm} mm do not fit on an edge of {edge_mm} mm")]
    TabsDoNotFit {
        tabs: u32,
        tab_width_mm: f64,
        edge_mm: f64,
    },
    #[error("the panel reaches further than 1 km from the origin")]
    TooLarge,
}
