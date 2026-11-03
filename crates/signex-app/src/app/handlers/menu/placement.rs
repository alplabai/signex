use iced::Task;

use super::super::super::*;

impl Signex {
    /// The Place rows that have no command id: the text and no-connect
    /// tools. Every other Place row runs its command through
    /// `dispatch_command`.
    pub(super) fn handle_menu_placement_command(
        &mut self,
        msg: &MenuMessage,
    ) -> Option<Task<Message>> {
        let tool = match msg {
            MenuMessage::PlaceText => Tool::Text,
            MenuMessage::PlaceNoConnect => Tool::NoConnect,
            _ => return None,
        };
        // Same path as the toolbar: buffer reset, cursor tag, ghost cleanup.
        Some(self.update(Message::Tool(ToolMessage::SelectTool(tool))))
    }
}
