//! Where a context menu opens: the click point, flipped or shifted so the
//! whole card stays inside the window. Every menu overlay uses this one
//! clamp, and sizes come from the type and spacing scales.

use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Line height of a text row, relative to its font size (iced's default).
const LINE_HEIGHT: f32 = crate::styles::TEXT_LINE_HEIGHT;

/// Gap kept between a menu and the window edge.
pub(in crate::app::view) const EDGE_MARGIN: f32 = SP.s;

/// Height of a card with `rows` menu rows: one row is a `title` label
/// between `SP.s + SP.xs / 2` of padding, and the card pads `SP.s` around
/// its rows (`signex_widgets::active_bar::dropdown`).
pub(in crate::app::view) fn menu_height(rows: usize) -> f32 {
    let row = (TS.title * LINE_HEIGHT).ceil() + 2.0 * (SP.s + SP.xs / 2.0);
    rows as f32 * row + 2.0 * SP.s
}

/// Top-left corner for a `menu` = `(width, height)` card opened at `anchor`
/// in a `window` = `(width, height)`: shifted left when it would cross the
/// right edge, and lifted by its own height when it would cross the bottom.
pub(in crate::app::view) fn clamp_menu_origin(
    anchor: (f32, f32),
    menu: (f32, f32),
    window: (f32, f32),
) -> (f32, f32) {
    let edge = EDGE_MARGIN;
    let x = if anchor.0 + menu.0 + edge > window.0 {
        (window.0 - menu.0 - edge).max(0.0)
    } else {
        anchor.0
    };
    let y = if anchor.1 + menu.1 + edge > window.1 {
        (anchor.1 - menu.1).max(0.0)
    } else {
        anchor.1
    };
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: (f32, f32) = (1000.0, 800.0);
    const MENU: (f32, f32) = (200.0, 100.0);

    #[test]
    fn an_anchor_with_room_is_kept() {
        assert_eq!(clamp_menu_origin((10.0, 20.0), MENU, WINDOW), (10.0, 20.0));
    }

    #[test]
    fn a_menu_crossing_the_right_edge_shifts_left_of_it() {
        let (x, y) = clamp_menu_origin((950.0, 20.0), MENU, WINDOW);
        assert_eq!(x, WINDOW.0 - MENU.0 - SP.s);
        assert_eq!(y, 20.0);
    }

    #[test]
    fn a_menu_crossing_the_bottom_edge_opens_upward() {
        let (x, y) = clamp_menu_origin((10.0, 780.0), MENU, WINDOW);
        assert_eq!(x, 10.0);
        assert_eq!(y, 780.0 - MENU.1);
    }

    #[test]
    fn a_menu_larger_than_the_window_stays_on_screen() {
        assert_eq!(
            clamp_menu_origin((5.0, 5.0), (2000.0, 2000.0), WINDOW),
            (0.0, 0.0)
        );
    }

    #[test]
    fn height_grows_by_one_row_each() {
        assert!(menu_height(3) - menu_height(2) > TS.title);
    }
}
