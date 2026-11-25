//! The edge pan preferences (Preferences ▸ Editing, #2872): on or off, and
//! how fast the view pans at the very edge.

use signex_i18n::{MessageId, ids};

/// Saved in `prefs.json` as `edge_pan_enabled` and `edge_pan_speed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgePanPrefs {
    pub enabled: bool,
    pub speed: EdgePanSpeed,
}

impl Default for EdgePanPrefs {
    fn default() -> Self {
        Self {
            enabled: true,
            speed: EdgePanSpeed::default(),
        }
    }
}

/// How far one tick pans with the pointer at the very edge, as a share of
/// the band's width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgePanSpeed {
    Slow,
    #[default]
    Medium,
    Fast,
}

impl EdgePanSpeed {
    pub const ALL: &'static [EdgePanSpeed] =
        &[EdgePanSpeed::Slow, EdgePanSpeed::Medium, EdgePanSpeed::Fast];

    /// The pan of one tick at the edge, in pixels, for a band `band` wide.
    pub fn step_px(self, band: f32) -> f32 {
        let share = match self {
            EdgePanSpeed::Slow => 0.25,
            EdgePanSpeed::Medium => 0.5,
            EdgePanSpeed::Fast => 1.0,
        };
        band * share
    }

    /// The `prefs.json` token.
    pub fn token(self) -> &'static str {
        match self {
            EdgePanSpeed::Slow => "slow",
            EdgePanSpeed::Medium => "medium",
            EdgePanSpeed::Fast => "fast",
        }
    }

    /// Parse a `prefs.json` token; anything unknown is the default.
    pub fn from_token(token: &str) -> Self {
        Self::ALL
            .iter()
            .copied()
            .find(|speed| token.eq_ignore_ascii_case(speed.token()))
            .unwrap_or_default()
    }

    /// The name the pick list shows, rendered at view time.
    pub fn label(self) -> MessageId {
        match self {
            EdgePanSpeed::Slow => ids::PREFERENCES_EDGE_PAN_SPEED_SLOW,
            EdgePanSpeed::Medium => ids::PREFERENCES_EDGE_PAN_SPEED_MEDIUM,
            EdgePanSpeed::Fast => ids::PREFERENCES_EDGE_PAN_SPEED_FAST,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_speed_round_trips_through_its_token_and_a_faster_one_steps_further() {
        for speed in EdgePanSpeed::ALL {
            assert_eq!(EdgePanSpeed::from_token(speed.token()), *speed);
        }
        assert_eq!(EdgePanSpeed::from_token("bogus"), EdgePanSpeed::Medium);
        let band = 24.0;
        assert!(EdgePanSpeed::Slow.step_px(band) < EdgePanSpeed::Medium.step_px(band));
        assert!(EdgePanSpeed::Medium.step_px(band) < EdgePanSpeed::Fast.step_px(band));
    }
}
