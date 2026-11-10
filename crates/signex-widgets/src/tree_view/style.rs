//! Opt-in row style for [`TreeView`](super::TreeView): fixed row height,
//! rounded selection and a horizontal inset. Without it the tree keeps its
//! compact legacy look.

use iced::{Color, Padding};

/// Row height in px.
pub const ROW_HEIGHT: f32 = 24.0;
/// Corner radius of the hover / selected row fill.
pub const ROW_RADIUS: f32 = 6.0;
/// Space between the panel edge and the row fill, each side.
pub const ROW_INSET: f32 = 6.0;

/// Tint of a row's badge pill. The colours come from
/// [`TreeStyle::badge_tones`]; without them every badge is neutral.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BadgeTone {
    #[default]
    Neutral,
    Info,
    Ok,
    Violet,
    Warn,
}

impl BadgeTone {
    fn index(self) -> usize {
        match self {
            Self::Neutral => 0,
            Self::Info => 1,
            Self::Ok => 2,
            Self::Violet => 3,
            Self::Warn => 4,
        }
    }
}

/// `(fill, ink)` of each [`BadgeTone`], in declaration order.
pub type BadgeTones = [(Color, Color); 5];

/// Row look for a themed tree. Colours come from the caller's tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeStyle {
    pub row_height: f32,
    pub radius: f32,
    /// Fill of the selected row.
    pub selected_fill: Color,
    /// Label, chevron and dot colour on the selected row.
    pub selected_ink: Color,
    /// Horizontal inset of the whole tree.
    pub inset: f32,
    /// Pill colours per badge tone; `None` keeps every badge neutral.
    pub tones: Option<BadgeTones>,
}

impl TreeStyle {
    /// Default metrics (24 / 6 / 6) with the given selected colours.
    pub fn new(selected_fill: Color, selected_ink: Color) -> Self {
        Self {
            row_height: ROW_HEIGHT,
            radius: ROW_RADIUS,
            selected_fill,
            selected_ink,
            inset: ROW_INSET,
            tones: None,
        }
    }

    /// Colour the badge pills by [`BadgeTone`].
    pub fn badge_tones(mut self, tones: BadgeTones) -> Self {
        self.tones = Some(tones);
        self
    }

    /// `(fill, ink)` of `tone`, if this style tints badges.
    pub fn tone_colors(&self, tone: BadgeTone) -> Option<(Color, Color)> {
        self.tones.map(|t| t[tone.index()])
    }

    pub fn row_height(mut self, row_height: f32) -> Self {
        self.row_height = row_height;
        self
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    pub fn inset(mut self, inset: f32) -> Self {
        self.inset = inset;
        self
    }
}

/// Row button padding: vertical padding collapses to 0 under a fixed row
/// height (the row centres its content); horizontal stays `pad_h`.
pub(super) fn row_padding(style: Option<&TreeStyle>, pad_v: f32, pad_h: f32) -> Padding {
    let v = if style.is_some() { 0.0 } else { pad_v };
    Padding {
        top: v,
        right: pad_h,
        bottom: v,
        left: pad_h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style() -> TreeStyle {
        TreeStyle::new(Color::BLACK, Color::WHITE)
    }

    #[test]
    fn defaults_are_24_6_6() {
        let s = style();
        assert_eq!((s.row_height, s.radius, s.inset), (24.0, 6.0, 6.0));
    }

    #[test]
    fn builders_override_metrics() {
        let s = style().row_height(28.0).radius(4.0).inset(0.0);
        assert_eq!((s.row_height, s.radius, s.inset), (28.0, 4.0, 0.0));
    }

    #[test]
    fn legacy_padding_keeps_vertical_and_styled_drops_it() {
        assert_eq!(row_padding(None, 2.0, 6.0).top, 2.0);
        let s = style();
        let p = row_padding(Some(&s), 2.0, 6.0);
        assert_eq!((p.top, p.bottom, p.left, p.right), (0.0, 0.0, 6.0, 6.0));
    }
}
