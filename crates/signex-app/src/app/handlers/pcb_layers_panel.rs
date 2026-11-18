//! The Layers panel's own controls (#2363): its tab, search, open colour
//! chip and set name are panel state; a layer set picked, saved or deleted
//! and the 3D switch run what the layer set button and View ▸ 3D View run.

use iced::Task;
use signex_renderer::pcb::LayerSetId;

use crate::app::{Board3dMsg, LayerSetMsg, Message, PcbMsg, Signex};
use crate::panels::LayersPanelMsg;

impl Signex {
    pub(crate) fn handle_layers_panel(&mut self, msg: &LayersPanelMsg) -> Task<Message> {
        let ui = &mut self.document_state.panel_ctx.pcb_layers_ui;
        if ui.apply(msg) {
            return Task::none();
        }
        let event = match msg {
            LayersPanelMsg::SavedSetShown(name) => PcbMsg::LayerSet(LayerSetMsg::Shown {
                window: None,
                name: name.clone(),
            }),
            LayersPanelMsg::SetSaved => PcbMsg::LayerSet(LayerSetMsg::Saved {
                window: None,
                name: ui.set_name.clone(),
            }),
            LayersPanelMsg::SetDeleted(name) => {
                PcbMsg::LayerSet(LayerSetMsg::Deleted(name.clone()))
            }
            LayersPanelMsg::Board3dToggled => PcbMsg::Board3d(Board3dMsg::Toggled),
            LayersPanelMsg::TabPicked(_)
            | LayersPanelMsg::SearchChanged(_)
            | LayersPanelMsg::ColourChipToggled(_)
            | LayersPanelMsg::CustomColourOpened
            | LayersPanelMsg::CustomColourClosed
            | LayersPanelMsg::SetNameChanged(_) => return Task::none(),
        };
        let task = self.update(Message::Pcb(event));
        self.refresh_panel_ctx();
        // The typed name goes once the view shows the set saved under it.
        let ctx = &mut self.document_state.panel_ctx;
        let saved = LayerSetId::Saved(ctx.pcb_layers_ui.set_name.trim().to_string());
        if matches!(msg, LayersPanelMsg::SetSaved) && ctx.pcb_layers.layer_set == Some(saved) {
            ctx.pcb_layers_ui.set_name.clear();
        }
        task
    }
}
