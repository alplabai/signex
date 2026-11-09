//! Which Graphite group glyph a bar button draws.
//!
//! Graphite keeps the group glyph on every button (the multi-colour
//! per-action icons would tint into blobs), except the power button: it
//! has a monochrome variant for each port family, so it follows the
//! last-used port the way the legacy bar does.

use iced::widget::svg;
use signex_types::theme::ThemeId;

use super::ActiveBarAction;
use crate::icons as ic;

/// The two power-port families the Graphite power button can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PowerGlyph {
    /// Stem on top, three bars shrinking downwards (GND, signal ground, earth).
    Ground,
    /// A bar on top of a stem (VCC, the +/- rails and the styled rail ports).
    Rail,
}

impl PowerGlyph {
    /// The family of the last-used power port; `None` (nothing placed yet)
    /// is the group default, GND.
    pub(crate) fn of(last: Option<&ActiveBarAction>) -> Self {
        use ActiveBarAction::*;
        match last {
            Some(
                PlacePowerVCC | PlacePowerPlus12 | PlacePowerPlus5 | PlacePowerMinus5
                | PlacePowerArrow | PlacePowerWave | PlacePowerBar | PlacePowerCircle,
            ) => Self::Rail,
            _ => Self::Ground,
        }
    }

    pub(crate) fn handle(self, tid: ThemeId) -> svg::Handle {
        match self {
            Self::Ground => ic::icon_power(tid),
            Self::Rail => ic::icon_power_rail(tid),
        }
    }
}
