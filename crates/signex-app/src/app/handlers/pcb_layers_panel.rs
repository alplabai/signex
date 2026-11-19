//! The Layers panel's own controls (#2363, #103): its tab, search, open
//! colour card, more menu and set name are panel state; a layer set picked,
//! saved or deleted, a section's eye, a typed colour, Layer Stack... and the
//! 3D switch act on the app as the layer set button, the eyes, a colour
//! swatch, the Layer Stack command and View ▸ 3D View do.

use iced::Task;
use iced::widget::operation;
use signex_renderer::pcb::LayerSetId;

use super::pcb_layer_strip::LAYER_STACK_COMMAND;
use crate::app::{Board3dMsg, LayerSetMsg, Message, PcbMsg, PcbViewMsg, Signex};
use crate::panels::{LayersPanelMsg, LayersPanelUi};

impl Signex {
    pub(crate) fn handle_layers_panel(&mut self, msg: &LayersPanelMsg) -> Task<Message> {
        let ctx = &mut self.document_state.panel_ctx;
        if ctx.pcb_layers_ui.apply(msg) {
            match msg {
                // An opened card's hex field starts at the layer's colour.
                LayersPanelMsg::ColourChipToggled(_) => {
                    if let Some(layer) = ctx.pcb_layers_ui.picker {
                        ctx.pcb_layers_ui.hex = ctx.pcb_layers.hex_of(layer);
                    }
                }
                LayersPanelMsg::SetNamingStarted => {
                    return operation::focus(LayersPanelUi::SET_NAME_INPUT);
                }
                _ => {}
            }
            return Task::none();
        }
        let event = match msg {
            LayersPanelMsg::HexEdited(_) => {
                self.apply_typed_layer_colour();
                return Task::none();
            }
            LayersPanelMsg::SectionVisibilitySet { section, visible } => {
                let panel = &self.document_state.panel_ctx.pcb_layers;
                let layers = panel.section_layers(*section);
                let flip_multi_layer = panel.toggles_multi_layer(*section, *visible);
                for layer in layers {
                    let visible = *visible;
                    self.apply_pcb_view_event(
                        None,
                        PcbViewMsg::LayerVisibilitySet { layer, visible },
                    );
                }
                if flip_multi_layer {
                    self.apply_pcb_view_event(None, PcbViewMsg::MultiLayerVisibilityToggled);
                }
                return Task::none();
            }
            LayersPanelMsg::LayerStackPicked => {
                let Ok(id) = crate::keymap::AppCommandId::new(LAYER_STACK_COMMAND) else {
                    return Task::none();
                };
                let args = crate::app::command::CommandArgs::none();
                return self.dispatch_command(&id, args, None);
            }
            LayersPanelMsg::SavedSetShown(name) => PcbMsg::LayerSet(LayerSetMsg::Shown {
                window: None,
                name: name.clone(),
            }),
            LayersPanelMsg::SetSaved => PcbMsg::LayerSet(LayerSetMsg::Saved {
                window: None,
                name: self.document_state.panel_ctx.pcb_layers_ui.set_name.clone(),
            }),
            LayersPanelMsg::SetDeleted(name) => {
                PcbMsg::LayerSet(LayerSetMsg::Deleted(name.clone()))
            }
            LayersPanelMsg::Board3dToggled => PcbMsg::Board3d(Board3dMsg::Toggled),
            LayersPanelMsg::TabPicked(_)
            | LayersPanelMsg::SearchChanged(_)
            | LayersPanelMsg::ColourChipToggled(_)
            | LayersPanelMsg::MoreMenuToggled
            | LayersPanelMsg::MoreMenuDismissed
            | LayersPanelMsg::SetNamingStarted
            | LayersPanelMsg::SetNamingCancelled
            | LayersPanelMsg::SetNameChanged(_) => return Task::none(),
        };
        let task = self.update(Message::Pcb(event));
        self.refresh_panel_ctx();
        // The typed name and its row go once the view shows the set saved
        // under it.
        let ctx = &mut self.document_state.panel_ctx;
        let saved = LayerSetId::Saved(ctx.pcb_layers_ui.set_name.trim().to_string());
        if matches!(msg, LayersPanelMsg::SetSaved) && ctx.pcb_layers.layer_set == Some(saved) {
            ctx.pcb_layers_ui.set_name.clear();
            ctx.pcb_layers_ui.naming = false;
        }
        task
    }

    /// The open colour card's hex field reads as a colour: the layer takes
    /// it at once, and the card stays open for the next keystroke.
    fn apply_typed_layer_colour(&mut self) {
        let ui = &self.document_state.panel_ctx.pcb_layers_ui;
        let (Some(layer), Some(colour)) = (ui.picker, ui.hex_colour()) else {
            return;
        };
        self.apply_pcb_view_event(
            None,
            PcbViewMsg::LayerColourSet {
                layer,
                colour: Some(colour),
            },
        );
    }
}

#[cfg(test)]
#[path = "pcb_layers_panel_tests.rs"]
mod tests;
