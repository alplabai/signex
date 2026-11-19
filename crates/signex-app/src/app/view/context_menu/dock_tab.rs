//! Right-click menu of a dock panel tab (Projects, Library, Properties, ...):
//! float it, move it to another dock region, or close it.

use super::*;

use super::items::{dd_msg, text};
use crate::dock::{DockMessage, DockTabAction, PanelPosition};
use crate::icons as ic;
use iced::Widget as _;
use signex_i18n::{I18n, menu_ids};
use signex_types::theme::ThemeId;
use signex_widgets::active_bar_dropdown::DropdownEntry;

/// Pure row data for the menu of a tab docked in `from`: Undock / Float
/// first, the Dock rows (the region it is already in is left out), and
/// Close last, after a separator, so it is not hit by accident (m5).
pub(super) fn dock_tab_menu_entries(
    i18n: &I18n,
    tid: ThemeId,
    from: PanelPosition,
) -> Vec<DropdownEntry<Message>> {
    let pick = |action| Message::Dock(DockMessage::TabMenuActionPicked(action));
    let mut entries = vec![
        dd_msg(
            Some(ic::icon_dd_undock(tid)),
            &text(i18n, menu_ids::CTX_DOCK_FLOAT),
            "",
            pick(DockTabAction::Float),
        ),
        DropdownEntry::Separator,
    ];
    for (position, label) in [
        (PanelPosition::Left, menu_ids::CTX_DOCK_LEFT),
        (PanelPosition::Right, menu_ids::CTX_DOCK_RIGHT),
        (PanelPosition::Bottom, menu_ids::CTX_DOCK_BOTTOM),
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
    entries.extend([
        DropdownEntry::Separator,
        dd_msg(
            Some(ic::icon_dd_close(tid)),
            &text(i18n, menu_ids::CTX_DOCK_CLOSE),
            "",
            pick(DockTabAction::Close),
        ),
    ]);
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
            dock_tab_menu_entries(
                &self.ui_state.i18n,
                self.ui_state.effective_theme_id(),
                menu.position,
            ),
            &panel_ctx.tokens,
            &panel_ctx.ui_font_name,
        )
        .element
    }
}
