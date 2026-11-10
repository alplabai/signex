//! Corner radii of the Graphite design, in px. Legacy chrome keeps
//! [`super::RADIUS`] and [`super::MODAL_CORNER_RADIUS`].

/// The radius scale, smallest to largest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radii {
    /// Rows, menu roots and chips inside a control.
    pub small: f32,
    /// Fields, icon buttons and the Active Bar buttons.
    pub control: f32,
    /// Dialog and toolbar buttons.
    pub button: f32,
    /// Cards, dropdowns and floating panels.
    pub card: f32,
    /// Dialog surfaces.
    pub dialog: f32,
    /// Pills: larger than any height, so the ends are round.
    pub pill: f32,
}

impl Radii {
    pub const GRAPHITE: Self = Self {
        small: 6.0,
        control: 7.0,
        button: 8.0,
        card: 10.0,
        dialog: 12.0,
        pill: 999.0,
    };
}

impl Default for Radii {
    fn default() -> Self {
        Self::GRAPHITE
    }
}
