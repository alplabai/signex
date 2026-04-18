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
                Some(Task::none())
            }
            MenuMessage::PlaceBus => {
                self.interaction_state.current_tool = Tool::Bus;
                Some(Task::none())
            }
            MenuMessage::PlaceLabel => {
                self.interaction_state.current_tool = Tool::Label;
                Some(Task::none())
            }
            MenuMessage::PlaceComponent => {
                self.interaction_state.current_tool = Tool::Component;
                Some(Task::none())
            }
            _ => None,
        }
    }
}
