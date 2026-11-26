//! A placed part's Properties messages (mockup rows 42, 50): the ⋯ menu, Copy Library
//! ID, Show net of a focused pin, the illustration hover, and the part
//! sections that fold while a pin is focused.

use iced::Task;

use super::super::super::*;
use crate::library::{LibraryMessage, PlacedPartMsg};
use crate::panels::net_inspector::NetInspectorTab;
use crate::panels::{PanelKind, PartPanelEffect, PartPanelMsg};

impl Signex {
    pub(super) fn handle_dock_part_message(&mut self, msg: &PartPanelMsg) -> Task<Message> {
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
