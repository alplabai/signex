//! Right-click menu of a dock panel tab (Projects, Library, Properties, ...):
//! close it, float it, or move it to another dock region.

use super::*;

use super::items::{dd_msg, text};
use crate::dock::{DockMessage, DockTabAction, PanelPosition};
use iced::Widget as _;
use signex_i18n::{I18n, ids};
use signex_widgets::active_bar_dropdown::DropdownEntry;

/// Pure row data for the menu of a tab docked in `from`. The region it is
/// already in is left out of the Dock rows.
pub(super) fn dock_tab_menu_entries(
    i18n: &I18n,
    from: PanelPosition,
) -> Vec<DropdownEntry<Message>> {
    let pick = |action| Message::Dock(DockMessage::TabMenuActionPicked(action));
    let mut entries = vec![
        dd_msg(
            None,
            &text(i18n, ids::CTX_DOCK_CLOSE),
            "",
            pick(DockTabAction::Close),
        ),
        dd_msg(
            None,
            &text(i18n, ids::CTX_DOCK_FLOAT),
            "",
            pick(DockTabAction::Float),
        ),
        DropdownEntry::Separator,
    ];
    for (position, label) in [
        (PanelPosition::Left, ids::CTX_DOCK_LEFT),
        (PanelPosition::Right, ids::CTX_DOCK_RIGHT),
        (PanelPosition::Bottom, ids::CTX_DOCK_BOTTOM),
    ] {
        if position != from {
            entries.push(dd_msg(
                None,
                &text(i18n, label),
                "",
                pick(DockTabAction::MoveTo(position)),
            ));
        }
    }
    entries
}

impl Signex {
    /// The open dock tab menu, or an empty element when none is open.
    pub(in crate::app::view) fn view_dock_tab_context_menu(&self) -> Element<'_, Message> {
        let Some(menu) = self.document_state.dock.tab_menu else {
            return iced::widget::Space::new().boxed();
        };
        let panel_ctx = &self.document_state.panel_ctx;
        super::placement::menu_card(
            dock_tab_menu_entries(&self.ui_state.i18n, menu.position),
            &panel_ctx.tokens,
            &panel_ctx.ui_font_name,
        )
        .element
    }
}
