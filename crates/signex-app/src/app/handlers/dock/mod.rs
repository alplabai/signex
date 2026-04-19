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
        if let DockMessage::Panel(panel_msg) = &msg
            && (self.handle_dock_panel_control_message(panel_msg)
                || self.handle_dock_library_browser_message(panel_msg)
                || self.handle_dock_property_editor_message(panel_msg)
                || self.handle_dock_project_navigation_panel_message(panel_msg))
        {
            return self.finish_update();
        }

        if self.handle_dock_floating_layout_message(&msg) {
            return self.finish_update();
        }

        self.document_state.dock.update(msg);
        // Persist the new layout so it survives restart. Cheap: one
        // JSON round-trip on the prefs file per dock mutation.
        crate::fonts::write_dock_layout(&self.document_state.dock);
        self.finish_update()
    }
}
