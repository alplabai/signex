//! Fixed heights of the Graphite chrome bands.
//!
//! The heights are authored in px at the default base size (12 px) and go
//! through [`chrome_px`], so they follow the base like every other chrome
//! length.

use super::typography::chrome_px;

/// `px` at the default 12 px base; [`chrome_px`] is authored at 11 px.
const fn at_default_base(px: f32) -> f32 {
    chrome_px(px * 11.0 / 12.0)
}

/// Heights of the chrome bands and controls, in px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChromeMetrics {
    /// Title and menu bar.
    pub menu: f32,
    /// Document tab strip.
    pub tab_strip: f32,
    /// Dock title bar.
    pub dock_title: f32,
    /// Dock tab strip.
    pub dock_tab_strip: f32,
    /// Status bar.
    pub status: f32,
    /// Collapsed dock rail width.
    pub rail: f32,
    /// Dialog and toolbar buttons.
    pub button: f32,
    /// Text fields.
    pub field: f32,
}

impl ChromeMetrics {
    pub const DEFAULT: Self = Self {
        menu: at_default_base(38.0),
        tab_strip: at_default_base(34.0),
        dock_title: at_default_base(28.0),
        dock_tab_strip: at_default_base(28.0),
        status: at_default_base(26.0),
        rail: at_default_base(28.0),
        button: at_default_base(30.0),
        field: at_default_base(26.0),
    };
}

impl Default for ChromeMetrics {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// At the default base the heights are the Graphite spec's px.
    #[test]
    fn the_default_heights_are_the_spec_px() {
        let m = ChromeMetrics::DEFAULT;
        for (got, want) in [
            (m.menu, 38.0),
            (m.tab_strip, 34.0),
            (m.dock_title, 28.0),
            (m.dock_tab_strip, 28.0),
            (m.status, 26.0),
            (m.rail, 28.0),
            (m.button, 30.0),
            (m.field, 26.0),
        ] {
            assert!((got - want).abs() < 1e-3, "{got} != {want}");
        }
    }
}
