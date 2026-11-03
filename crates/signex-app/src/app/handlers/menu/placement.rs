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
        // The same path as the Place Net Label command and the Active Bar.
        if matches!(msg, MenuMessage::PlaceLabel) {
            return Some(
                self.handle_active_bar_action(crate::active_bar::ActiveBarAction::PlaceNetLabel),
            );
        }
        // The picker is the one way to choose a part; arming the tool would
        // leave nothing on the cursor (#1767).
        if matches!(msg, MenuMessage::PlaceComponent) {
            return Some(self.update(Message::Library(crate::library::LibraryMessage::OpenPicker)));
        }
        let tool = match msg {
            MenuMessage::PlaceWire => Tool::Wire,
            MenuMessage::PlaceBus => Tool::Bus,
            MenuMessage::PlaceText => Tool::Text,
            MenuMessage::PlaceNoConnect => Tool::NoConnect,
            MenuMessage::PlaceJunction => Tool::Junction,
            _ => return None,
        };
        // Same path as the toolbar: buffer reset, cursor tag, ghost cleanup.
        Some(self.update(Message::Tool(ToolMessage::SelectTool(tool))))
    }
}
