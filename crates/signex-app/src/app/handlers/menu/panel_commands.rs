use iced::Task;

use super::super::super::*;

impl Signex {
    pub(super) fn handle_menu_panel_command(&mut self, msg: &MenuMessage) -> Option<Task<Message>> {
        match msg {
            MenuMessage::OpenProjectsPanel => {
                self.document_state.dock.add_panel(crate::dock::PanelPosition::Left, crate::panels::PanelKind::Projects);
                Some(Task::none())
            }
            MenuMessage::OpenComponentsPanel => {
                self.document_state.dock.add_panel(crate::dock::PanelPosition::Left, crate::panels::PanelKind::Components);
                Some(Task::none())
            }
            MenuMessage::OpenNavigatorPanel => {
                self.document_state.dock.add_panel(crate::dock::PanelPosition::Right, crate::panels::PanelKind::Navigator);
                Some(Task::none())
            }
            MenuMessage::OpenPropertiesPanel => {
                self.document_state.dock.add_panel(crate::dock::PanelPosition::Right, crate::panels::PanelKind::Properties);
                Some(Task::none())
            }
            MenuMessage::OpenMessagesPanel => {
                self.document_state.dock.add_panel(crate::dock::PanelPosition::Bottom, crate::panels::PanelKind::Messages);
                Some(Task::none())
            }
            MenuMessage::OpenSignalPanel => {
                self.document_state.dock.add_panel(crate::dock::PanelPosition::Bottom, crate::panels::PanelKind::Signal);
                Some(Task::none())
            }
            MenuMessage::OpenPreferences => Some(self.update(Message::OpenPreferences)),
            _ => None,
        }
    }
}