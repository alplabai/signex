//! The look of a Components panel table row (#2535, mockup row 52): its
//! colours and which fill its state asks for.

use iced::Color;

/// The theme colours a Components panel row is drawn with.
#[derive(Debug, Clone, Copy)]
pub(super) struct RowColors {
    pub text: Color,
    pub muted: Color,
    /// Selection fill.
    pub tint: Color,
    /// Hover fill.
    pub hover: Color,
    /// Fill of the row whose menu is open.
    pub menu: Color,
}

/// What the row's state asks of its look.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct RowFlags {
    pub selected: bool,
    pub hovered: bool,
    /// The row's context menu is open: it stays highlighted.
    pub menu_open: bool,
}

impl RowFlags {
    /// The fill the row is drawn with, if any: an open menu wins, then the
    /// selection, then the hover.
    pub(super) fn fill(self, colors: RowColors) -> Option<Color> {
        if self.menu_open {
            Some(colors.menu)
        } else if self.selected {
            Some(colors.tint)
        } else {
            self.hovered.then_some(colors.hover)
        }
    }
}
