use iced::Task;

use super::super::*;

impl Signex {
    pub(super) fn dispatch_pcb_message(&mut self, message: PcbMsg) -> Task<Message> {
        match message {
            PcbMsg::RatsnestNetVisibilityToggled(net) => {
                self.interaction_state.pcb_ratsnest.toggle_net(net);
                // The cached lines stay; only the snapshot is rebuilt.
                self.sync_pcb_canvas_from_visible_board();
                Task::none()
            }
        }
    }
}
