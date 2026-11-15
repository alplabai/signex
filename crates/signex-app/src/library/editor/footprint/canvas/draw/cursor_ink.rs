//! Ink of the cursor marks sketch mode draws in place of the hidden OS
//! cursor (#2356): light on the dark board ground, dark on the white
//! sketch canvas, so the mark reads on the ground under it.

use iced::Color;

use super::graphite::look_from_bg;

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

/// Marks for a light ground.
const DARK_INK: CursorInk = CursorInk {
    mark: Color::from_rgba(0.10, 0.10, 0.10, 0.90),
    frame: Color::from_rgba(0.04, 0.18, 0.36, 1.0),
    halo: Color::from_rgba(1.0, 1.0, 1.0, 0.85),
};

/// Marks for a dark ground.
const LIGHT_INK: CursorInk = CursorInk {
    mark: Color::from_rgba(0.94, 0.94, 0.94, 0.95),
    frame: Color::from_rgba(0.55, 0.76, 1.0, 1.0),
    halo: Color::from_rgba(0.0, 0.0, 0.0, 0.75),
};

impl CursorInk {
    /// The ink for a canvas whose background colour is `bg`, in sketch
    /// mode: the Graphite board keeps its own ground, any other canvas
    /// turns white (see `background::draw_backdrop`).
    pub(in crate::library::editor::footprint::canvas) fn for_sketch_on(bg: Color) -> Self {
        let ground = if look_from_bg(bg).is_some() {
            bg
        } else {
            Color::WHITE
        };
        if luminance(ground) < 0.5 {
            LIGHT_INK
        } else {
            DARK_INK
        }
    }
}

/// Perceived brightness of `c`, 0 (black) to 1 (white).
fn luminance(c: Color) -> f32 {
    0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b
}

#[cfg(test)]
mod tests {
    use super::super::graphite::board_look;
    use super::*;

    /// How far apart two colours read, ignoring alpha.
    fn contrast(a: Color, b: Color) -> f32 {
        (luminance(a) - luminance(b)).abs()
    }

    /// #2356: on the dark board ground of the footprint editor and board
    /// sketch, the cursor cross and the reticle stand out from the ground.
    #[test]
    fn the_cursor_marks_read_on_the_board_ground() {
        let ground = board_look().bg;
        let ink = CursorInk::for_sketch_on(ground);
        assert!(contrast(ink.mark, ground) > 0.6, "{ink:?} on {ground:?}");
        assert!(contrast(ink.frame, ground) > 0.4, "{ink:?} on {ground:?}");
        assert!(contrast(ink.halo, ink.mark) > 0.6, "{ink:?}");
    }

    /// Any other canvas paints sketch mode white: the marks stay dark.
    #[test]
    fn the_cursor_marks_read_on_the_white_sketch_canvas() {
        let ink = CursorInk::for_sketch_on(Color::BLACK);
        assert_eq!(ink, DARK_INK);
        assert!(contrast(ink.mark, Color::WHITE) > 0.6);
    }
}
