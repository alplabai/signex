//! Right-click menu of a layer tab under the board canvas (#2379): hide the
//! layer, show it alone, or show every layer again.

use super::*;

use super::items::{dd_msg, text};
use crate::app::{LayerTabMenuMsg, PcbMsg, PcbViewMsg};
use signex_i18n::{I18n, ids};
use signex_types::pcb_layer::PcbLayer;
use signex_widgets::active_bar_dropdown::DropdownEntry;

/// Pure row data for the menu of the tab of `layer`, so the labels and
/// messages are tested without a renderer.
pub(super) fn layer_tab_menu_entries(i18n: &I18n, layer: PcbLayer) -> Vec<DropdownEntry<Message>> {
    let pick = |event| Message::Pcb(PcbMsg::LayerTabMenu(LayerTabMenuMsg::Picked(event)));
    vec![
        dd_msg(
            None,
            &text(i18n, ids::PCB_LAYER_TAB_HIDE),
            "",
            pick(PcbViewMsg::LayerHidden(layer)),
        ),
        dd_msg(
            None,
            &text(i18n, ids::PCB_LAYER_TAB_SHOW_ONLY),
            "",
            pick(PcbViewMsg::OnlyLayerShown(layer)),
        ),
        dd_msg(
            None,
            &text(i18n, ids::PCB_LAYERS_SHOW_ALL),
            "",
            pick(PcbViewMsg::AllLayersShown),
        ),
    ]
}

impl Signex {
    /// The open layer tab menu with its click-outside backdrop, in the
    /// window it was opened in; empty otherwise.
    pub(in crate::app::view) fn layer_tab_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let Some(menu) = self.interaction_state.pcb_layer_tab_menu else {
            return Vec::new();
        };
        if !self.overlay_belongs_to(menu.window, window_id) {
            return Vec::new();
        }
        let panel_ctx = &self.document_state.panel_ctx;
        let card = super::placement::menu_card(
            layer_tab_menu_entries(&self.ui_state.i18n, menu.layer),
            &panel_ctx.tokens,
            &panel_ctx.ui_font_name,
        );
        Self::menu_overlay(
            Message::Pcb(PcbMsg::LayerTabMenu(LayerTabMenuMsg::Dismissed)),
            card.element,
            (menu.x, menu.y),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb_layer::CopperLayer;

    #[test]
    fn the_menu_hides_shows_only_and_shows_all() {
        let layer = PcbLayer::Copper(CopperLayer::Bottom);
        let rows: Vec<(String, Message)> = layer_tab_menu_entries(&I18n::en_us(), layer)
            .into_iter()
            .filter_map(|e| match e {
                DropdownEntry::Item(item) => Some((item.label, item.on_press?)),
                _ => None,
            })
            .collect();
        let labels: Vec<&str> = rows.iter().map(|(label, _)| label.as_str()).collect();
        assert_eq!(
            labels,
            ["Hide this layer", "Show only this layer", "Show all layers"]
        );
        let sent: Vec<PcbViewMsg> = rows
            .iter()
            .map(|(_, message)| match message {
                Message::Pcb(PcbMsg::LayerTabMenu(LayerTabMenuMsg::Picked(event))) => *event,
                other => panic!("{other:?} is not a layer tab menu pick"),
            })
            .collect();
        assert_eq!(
            sent,
            [
                PcbViewMsg::LayerHidden(layer),
                PcbViewMsg::OnlyLayerShown(layer),
                PcbViewMsg::AllLayersShown,
            ]
        );
    }
}
