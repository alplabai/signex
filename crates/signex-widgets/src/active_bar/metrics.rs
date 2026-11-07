//! Pixel metrics of the Active Bar row.
//!
//! The renderer, the slot-offset maths and the app's hit-test all read the
//! same [`Metrics`], so a drawn button and its hit rect cannot drift apart.
//! Legacy themes keep the original 26 / 1 / 2 / 2 numbers; Graphite uses
//! 30 px buttons with 3 px padding and a 1 px gap.

use signex_types::theme::ThemeTokens;

/// Layout numbers of one Active Bar variant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    /// Square button edge.
    pub btn: f32,
    /// Separator width.
    pub sep_w: f32,
    /// Container padding, all four sides.
    pub padding: f32,
    /// Gap between adjacent slots.
    pub spacing: f32,
    /// Icon edge inside a button.
    pub icon: f32,
    /// Dropdown chevron edge.
    pub chevron: f32,
    /// Separator height.
    pub sep_h: f32,
}

impl Metrics {
    /// The original multi-colour bar.
    pub const LEGACY: Self = Self {
        btn: 26.0,
        sep_w: 1.0,
        padding: 2.0,
        spacing: 2.0,
        icon: 20.0,
        chevron: 14.0,
        sep_h: 18.0,
    };

    /// Graphite: 30 px buttons, 16 px icons, 3 px padding, 1 px gap.
    pub const GRAPHITE: Self = Self {
        btn: 30.0,
        sep_w: 1.0,
        padding: 3.0,
        spacing: 1.0,
        icon: 16.0,
        chevron: 10.0,
        sep_h: 18.0,
    };

    /// Metrics for `tokens`: Graphite when the theme sets extended tokens.
    pub fn of(tokens: &ThemeTokens) -> Self {
        if tokens.extended.is_empty() {
            Self::LEGACY
        } else {
            Self::GRAPHITE
        }
    }

    /// Height of the whole bar: one button row plus the padding.
    pub fn bar_height(&self) -> f32 {
        self.btn + 2.0 * self.padding
    }
}
