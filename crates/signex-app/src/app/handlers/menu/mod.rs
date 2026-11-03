use iced::Task;

use super::super::*;

mod editing;
pub(crate) mod export;
mod file_commands;
mod panel_commands;
mod placement;
pub(crate) mod save_target;
mod view_commands;

impl Signex {
    pub(crate) fn handle_menu_message(&mut self, msg: MenuMessage) -> Task<Message> {
        // A row that carries a command id runs it like its shortcut does.
        if let MenuMessage::Command(id) = &msg {
            return self.run_menu_command(id, None);
        }
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
