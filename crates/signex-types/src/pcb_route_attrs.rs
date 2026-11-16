//! The lock and mask settings of a board track or via (#2378): what the
//! Track and Via panels edit beyond geometry and net. Saved in optional
//! `[tracks]` / `[vias]` columns (`format::pcb_row_columns`), written only
//! when a setting differs from the default, so a board that never sets one
//! re-saves byte-identical.

use serde::{Deserialize, Serialize};

/// How far a mask opening reaches past the copper of one item.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaskExpansion {
    /// The board's design rule decides.
    #[default]
    Rule,
    /// This item's own expansion in mm; may be negative (a smaller opening).
    Manual { mm: f64 },
}

impl MaskExpansion {
    /// The manual expansion in mm, `None` for the rule.
    pub fn manual_mm(self) -> Option<f64> {
        match self {
            Self::Rule => None,
            Self::Manual { mm } => Some(mm),
        }
    }

    /// Whether the value can be stored: a manual expansion is finite.
    pub fn is_finite(self) -> bool {
        self.manual_mm().is_none_or(f64::is_finite)
    }
}

/// The settings of a track beyond its geometry and net.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct TrackAttributes {
    /// A locked track refuses every edit but its unlock.
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub paste_mask: MaskExpansion,
    #[serde(default)]
    pub solder_mask: MaskExpansion,
}

impl TrackAttributes {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Every manual expansion is a finite number.
    pub fn is_finite(&self) -> bool {
        self.paste_mask.is_finite() && self.solder_mask.is_finite()
    }
}

/// One side of a via's solder mask.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ViaMaskSide {
    #[serde(default)]
    pub expansion: MaskExpansion,
    /// Mask covers the via on this side: no opening at all.
    #[serde(default)]
    pub tented: bool,
}

/// The settings of a via beyond its geometry, span and net.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ViaAttributes {
    /// A locked via refuses every edit but its unlock.
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub top: ViaMaskSide,
    #[serde(default)]
    pub bottom: ViaMaskSide,
    /// The expansion is measured from the hole edge, not the via's pad.
    #[serde(default)]
    pub from_hole_edge: bool,
}

impl ViaAttributes {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Every manual expansion is a finite number.
    pub fn is_finite(&self) -> bool {
        self.top.expansion.is_finite() && self.bottom.expansion.is_finite()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_unlocked_rule_masks_and_open_vias() {
        let track = TrackAttributes::default();
        assert!(track.is_default() && !track.locked);
        assert_eq!(track.solder_mask, MaskExpansion::Rule);
        let via = ViaAttributes::default();
        assert!(via.is_default() && !via.top.tented && !via.bottom.tented);
    }

    #[test]
    fn a_manual_expansion_must_be_finite() {
        assert!(MaskExpansion::Manual { mm: -0.05 }.is_finite());
        assert!(!MaskExpansion::Manual { mm: f64::NAN }.is_finite());
        let bad = TrackAttributes {
            paste_mask: MaskExpansion::Manual { mm: f64::INFINITY },
            ..TrackAttributes::default()
        };
        assert!(!bad.is_finite());
    }
}
