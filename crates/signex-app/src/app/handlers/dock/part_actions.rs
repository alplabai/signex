//! A placed part's Properties messages (mockup rows 42, 50): the ⋯ menu, Copy Library
//! ID, Show net of a focused pin, the illustration hover, and the part
//! sections that fold while a pin is focused.

use iced::Task;

use super::super::super::*;
use crate::library::{LibraryMessage, PlacedPartMsg};
use crate::panels::net_inspector::NetInspectorTab;
use crate::panels::{PanelKind, PartPanelEffect, PartPanelMsg};

/// Where the open ⋯ menu is (#3063): the window that paints it, the one
/// showing the Properties panel that holds it, and the document window
/// whose panel state holds it (`None` for the main window's).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PartMenuSite {
    pub window: iced::window::Id,
    pub owner: Option<iced::window::Id>,
}

impl Signex {
    /// The open ⋯ menu's site, if one is open for the selected part. A
    /// Properties panel in a window of its own paints the menu there.
    pub(crate) fn part_menu_site(&self) -> Option<PartMenuSite> {
        let is_open = |owner: Option<iced::window::Id>| {
            let ctx = owner.map_or(&self.document_state.panel_ctx, |w| self.panel_ctx_for(w));
            crate::panels::open_part_menu(ctx).is_some()
        };
        let detached = self.ui_state.windows.iter().find_map(|(id, kind)| {
            matches!(
                kind,
                crate::app::state::WindowKind::DetachedPanel(PanelKind::Properties)
            )
            .then_some(*id)
        });
        if let Some(window) = detached {
            let owner = self.detached_properties_owner(PanelKind::Properties);
            return is_open(owner).then_some(PartMenuSite { window, owner });
        }
        if is_open(None) {
            let window = self.ui_state.main_window_id?;
            return Some(PartMenuSite {
                window,
                owner: None,
            });
        }
        // One menu is open at a time: a press outside one closes it.
        (self.document_state.window_panel_ctx.keys().copied())
            .find(|w| self.is_document_window(*w) && is_open(Some(*w)))
            .map(|window| PartMenuSite {
                window,
                owner: Some(window),
            })
    }

    /// A ⋯ menu message for the panel state of `owner`'s window.
    pub(crate) fn part_menu_message(
        &self,
        owner: Option<iced::window::Id>,
        msg: PartPanelMsg,
    ) -> Message {
        let message = Message::Dock(crate::dock::DockMessage::Panel(
            crate::panels::PanelMsg::Part(msg),
        ));
        match owner {
            None => message,
            Some(window) => self.scope_dock_message(window, message),
        }
    }

    /// What closes the ⋯ menu held by `owner`'s panel state: a press
    /// outside it or Esc.
    pub(crate) fn part_menu_dismissed(&self, owner: Option<iced::window::Id>) -> Message {
        self.part_menu_message(owner, PartPanelMsg::MenuDismissed)
    }

    pub(super) fn handle_dock_part_message(&mut self, msg: &PartPanelMsg) -> Task<Message> {
        // Opening the ⋯ menu closes every other menu first, as each menu's
        // open handler does; a toggle that closes it closes nothing else.
        if let PartPanelMsg::MenuToggled(part) = msg
            && self.document_state.panel_ctx.part_panel.menu_for != Some(*part)
        {
            self.close_all_context_menus();
        }
        let part_panel = &mut self.document_state.panel_ctx.part_panel;
        let effect = part_panel.update(msg.clone());
        // The ⋯ menu opens level with the press (#3063).
        if matches!(msg, PartPanelMsg::MenuToggled(_)) && part_panel.menu_for.is_some() {
            part_panel.menu_at = self.interaction_state.last_mouse_pos;
        }
        match effect {
            PartPanelEffect::None => Task::none(),
            PartPanelEffect::Action(action) => self.dispatch_library_message(
                LibraryMessage::PlacedPart(PlacedPartMsg::ActionRequested(action)),
            ),
            PartPanelEffect::CopyToClipboard(text) => iced::clipboard::write(text).discard(),
            PartPanelEffect::ShowNet(net) => self.show_net_in_inspector(net),
        }
    }

    /// Open the Net Inspector on `net`: its by-net list filtered to it, the
    /// net's row expanded.
    fn show_net_in_inspector(&mut self, net: String) -> Task<Message> {
        let inspector = &mut self.ui_state.net_inspector;
        inspector.tab = NetInspectorTab::ByNet;
        inspector.filter = net.clone();
        inspector.expanded.insert(net);
        self.show_panel(PanelKind::NetInspector)
    }

    /// Fold the part sections while a pin of the selected part is focused,
    /// and give the user's state back when the focus ends.
    pub(crate) fn sync_pin_focus_sections(&mut self, is_pin_focused: bool) {
        let ctx = &mut self.document_state.panel_ctx;
        ctx.part_panel
            .sync_pin_focus(&mut ctx.collapsed_sections, is_pin_focused);
    }

    /// Whether a pin focus on `symbol` folds the part sections. A power
    /// port's panel shows no pin facts in exchange, so its Location stays
    /// open (mockup row 45).
    pub(crate) fn pin_focus_folds(&self, symbol: Option<uuid::Uuid>) -> bool {
        symbol.is_some_and(|id| {
            let is_power = self.document_state.active_engine().is_some_and(|engine| {
                (engine.document().symbols.iter()).any(|s| s.uuid == id && s.is_power)
            });
            !is_power
        })
    }
}
