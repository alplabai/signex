use iced::Task;

use super::super::*;

impl Signex {
    pub(super) fn dispatch_pcb_message(&mut self, message: PcbMsg) -> Task<Message> {
        match message {
            PcbMsg::RatsnestNetVisibilityToggled(net) => {
                if let Some(path) = self.active_tab_path() {
                    self.interaction_state.pcb_ratsnest.toggle_net(&path, net);
                    // The cached lines stay; only the snapshot is rebuilt.
                    self.sync_pcb_canvas_from_visible_board();
                }
                Task::none()
            }
            PcbMsg::DrcRequested => self.handle_pcb_drc_requested(),
            PcbMsg::DrcFinished { generation, result } => {
                self.handle_pcb_drc_finished(generation, &result);
                Task::none()
            }
            PcbMsg::DrcViolationPicked(index) => {
                self.handle_pcb_drc_violation_picked(index);
                Task::none()
            }
        }
    }
}
