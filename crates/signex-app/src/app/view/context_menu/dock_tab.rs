//! Right-click menu of a dock panel tab (Projects, Library, Properties, ...):
//! close it, float it, or move it to another dock region.

use super::*;

use super::items::dd_msg;
use crate::dock::{DockMessage, DockTabAction, PanelPosition};
use iced::Widget as _;
use signex_widgets::active_bar_dropdown::DropdownEntry;

/// Pure row data for the menu of a tab docked in `from`. The region it is
/// already in is left out of the Dock rows.
pub(super) fn dock_tab_menu_entries(from: PanelPosition) -> Vec<DropdownEntry<Message>> {
    let pick = |action| Message::Dock(DockMessage::TabMenuActionPicked(action));
    let mut entries = vec![
        dd_msg(None, "Close", "", pick(DockTabAction::Close)),
        dd_msg(None, "Undock / Float", "", pick(DockTabAction::Float)),
        DropdownEntry::Separator,
    ];
    for (position, label) in [
        (PanelPosition::Left, "Dock Left"),
        (PanelPosition::Right, "Dock Right"),
        (PanelPosition::Bottom, "Dock Bottom"),
    ] {
        if position != from {
            entries.push(dd_msg(
                None,
                label,
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
        let tokens = &self.document_state.panel_ctx.tokens;
        signex_widgets::active_bar_dropdown::view(
            dock_tab_menu_entries(menu.position),
            tokens,
            Some(Self::CONTEXT_MENU_WIDTH),
        )
    }
}
