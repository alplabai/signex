use iced::Task;

use crate::dock::DockMessage;

use super::super::*;

mod floating_layout;
mod library_browser;
mod panel_controls;
mod project_navigation;
mod property_editor;

impl Signex {
    pub(crate) fn handle_dock_message(&mut self, msg: DockMessage) -> Task<Message> {
        if let DockMessage::Panel(panel_msg) = &msg {
            if self.handle_dock_panel_control_message(panel_msg)
                || self.handle_dock_library_browser_message(panel_msg)
                || self.handle_dock_property_editor_message(panel_msg)
                || self.handle_dock_project_navigation_panel_message(panel_msg)
            {
                return Task::none();
            }
        }

        if self.handle_dock_floating_layout_message(&msg) {
            return Task::none();
        }

        self.document_state.dock.update(msg);
        Task::none()
    }
}