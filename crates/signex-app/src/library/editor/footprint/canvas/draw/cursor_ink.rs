//! Ink of the cursor marks sketch mode draws in place of the hidden OS
//! cursor (#2356): light marks on the dark board ground, which sketch mode
//! paints on every theme (decision 7, Caner 2026-11-20).

use iced::Color;

/// Colours of the sketch cursor marks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::library::editor::footprint::canvas) struct CursorInk {
    /// The cross arms and the resize arrow.
    pub mark: Color,
    /// The square around the snapped reticle.
    pub frame: Color,
    /// The halo behind the resize arrow.
    pub halo: Color,
}

/// Marks for the dark board ground.
const LIGHT_INK: CursorInk = CursorInk {
    mark: Color::from_rgba(0.94, 0.94, 0.94, 0.95),
    frame: Color::from_rgba(0.55, 0.76, 1.0, 1.0),
    halo: Color::from_rgba(0.0, 0.0, 0.0, 0.75),
};

impl CursorInk {
    /// The ink of sketch mode: it paints the board ground whatever the
    /// canvas colour (see `background::draw_backdrop`), so the ink is the
    /// board's.
    pub(in crate::library::editor::footprint::canvas) fn sketch() -> Self {
        LIGHT_INK
    }
}

#[cfg(test)]
mod tests {
    use super::super::graphite::board_look;
    use super::*;

    /// Perceived brightness of `c`, 0 (black) to 1 (white).
    fn luminance(c: Color) -> f32 {
        0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b
    }

    /// How far apart two colours read, ignoring alpha.
    fn contrast(a: Color, b: Color) -> f32 {
        (luminance(a) - luminance(b)).abs()
    }

    /// #2356: on the dark board ground of the footprint editor and board
    /// sketch, the cursor cross and the reticle stand out from the ground.
    #[test]
    fn the_cursor_marks_read_on_the_board_ground() {
        let ground = board_look().bg;
        let ink = CursorInk::sketch();
        assert!(contrast(ink.mark, ground) > 0.6, "{ink:?} on {ground:?}");
        assert!(contrast(ink.frame, ground) > 0.4, "{ink:?} on {ground:?}");
        assert!(contrast(ink.halo, ink.mark) > 0.6, "{ink:?}");
    }

    /// Every theme paints sketch mode on the board ground (Caner
    /// 2026-11-20, decision 7): the marks stay light.
    #[test]
    fn the_cursor_marks_read_on_the_board_ground_on_every_theme() {
        let ink = CursorInk::sketch();
        assert_eq!(ink, LIGHT_INK);
        assert!(contrast(ink.mark, board_look().bg) > 0.6);
    }
}
