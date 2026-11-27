//! The look of a Components panel table row (#2535): which fill its
//! state asks for. The row's gestures are tested with the table
//! (`table/view_tests.rs`).

use super::rows::{RowColors, RowFlags};

/// The open menu's row wins over the selection, the selection over hover.
#[test]
fn the_row_whose_menu_is_open_stays_highlighted() {
    let colors = RowColors {
        text: iced::Color::BLACK,
        muted: iced::Color::BLACK,
        tint: iced::Color::from_rgb(0.1, 0.0, 0.0),
        hover: iced::Color::from_rgb(0.0, 0.1, 0.0),
        menu: iced::Color::from_rgb(0.0, 0.0, 0.1),
    };
    let flags = |selected, hovered, menu_open| RowFlags {
        selected,
        hovered,
        menu_open,
    };
    assert_eq!(flags(true, true, true).fill(colors), Some(colors.menu));
    assert_eq!(flags(true, true, false).fill(colors), Some(colors.tint));
    assert_eq!(flags(false, true, false).fill(colors), Some(colors.hover));
    assert_eq!(flags(false, false, false).fill(colors), None);
}
