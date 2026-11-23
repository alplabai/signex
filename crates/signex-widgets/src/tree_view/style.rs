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
    /// A problem with the row's file ("missing"). On a kind-tag tree the row
    /// leads with a warn glyph, its label takes the tone's ink and the badge
    /// trails as a pill. Coloured by [`TreeStyle::err_tone`].
    Err,
}

impl BadgeTone {
    /// Slot in [`BadgeTones`]; `None` for [`Self::Err`], which has its own.
    fn index(self) -> Option<usize> {
        match self {
            Self::Neutral => Some(0),
            Self::Info => Some(1),
            Self::Ok => Some(2),
            Self::Violet => Some(3),
            Self::Warn => Some(4),
            Self::Err => None,
        }
    }
}

/// `(fill, ink)` of each [`BadgeTone`] but `Err`, in declaration order.
pub type BadgeTones = [(Color, Color); 5];

/// Look of the group header rows: the branches one level under a root.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroupRows {
    /// Label colour; the label is set at the section size, weight 500.
    pub ink: Color,
    /// Chevron and count colour; the count is a plain mono note, no pill.
    pub muted: Color,
}

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
    /// `(fill, ink)` of [`BadgeTone::Err`]; `None` draws it neutral.
    pub err_tone: Option<(Color, Color)>,
    /// Draw a leaf row's badge as a leading kind tag ("SCH") in place of
    /// its file icon, and no icon on branch rows.
    pub kind_tags: bool,
    /// Group header look; `None` draws those rows as any other.
    pub groups: Option<GroupRows>,
    /// Colour of the edge left of the active row. With it the active row
    /// takes the full selection fill and a weight-500 label.
    pub active_edge: Option<Color>,
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
            err_tone: None,
            kind_tags: false,
            groups: None,
            active_edge: None,
        }
    }

    /// Colour the badge pills by [`BadgeTone`].
    pub fn badge_tones(mut self, tones: BadgeTones) -> Self {
        self.tones = Some(tones);
        self
    }

    /// Colour [`BadgeTone::Err`] badges and the problem rows they mark.
    pub fn err_tone(mut self, tone: (Color, Color)) -> Self {
        self.err_tone = Some(tone);
        self
    }

    /// Leaf badges lead the row as kind tags; branches draw no icon.
    pub fn kind_tags(mut self) -> Self {
        self.kind_tags = true;
        self
    }

    /// Draw the branches one level under a root as group headers.
    pub fn group_rows(mut self, groups: GroupRows) -> Self {
        self.groups = Some(groups);
        self
    }

    /// Mark the active row with an edge of `color` on its left.
    pub fn active_edge(mut self, color: Color) -> Self {
        self.active_edge = Some(color);
        self
    }

    /// `(fill, ink)` of `tone`, if this style tints it.
    pub fn tone_colors(&self, tone: BadgeTone) -> Option<(Color, Color)> {
        match tone.index() {
            Some(slot) => self.tones.map(|t| t[slot]),
            None => self.err_tone,
        }
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

/// Padding of the whole tree: the inset each side, less the edge slot on
/// the left of a tree that marks its active row, so the row fills stay put.
pub(super) fn tree_padding(style: Option<&TreeStyle>) -> Padding {
    let Some(s) = style else {
        return Padding::ZERO;
    };
    let slot = if s.active_edge.is_some() {
        super::marks::EDGE_SLOT
    } else {
        0.0
    };
    Padding {
        top: 0.0,
        right: s.inset,
        bottom: 0.0,
        left: (s.inset - slot).max(0.0),
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

    /// The five-tone array still colours its tones; the problem tone has its
    /// own colours and falls back to none, not to a neighbour's slot.
    #[test]
    fn the_err_tone_is_separate_from_the_five_badge_tones() {
        let five = [
            (Color::BLACK, Color::WHITE),
            (Color::WHITE, Color::BLACK),
            (Color::BLACK, Color::BLACK),
            (Color::WHITE, Color::WHITE),
            (Color::TRANSPARENT, Color::BLACK),
        ];
        let s = style().badge_tones(five);
        assert_eq!(s.tone_colors(BadgeTone::Warn), Some(five[4]));
        assert_eq!(s.tone_colors(BadgeTone::Neutral), Some(five[0]));
        assert_eq!(s.tone_colors(BadgeTone::Err), None);
        let err = (
            Color::from_rgb(1.0, 0.9, 0.9),
            Color::from_rgb(0.8, 0.1, 0.1),
        );
        let s = s.err_tone(err);
        assert_eq!(s.tone_colors(BadgeTone::Err), Some(err));
        assert_eq!(s.tone_colors(BadgeTone::Violet), Some(five[3]));
    }

    /// A tree that marks its active row keeps its row fills where they were:
    /// the edge slot comes out of the left inset.
    #[test]
    fn the_edge_slot_comes_out_of_the_left_inset() {
        assert_eq!(tree_padding(None), Padding::ZERO);
        let plain = tree_padding(Some(&style()));
        assert_eq!((plain.left, plain.right), (6.0, 6.0));
        let marked = tree_padding(Some(&style().active_edge(Color::BLACK)));
        assert_eq!(marked.left + super::super::marks::EDGE_SLOT, 6.0);
        assert_eq!(marked.right, 6.0);
    }
}
