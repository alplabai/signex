use iced::Task;

use super::super::super::*;

impl Signex {
    pub(super) fn handle_menu_placement_command(
        &mut self,
        msg: &MenuMessage,
    ) -> Option<Task<Message>> {
        match msg {
            MenuMessage::PlaceWire => {
                self.interaction_state.current_tool = Tool::Wire;
                self.clear_measurement();
                Some(Task::none())
            }
            MenuMessage::PlaceBus => {
                self.interaction_state.current_tool = Tool::Bus;
                self.clear_measurement();
                Some(Task::none())
            }
            MenuMessage::PlaceLabel => {
                self.interaction_state.current_tool = Tool::Label;
                self.clear_measurement();
                Some(Task::none())
            }
            MenuMessage::PlaceComponent => {
                self.interaction_state.current_tool = Tool::Component;
                self.clear_measurement();
                Some(Task::none())
            }
            _ => None,
        }
    }
}