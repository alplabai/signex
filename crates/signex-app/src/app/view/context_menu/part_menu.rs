//! The placed part's ⋯ menu in Properties (mockup row 42, #3063): a menu
//! over the menu scrim like every other app menu, in the window that shows
//! the Properties panel. Docked in the main window it opens right-aligned in
//! the dock that holds Properties, level with the ⋯ press; elsewhere at the
//! press. A press outside or Esc closes it.

use super::*;

use iced::Widget as _;

use super::items::{dd_msg, text};
use crate::panels::PanelKind;
use signex_widgets::active_bar_dropdown::MenuMetrics;

impl Signex {
    /// The open ⋯ menu with its scrim, when `window_id` paints it; empty
    /// otherwise.
    pub(in crate::app::view) fn part_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let Some(site) = self
            .part_menu_site()
            .filter(|site| site.window == window_id)
        else {
            return Vec::new();
        };
        let ctx = site
            .owner
            .map_or(&self.document_view.panel_ctx, |w| self.panel_ctx_for(w));
        let Some(open) = crate::panels::open_part_menu(ctx) else {
            return Vec::new();
        };
        let i18n = &self.ui_state.i18n;
        let entries: Vec<_> = open
            .entries
            .into_iter()
            .map(|(label, msg)| {
                let message = self.part_menu_message(site.owner, msg);
                dd_msg(None, &text(i18n, label), "", message)
            })
            .collect();
        let panel_ctx = &self.document_view.panel_ctx;
        let docked_in_main =
            site.owner.is_none() && self.ui_state.main_window_id == Some(window_id);
        let anchor = if docked_in_main {
            super::components_row::dock_menu_anchor(
                self.document_view.dock.locate(PanelKind::Properties),
                self.document_area_in(None),
                self.ui_state.window_size.0,
                open.at,
                MenuMetrics::GRAPHITE.fitted_width(&entries, f32::MAX, &panel_ctx.ui_font_name),
            )
        } else {
            // The press, in the coordinates of the window that took it.
            open.at
        };
        let menu = super::placement::menu_card(entries, &panel_ctx.tokens, &panel_ctx.ui_font_name);
        Self::menu_overlay(self.part_menu_dismissed(site.owner), menu.element, anchor)
    }

    /// `body` with the ⋯ menu over it when `window_id` paints the menu: a
    /// Properties panel in a window of its own collects no other overlay.
    pub(in crate::app::view) fn with_part_menu<'a>(
        &'a self,
        window_id: iced::window::Id,
        body: Element<'a, Message>,
    ) -> Element<'a, Message> {
        let layers = self.part_menu_overlay(window_id);
        if layers.is_empty() {
            return body;
        }
        let mut stack = iced::widget::Stack::new()
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .push(body);
        for layer in layers {
            stack = stack.push(layer);
        }
        stack.boxed()
    }
}

#[cfg(test)]
#[path = "part_menu_tests.rs"]
mod tests;
