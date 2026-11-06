//! Where a context menu opens: the click point, flipped or shifted so the
//! whole card stays inside the window. Every menu overlay uses this one
//! clamp, and sizes come from the type and spacing scales.

use super::*;
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Line height of a text row, relative to its font size (iced's default).
const LINE_HEIGHT: f32 = 1.3;

/// Gap kept between a menu and the window edge.
pub(in crate::app::view) const EDGE_MARGIN: f32 = SP.s;

/// Height of a card with `rows` menu rows: one row is a `title` label
/// between `SP.s + SP.xs / 2` of padding, and the card pads `SP.s` around
/// its rows (`signex_widgets::active_bar::dropdown`).
pub(in crate::app::view) fn menu_height(rows: usize) -> f32 {
    rows as f32 * menu_row_height() + 2.0 * SP.s
}

/// Height of one text-only menu row.
pub(in crate::app::view) fn menu_row_height() -> f32 {
    menu_row_height_with(false)
}

/// Side of a menu row's leading icon (`signex_widgets::active_bar::dropdown`).
const MENU_ICON_SIZE: f32 = 20.0;

/// Height of one menu row; a menu whose rows carry icons has taller rows,
/// since the icon is taller than the text line.
pub(in crate::app::view) fn menu_row_height_with(has_icons: bool) -> f32 {
    let line = (TS.title * LINE_HEIGHT).ceil();
    let content = if has_icons {
        line.max(MENU_ICON_SIZE)
    } else {
        line
    };
    content + 2.0 * (SP.s + SP.xs / 2.0)
}

/// Height of a menu separator line.
const SEPARATOR_HEIGHT: f32 = 1.0;

/// Top-left corner of a flyout of width `sub_w` beside the parent card at
/// `parent` (width `parent_w`) in a window `window_w` wide: right of the
/// parent, or left of it when the right edge would be crossed. Its top lines
/// up with the launcher row under `rows_above` rows and `separators_above`
/// separators of the parent; `has_icons` says whether the parent's rows carry
/// icons (taller rows).
pub(in crate::app::view) fn submenu_origin(
    parent: (f32, f32),
    parent_w: f32,
    sub_w: f32,
    rows_above: usize,
    separators_above: usize,
    has_icons: bool,
    window_w: f32,
) -> (f32, f32) {
    let x = if parent.0 + parent_w + sub_w + EDGE_MARGIN > window_w {
        (parent.0 - sub_w).max(0.0)
    } else {
        parent.0 + parent_w
    };
    let y = parent.1
        + rows_above as f32 * menu_row_height_with(has_icons)
        + separators_above as f32 * SEPARATOR_HEIGHT;
    (x, y)
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

/// `menu` laid at `origin` (from [`clamp_menu_origin`]) on a full-window
/// layer: the one placement every context menu overlay shares.
pub(in crate::app::view) fn anchored_menu<'a, M: 'a>(
    menu: iced::Element<'a, M>,
    origin: (f32, f32),
) -> iced::Element<'a, M> {
    use iced::Length;
    use iced::widget::{Space, Widget as _, column, row};
    column![
        Space::new().height(origin.1),
        row![
            Space::new().width(origin.0),
            menu,
            Space::new().width(Length::Fill)
        ]
        .width(Length::Fill),
    ]
    .boxed()
}

impl Signex {
    /// Where a card of `width` and `rows` rows opened at `anchor` in
    /// `window_id` lands, clamped to that window's own size.
    pub(in crate::app::view) fn menu_origin(
        &self,
        window_id: iced::window::Id,
        anchor: (f32, f32),
        width: f32,
        rows: usize,
    ) -> (f32, f32) {
        clamp_menu_origin(
            anchor,
            (width, menu_height(rows)),
            self.ui_state.size_of_window(window_id),
        )
    }

    /// A right-click menu as overlay layers: the click-outside layer that
    /// sends `close`, then `menu` clamped into `window_id`.
    pub(in crate::app::view) fn menu_overlay<'a>(
        &self,
        window_id: iced::window::Id,
        close: Message,
        menu: iced::Element<'a, Message>,
        anchor: (f32, f32),
        width: f32,
        rows: usize,
    ) -> Vec<iced::Element<'a, Message>> {
        let origin = self.menu_origin(window_id, anchor, width, rows);
        Self::menu_layers(close, menu, origin)
    }

    /// The click-outside layer for `close`, then `menu` laid at `origin`.
    pub(in crate::app::view) fn menu_layers<'a>(
        close: Message,
        menu: iced::Element<'a, Message>,
        origin: (f32, f32),
    ) -> Vec<iced::Element<'a, Message>> {
        vec![Self::dismiss_layer(close), anchored_menu(menu, origin)]
    }

    /// A submenu flyout at `origin`; entering or leaving its panel extends
    /// or restarts the close timer.
    pub(in crate::app::view) fn submenu_flyout<'a>(
        submenu: iced::Element<'a, Message>,
        origin: (f32, f32),
    ) -> iced::Element<'a, Message> {
        let panel = iced::widget::mouse_area(submenu)
            .on_enter(Message::ContextMenu(ContextMenuMsg::SubmenuEnterPanel))
            .on_exit(Message::ContextMenu(ContextMenuMsg::SubmenuLeavePanel));
        use iced::Widget as _;
        anchored_menu(panel.boxed(), origin)
    }
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
    fn a_flyout_opens_right_of_its_parent_and_level_with_the_launcher() {
        let (x, y) = submenu_origin((100.0, 50.0), 200.0, 200.0, 3, 1, false, 1000.0);
        assert_eq!(x, 300.0);
        assert_eq!(y, 50.0 + 3.0 * menu_row_height() + SEPARATOR_HEIGHT);
    }

    #[test]
    fn icon_rows_are_taller_than_text_rows_and_move_the_flyout_down() {
        assert!(menu_row_height_with(true) > menu_row_height_with(false));
        let (_, y) = submenu_origin((100.0, 50.0), 200.0, 200.0, 3, 1, true, 1000.0);
        assert_eq!(
            y,
            50.0 + 3.0 * menu_row_height_with(true) + SEPARATOR_HEIGHT
        );
    }

    #[test]
    fn a_flyout_that_would_cross_the_right_edge_opens_left() {
        let (x, _) = submenu_origin((700.0, 0.0), 200.0, 200.0, 0, 0, false, 1000.0);
        assert_eq!(x, 500.0);
    }

    #[test]
    fn a_menu_in_an_undocked_window_clamps_to_that_windows_own_size() {
        let (mut app, _boot) = Signex::new();
        app.ui_state.window_size = WINDOW;
        let undocked = iced::window::Id::unique();
        app.ui_state
            .undocked_window_sizes
            .insert(undocked, (300.0, 200.0));
        let anchor = (250.0, 190.0);
        assert_eq!(
            app.menu_origin(iced::window::Id::unique(), anchor, MENU.0, 1),
            anchor,
            "the main window has room"
        );
        let (x, y) = app.menu_origin(undocked, anchor, MENU.0, 1);
        assert_eq!(x, 300.0 - MENU.0 - SP.s);
        assert_eq!(y, anchor.1 - menu_height(1));
    }

    #[test]
    fn height_grows_by_one_row_each() {
        assert!(menu_height(3) - menu_height(2) > TS.title);
    }
}
