use iced::Task;

use crate::dock::DockMessage;

use super::super::*;

mod child_sheet_identity;
mod floating_layout;
mod layout_persist;
mod library_browser;
mod multi_edit;
mod navigator;
mod net_classes;
mod panel_controls;
mod panel_open;
pub(crate) mod project_navigation;
mod property_editor;
mod sch_library;
mod search_prefs_persist;

impl Signex {
    pub(crate) fn handle_dock_message(&mut self, msg: DockMessage) -> Task<Message> {
        // v0.9 Library — bubbled from the Library dock panel via
        // DockMessage::Library(LibraryMessage). Re-dispatch through
        // the library subsystem.
        if let DockMessage::Library(lib_msg) = msg.clone() {
            return self.dispatch_library_message(lib_msg);
        }

        if let DockMessage::Panel(panel_msg) = &msg {
            // Panel-control, project-navigation, and SCH-library handlers
            // return an optional follow-up task (from a re-entrant
            // self.update, or from opening a project-tree document);
            // batch it with the finish_update so queued async work isn't
            // dropped. The two remaining middle handlers have no such
            // follow-up and stay bool.
            // "Update from library" reads the library off-thread.
            if let crate::panels::PanelMsg::UpdateSymbolFromLibrary(uuid) = panel_msg {
                let task = self.update_symbol_from_library(*uuid);
                return Task::batch([task, self.finish_update()]);
            }
            if let crate::panels::PanelMsg::PlacedPartAction(action) = panel_msg {
                return self.dispatch_library_message(crate::library::LibraryMessage::PlacedPart(
                    crate::library::PlacedPartMsg::ActionRequested(*action),
                ));
            }
            if let crate::panels::PanelMsg::SwitchSymbolChoice(uuid, lib_id) = panel_msg {
                let task = self.handle_switch_symbol_choice(*uuid, lib_id);
                return Task::batch([task, self.finish_update()]);
            }
            if let Some(task) = self.handle_dock_panel_control_message(panel_msg) {
                return Task::batch([task, self.finish_update()]);
            }
            if let Some(task) = self.handle_dock_project_navigation_panel_message(panel_msg) {
                return Task::batch([task, self.finish_update()]);
            }
            if let Some(task) = self.handle_dock_navigator_panel_message(panel_msg) {
                return Task::batch([task, self.finish_update()]);
            }
            if self.handle_dock_library_browser_message(panel_msg)
                || self.handle_dock_multi_edit_message(panel_msg)
                || self.handle_dock_property_editor_message(panel_msg)
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
        // Persist the layout so it survives restart — only when it really
        // changed, debounced, and off the UI thread (#692).
        let persist = self.persist_dock_layout_soon();
        Task::batch([persist, self.finish_update()])
    }
}
