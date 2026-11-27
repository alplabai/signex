//! The PCB panel's controls (mockup row 106): mode, filter and chip are panel
//! state; a net row's click also highlights the net on the board through the
//! Highlight Net path. Nothing here edits the board, but the Polygons mode,
//! whose events the Polygon Manager handles (`handlers/polygon_manager`).

use iced::Task;

use crate::app::{BoardNetMsg, Message, PcbMsg, Signex};
use crate::panels::PcbPanelMsg;
use crate::panels::pcb_nets::highlight_toggle;

impl Signex {
    pub(crate) fn handle_pcb_panel(&mut self, msg: PcbPanelMsg) -> Task<Message> {
        if let PcbPanelMsg::Polygons(event) = msg {
            return self.handle_polygons(event);
        }
        let ui = &mut self.document_view.panel_ctx.pcb_panel;
        let previous = ui.selected.clone();
        let is_click = matches!(msg, PcbPanelMsg::NetClicked(_));
        ui.apply(msg);
        if !is_click {
            return Task::none();
        }
        let selected = ui.selected.clone();
        let query = &self.interaction_state.pcb_query;
        let shown = query.net_highlight.as_ref().map(|h| h.name.as_str());
        match highlight_toggle(shown, previous.as_deref(), selected.as_deref()) {
            Some(net) => self.update(Message::Pcb(PcbMsg::Net(BoardNetMsg::HighlightRequested(
                Some(net),
            )))),
            None => Task::none(),
        }
    }
}
