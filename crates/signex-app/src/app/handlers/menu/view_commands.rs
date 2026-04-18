use iced::Task;

use super::super::super::*;

impl Signex {
    pub(super) fn handle_menu_view_command(&mut self, msg: &MenuMessage) -> Option<Task<Message>> {
        match msg {
            MenuMessage::ZoomFit => {
                if self.has_active_schematic() {
                    self.interaction_state.canvas.fit_to_paper();
                    self.interaction_state.canvas.clear_bg_cache();
                    self.interaction_state.canvas.clear_content_cache();
                } else if self.has_active_pcb() {
                    self.interaction_state.pcb_canvas.fit_to_board();
                    self.interaction_state.pcb_canvas.clear_bg_cache();
                    self.interaction_state.pcb_canvas.clear_content_cache();
                }
                Some(Task::none())
            }
            MenuMessage::ToggleGrid => {
                self.ui_state.grid_visible = !self.ui_state.grid_visible;
                self.interaction_state.canvas.grid_visible = self.ui_state.grid_visible;
                self.interaction_state.pcb_canvas.grid_visible = self.ui_state.grid_visible;
                self.interaction_state.canvas.clear_bg_cache();
                self.interaction_state.pcb_canvas.clear_bg_cache();
                Some(Task::none())
            }
            MenuMessage::CycleGrid => {
                self.interaction_state.canvas.clear_bg_cache();
                Some(Task::none())
            }
            MenuMessage::ZoomIn | MenuMessage::ZoomOut => Some(Task::none()),
            _ => None,
        }
    }
}
