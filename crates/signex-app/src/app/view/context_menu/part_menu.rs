//! The placed part's ⋯ menu in Properties (mockup row 42, #3063): a menu
//! over the menu scrim like every other app menu, right-aligned in the dock
//! that holds Properties and level with the ⋯ press. A press outside
//! closes it.

use super::*;

use super::items::{dd_msg, text};
use crate::dock::DockMessage;
use crate::panels::{PanelKind, PanelMsg, PartPanelMsg};
use signex_widgets::active_bar_dropdown::MenuMetrics;

impl Signex {
    /// The open ⋯ menu of the Properties panel `window_id` shows, with its
    /// scrim; empty when none is open.
    pub(in crate::app::view) fn part_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let owner = self.overlay_owner(window_id);
        let ctx = match owner {
            None => Some(&self.document_state.panel_ctx),
            Some(window) => self.document_state.window_panel_ctx.get(&window),
        };
        let Some(open) = ctx.and_then(crate::panels::open_part_menu) else {
            return Vec::new();
        };
        // A row picked in an undocked window acts on that window's part.
        let message = |msg: PartPanelMsg| {
            let message = Message::Dock(DockMessage::Panel(PanelMsg::Part(msg)));
            match owner {
                None => message,
                Some(window) => self.scope_dock_message(window, message),
            }
        };
        let i18n = &self.ui_state.i18n;
        let entries: Vec<_> = open
            .entries
            .into_iter()
            .map(|(label, msg)| dd_msg(None, &text(i18n, label), "", message(msg)))
            .collect();
        let panel_ctx = &self.document_state.panel_ctx;
        let anchor = match owner {
            None => super::components_row::dock_menu_anchor(
                self.document_state.dock.locate(PanelKind::Properties),
                self.document_area_in(None),
                self.ui_state.window_size.0,
                open.at,
                MenuMetrics::GRAPHITE.fitted_width(&entries, f32::MAX, &panel_ctx.ui_font_name),
            ),
            // An undocked window's panel is beside the press.
            Some(_) => open.at,
        };
        let menu = super::placement::menu_card(entries, &panel_ctx.tokens, &panel_ctx.ui_font_name);
        Self::menu_overlay(message(PartPanelMsg::MenuDismissed), menu.element, anchor)
    }
}
