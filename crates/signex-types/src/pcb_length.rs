//! Length-matching groups of a board (#2797, mockup row 107): a named set
//! of nets that should route to one target length within a tolerance,
//! with the topology their loads are wired in. The Length Matching tab and
//! its dock panel list them; the board options hold them
//! (`BoardOptions::length_groups`), so every edit is one board undo step
//! and they save with the board. See `docs/PCB_LENGTH_MATCHING.md`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Nanometres per millimetre: band tests compare at the stored resolution,
/// so a length on the band edge is in the band.
const NM_PER_MM: f64 = 1_000_000.0;

/// How a group's loads are wired, as its topology dropdown names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LengthTopology {
    Star,
    #[default]
    Chain,
    FlyBy,
    T,
}

impl LengthTopology {
    pub const ALL: [LengthTopology; 4] = [
        LengthTopology::Star,
        LengthTopology::Chain,
        LengthTopology::FlyBy,
        LengthTopology::T,
    ];

    /// The word the file and the commands use.
    pub const fn token(self) -> &'static str {
        match self {
            LengthTopology::Star => "star",
            LengthTopology::Chain => "chain",
            LengthTopology::FlyBy => "fly_by",
            LengthTopology::T => "t",
        }
    }

    pub fn from_token(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.token() == word)
    }
}

/// One length-matching group. Members are nets, by name, each in at most
/// one group; a name the board does not have is kept and reported as
/// missing, so a renamed net does not drop out silently.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LengthGroup {
    pub uuid: Uuid,
    pub name: String,
    /// The length every member should reach, in mm.
    pub target_mm: f64,
    /// How far a member may be from the target, in mm.
    pub tolerance_mm: f64,
    #[serde(default)]
    pub topology: LengthTopology,
    #[serde(default)]
    pub members: Vec<String>,
}

impl LengthGroup {
    /// The name of the High Speed rule the group stands for in the Rules
    /// manager (mockup row 107).
    pub fn rule_name(&self) -> String {
        format!("MatchedLengths_{}", self.name)
    }

    /// Whether `net` is a member.
    pub fn has_member(&self, net: &str) -> bool {
        self.members.iter().any(|m| m == net)
    }

    /// Whether `length` mm is outside this group's band.
    pub fn is_out_of_band(&self, length: f64) -> bool {
        is_out_of_band(length, self.target_mm, self.tolerance_mm)
    }
}

/// Whether `value` is further than `tolerance` from `target`, all mm,
/// compared in whole nanometres. A value that is not a number is out of
/// band; a negative tolerance counts as zero. The one band test the
/// length report, the tab and the `target_band` widget share.
pub fn is_out_of_band(value: f64, target: f64, tolerance: f64) -> bool {
    let off = ((value - target).abs() * NM_PER_MM).round();
    let allowed = (tolerance.max(0.0) * NM_PER_MM).round();
    off.is_nan() || off > allowed
}

#[cfg(test)]
#[path = "pcb_length_tests.rs"]
mod tests;
