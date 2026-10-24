use iced::Task;

use super::super::super::*;

impl Signex {
    pub(super) fn handle_menu_placement_command(
        &mut self,
        msg: &MenuMessage,
    ) -> Option<Task<Message>> {
        let tool = match msg {
            MenuMessage::PlaceWire => Tool::Wire,
            MenuMessage::PlaceBus => Tool::Bus,
            MenuMessage::PlaceLabel => Tool::Label,
            MenuMessage::PlaceComponent => Tool::Component,
            _ => return None,
        };
        // Same path as the toolbar: buffer reset, cursor tag, ghost cleanup.
        Some(self.update(Message::Tool(ToolMessage::SelectTool(tool))))
    }
}
