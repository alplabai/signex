//! The board Tools edits (#2408, #2409): via stitching, teardrops, gloss,
//! board outline and keepout from a selection, and arrange in a box.
//!
//! Pure: each reads a board, does its geometry in `pcb_geom`, and returns
//! the commands the caller runs as one undo step with a report of what they
//! do (JSON-serialisable, for an agent). Nothing here writes. A copper tool
//! takes the clearance rule as `clearance(a, b)`, the gap nets `a` and `b`
//! need, so its copper keeps what the design rule check asks for.

mod arrange;
mod gloss;
mod keepout;
mod outline;
mod stitch;
mod teardrop;

use serde::{Deserialize, Serialize};

pub use arrange::arrange_in_box;
pub use gloss::gloss_tracks;
pub use keepout::keepout_from_selection;
pub use outline::outline_from_tracks;
pub use stitch::{DEFAULT_STITCH_PITCH_MM, StitchArea, default_stitch_pitch_mm, stitch_vias};
pub use teardrop::{add_teardrops, remove_teardrops};

use super::command::BoardCommand;
use super::placement::PlacementRefusal;

/// Why a board tool changed nothing.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ToolRefusal {
    #[error("nothing is selected that this can act on")]
    NothingSelected,
    #[error("{0}")]
    Path(String),
    #[error("the board sketch owns the board outline: change it in Board Sketch")]
    OutlineOwnedBySketch,
    #[error("the area has no net: name the net to stitch with")]
    NoNet,
    #[error("the net {0} is not on the board")]
    UnknownNet(String),
    #[error("the grid step {pitch} mm is smaller than the via ({diameter} mm)")]
    PitchTooSmall { pitch: f64, diameter: f64 },
    #[error("the stitching area has no area")]
    NoArea,
    #[error("no via fits in the area clear of other nets and keepouts")]
    NothingFits,
    #[error("there are no teardrops to remove")]
    NoTeardrops,
    #[error("no track of the chosen nets enters a pad or via of its net")]
    NoTeardropPlace,
    #[error(
        "no corner of the selected routes can be cut without breaking the clearance or leaving copper of its net loose"
    )]
    NothingToGloss,
    #[error("{0} is already a keepout")]
    AlreadyKeepout(String),
    #[error("{0} of the selected lines carry a net: only lines on no net become a keepout")]
    LinesOnNet(usize),
    #[error("the clearance rule cannot be read: {0}")]
    Rules(String),
    #[error("{0} is on no layer a keepout can keep clear")]
    NoKeepoutLayer(String),
    #[error("the components do not fit in the box")]
    DoesNotFit,
    #[error("{0}")]
    Placement(#[from] PlacementRefusal),
}

/// The commands a tool built, to run as one undo step, and what they do.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolEdit {
    pub commands: Vec<BoardCommand>,
    pub report: ToolReport,
}

/// What a tool edit does, as an agent reads it back and as the GUI reads
/// it back from the operand handler's answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolReport {
    /// Items the edit adds (vias, teardrops, a keepout), by uuid.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub added: Vec<uuid::Uuid>,
    /// Items it removes (teardrops replaced, tracks merged or turned into a
    /// keepout).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<uuid::Uuid>,
    /// Items it changes in place (a glossed track, a moved footprint, a zone
    /// turned into a keepout).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub changed: Vec<uuid::Uuid>,
    /// Places the tool passed over because the result would break the
    /// clearance or a keepout (teardrops), or locked tracks it left as
    /// they are (gloss).
    #[serde(skip_serializing_if = "is_zero")]
    pub skipped: usize,
    /// The outline points written, for an outline edit.
    #[serde(skip_serializing_if = "is_zero")]
    pub outline_points: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

#[cfg(test)]
mod tests;
