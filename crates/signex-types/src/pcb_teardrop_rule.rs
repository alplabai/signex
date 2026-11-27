//! The board's teardrop rule: which pads and vias Add Teardrops widens and
//! how wide and long a teardrop is, with a per-pad override. Board data in
//! `[board_options.teardrops]` (`snxpcb/1.12`), changed only through the
//! board engine's `SetBoardOptions` command. The default is what
//! Add Teardrops did before the rule existed, so a board without one
//! writes no section. See `docs/PCB_TOOLS.md`, "Teardrop rule".

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// How far a teardrop's base reaches across the pad, as a share of the
/// pad's or via's inscribed radius on the track's nearer side.
pub const DEFAULT_WIDTH_SHARE: f64 = 0.9;

/// How far a teardrop runs past the pad's edge along the track, as a share
/// of the pad's or via's inscribed radius.
pub const DEFAULT_LENGTH_SHARE: f64 = 1.0;

/// The longest teardrop, as a share of the radius.
pub const MAX_LENGTH_SHARE: f64 = 4.0;

/// The teardrop rule of a board.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TeardropRule {
    /// Pads get teardrops unless a pad's override says otherwise.
    pub pads: bool,
    /// Vias get teardrops.
    pub vias: bool,
    /// In `(0, 1]`: [`DEFAULT_WIDTH_SHARE`].
    pub width_share: f64,
    /// In `(0, MAX_LENGTH_SHARE]`: [`DEFAULT_LENGTH_SHARE`].
    pub length_share: f64,
    /// Pads that follow their own setting instead of [`Self::pads`]: `true`
    /// always takes a teardrop, `false` never does. Keyed by pad uuid,
    /// which is unique on a board.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pad_overrides: BTreeMap<Uuid, bool>,
}

impl Default for TeardropRule {
    fn default() -> Self {
        Self {
            pads: true,
            vias: true,
            width_share: DEFAULT_WIDTH_SHARE,
            length_share: DEFAULT_LENGTH_SHARE,
            pad_overrides: BTreeMap::new(),
        }
    }
}

/// Why a teardrop rule cannot be stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TeardropRuleError {
    #[error("the teardrop width is a share of the pad above 0 and at most 1")]
    Width,
    #[error("the teardrop length is a share of the pad above 0 and at most 4")]
    Length,
}

impl TeardropRule {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Whether the pad `pad` takes a teardrop: its override, else the rule.
    pub fn pad_wants(&self, pad: Uuid) -> bool {
        self.pad_overrides.get(&pad).copied().unwrap_or(self.pads)
    }

    /// The pad's own setting, `None` when it follows the rule.
    pub fn pad_override(&self, pad: Uuid) -> Option<bool> {
        self.pad_overrides.get(&pad).copied()
    }

    /// Give `pad` its own setting, or `None` to follow the rule again.
    pub fn set_pad_override(&mut self, pad: Uuid, wants: Option<bool>) {
        match wants {
            Some(on) => self.pad_overrides.insert(pad, on),
            None => self.pad_overrides.remove(&pad),
        };
    }

    /// Whether the shares can shape a teardrop; a value that is not a
    /// number is out of range.
    pub fn check(&self) -> Result<(), TeardropRuleError> {
        let width = self.width_share;
        if !(width.is_finite() && width > 0.0 && width <= 1.0) {
            return Err(TeardropRuleError::Width);
        }
        let length = self.length_share;
        if !(length.is_finite() && length > 0.0 && length <= MAX_LENGTH_SHARE) {
            return Err(TeardropRuleError::Length);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_rule_widens_every_pad_and_via_as_before() {
        let rule = TeardropRule::default();
        assert!(rule.is_default() && rule.pads && rule.vias);
        assert_eq!(rule.width_share, 0.9);
        assert_eq!(rule.length_share, 1.0);
        assert_eq!(rule.check(), Ok(()));
    }

    #[test]
    fn a_pad_override_wins_over_the_rule_and_clears_back_to_it() {
        let pad = Uuid::from_u128(7);
        let mut rule = TeardropRule {
            pads: false,
            ..TeardropRule::default()
        };
        assert!(!rule.pad_wants(pad));
        rule.set_pad_override(pad, Some(true));
        assert!(rule.pad_wants(pad));
        assert_eq!(rule.pad_override(pad), Some(true));
        rule.set_pad_override(pad, None);
        assert_eq!(rule.pad_override(pad), None);
        assert!(!rule.pad_wants(pad));
    }

    #[test]
    fn shares_outside_their_range_or_not_a_number_are_refused() {
        let with = |width_share, length_share| TeardropRule {
            width_share,
            length_share,
            ..TeardropRule::default()
        };
        assert_eq!(with(0.0, 1.0).check(), Err(TeardropRuleError::Width));
        assert_eq!(with(1.01, 1.0).check(), Err(TeardropRuleError::Width));
        assert_eq!(with(f64::NAN, 1.0).check(), Err(TeardropRuleError::Width));
        assert_eq!(with(0.5, 0.0).check(), Err(TeardropRuleError::Length));
        assert_eq!(with(0.5, 4.5).check(), Err(TeardropRuleError::Length));
        assert_eq!(
            with(0.5, f64::INFINITY).check(),
            Err(TeardropRuleError::Length)
        );
        assert_eq!(with(1.0, 4.0).check(), Ok(()));
    }
}
