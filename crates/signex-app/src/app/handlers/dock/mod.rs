use iced::Task;

use crate::dock::DockMessage;

use super::super::*;

mod floating_layout;
mod library_browser;
mod panel_controls;
mod project_navigation;
mod property_editor;
mod sch_library;

impl Signex {
    pub(crate) fn handle_dock_message(&mut self, msg: DockMessage) -> Task<Message> {
        // v0.9 Library — bubbled from the Library dock panel via
        // DockMessage::Library(LibraryMessage). Re-dispatch through
        // the library subsystem.
        if let DockMessage::Library(lib_msg) = msg.clone() {
            return self.dispatch_library_message(lib_msg);
        }

        if let DockMessage::Panel(panel_msg) = &msg {
            // Panel-control and SCH-library handlers return an optional
            // follow-up task (from a re-entrant self.update); batch it with
            // the finish_update so queued async work isn't dropped. The
            // three middle handlers have no such follow-up and stay bool.
            if let Some(task) = self.handle_dock_panel_control_message(panel_msg) {
                return Task::batch([task, self.finish_update()]);
            }
            if self.handle_dock_library_browser_message(panel_msg)
                || self.handle_dock_property_editor_message(panel_msg)
                || self.handle_dock_project_navigation_panel_message(panel_msg)
            {
                return self.finish_update();
            }
            if let Some(task) = self.handle_dock_sch_library_message(panel_msg) {
                return Task::batch([task, self.finish_update()]);
            }
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
