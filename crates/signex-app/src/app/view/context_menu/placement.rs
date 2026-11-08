//! Where a menu card opens: the click point, moved only as far as needed to
//! keep the whole card inside its window. Every right-click menu and its
//! flyout goes through [`super::anchored::AnchoredMenu`], which applies
//! these clamps in layout to the card's real size.

use super::*;
use crate::styles::Spacing;
use signex_widgets::active_bar_dropdown::{DropdownEntry, MenuMetrics};

const SP: Spacing = Spacing::DEFAULT;

/// Gap kept between a menu and the window edge.
pub(in crate::app::view) const EDGE_MARGIN: f32 = SP.s;

/// Top-left corner for a `menu` = `(width, height)` card opened at `anchor`
/// in a `window` = `(width, height)`: the anchor itself, pulled left or up
/// only as far as needed to keep the card [`EDGE_MARGIN`] inside the right
/// and bottom edges, and never past the top-left corner.
pub(in crate::app::view) fn clamp_menu_origin(
    anchor: (f32, f32),
    menu: (f32, f32),
    window: (f32, f32),
) -> (f32, f32) {
    let clamp = |at: f32, size: f32, room: f32| at.min(room - size - EDGE_MARGIN).max(0.0);
    (
        clamp(anchor.0, menu.0, window.0),
        clamp(anchor.1, menu.1, window.1),
    )
}

/// Top-left corner of a `sub` = `(width, height)` flyout beside the parent
/// card at `parent` (width `parent_w`) in `window`: right of the parent, or
/// left of it when the right edge would be crossed. Its top sits `offset`
/// below the parent's top (level with the launcher row), lifted only to
/// stay inside the bottom edge.
pub(in crate::app::view) fn submenu_origin(
    parent: (f32, f32),
    parent_w: f32,
    sub: (f32, f32),
    offset: f32,
    window: (f32, f32),
) -> (f32, f32) {
    let x = if parent.0 + parent_w + sub.0 + EDGE_MARGIN > window.0 {
        (parent.0 - sub.0).max(0.0)
    } else {
        parent.0 + parent_w
    };
    let y = (parent.1 + offset)
        .min(window.1 - sub.1 - EDGE_MARGIN)
        .max(0.0);
    (x, y)
}

/// A menu card built from its rows, with where its flyout goes.
pub(in crate::app::view) struct MenuCard<'a> {
    pub element: Element<'a, Message>,
    /// Where a flyout's top sits below the card's top, level with the open
    /// launcher row; `None` when no launcher is open.
    pub flyout_offset: Option<f32>,
}

/// `entries` as a card fitted to its labels in `tokens`' look, measured in
/// `ui_family`.
pub(in crate::app::view) fn menu_card<'a>(
    entries: Vec<DropdownEntry<Message>>,
    tokens: &'a signex_types::theme::ThemeTokens,
    ui_family: &str,
) -> MenuCard<'a> {
    let metrics = MenuMetrics::of(tokens);
    let width = metrics.fitted_width(&entries, f32::MAX, ui_family);
    // The flyout's first row lines up with the launcher: its top is one
    // card padding above the launcher's top.
    let flyout_offset = metrics
        .active_submenu_top(&entries)
        .map(|top| top - metrics.panel_pad);
    MenuCard {
        element: signex_widgets::active_bar_dropdown::view(entries, tokens, Some(width)),
        flyout_offset,
    }
}

impl Signex {
    /// A right-click menu as overlay layers: the click-outside layer that
    /// sends `close`, then `menu` opened at the window point `anchor`.
    pub(in crate::app::view) fn menu_overlay<'a>(
        close: Message,
        menu: Element<'a, Message>,
        anchor: (f32, f32),
    ) -> Vec<Element<'a, Message>> {
        Self::anchored_layers(close, super::anchored::AnchoredMenu::new(menu, anchor))
    }

    /// The click-outside layer for `close`, then the placed `menu`.
    pub(in crate::app::view) fn anchored_layers<'a>(
        close: Message,
        menu: super::anchored::AnchoredMenu<'a, Message>,
    ) -> Vec<Element<'a, Message>> {
        use iced::Widget as _;
        vec![Self::dismiss_layer(close), menu.boxed()]
    }

    /// A submenu flyout panel; entering or leaving it extends or restarts
    /// the close timer.
    pub(in crate::app::view) fn submenu_panel<'a>(
        submenu: Element<'a, Message>,
    ) -> Element<'a, Message> {
        use iced::Widget as _;
        iced::widget::mouse_area(submenu)
            .on_enter(Message::ContextMenu(ContextMenuMsg::SubmenuEnterPanel))
            .on_exit(Message::ContextMenu(ContextMenuMsg::SubmenuLeavePanel))
            .boxed()
    }
}

#[cfg(test)]
pub(super) mod tests;
