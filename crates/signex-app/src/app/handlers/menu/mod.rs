use iced::Task;

use super::super::*;

mod editing;
mod file_commands;
mod panel_commands;
mod placement;
mod view_commands;

impl Signex {
    pub(crate) fn handle_menu_message(&mut self, msg: MenuMessage) -> Task<Message> {
        if let Some(task) = self.handle_menu_file_command(&msg) {
            return task;
        }
        if let Some(task) = self.handle_menu_view_command(&msg) {
            return task;
        }
        if let Some(task) = self.handle_menu_panel_command(&msg) {
            return task;
        }
        if let Some(task) = self.handle_menu_editing_command(&msg) {
            return task;
        }
        if let Some(task) = self.handle_menu_placement_command(&msg) {
            return task;
        }

        Task::none()
    }
}