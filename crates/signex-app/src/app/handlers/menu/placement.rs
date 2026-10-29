use iced::Task;

use super::super::super::*;

impl Signex {
    pub(super) fn handle_menu_placement_command(
        &mut self,
        msg: &MenuMessage,
    ) -> Option<Task<Message>> {
        if matches!(msg, MenuMessage::PlacePowerPort) {
            return Some(
                self.handle_active_bar_action(crate::active_bar::ActiveBarAction::PlacePowerGND),
            );
        }
        let tool = match msg {
            MenuMessage::PlaceWire => Tool::Wire,
            MenuMessage::PlaceBus => Tool::Bus,
            MenuMessage::PlaceLabel => Tool::Label,
            MenuMessage::PlaceComponent => Tool::Component,
            MenuMessage::PlaceText => Tool::Text,
            MenuMessage::PlaceNoConnect => Tool::NoConnect,
            _ => return None,
        };
        // Same path as the toolbar: buffer reset, cursor tag, ghost cleanup.
        Some(self.update(Message::Tool(ToolMessage::SelectTool(tool))))
    }
}
