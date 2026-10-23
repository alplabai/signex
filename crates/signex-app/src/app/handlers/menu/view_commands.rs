use iced::Task;

use crate::dock::DockMessage;
use crate::panels::PanelMsg;

use super::super::super::*;

impl Signex {
    /// Step the grid to the next preset — shared by View > Cycle Grid Size,
    /// `UiMsg::GridCycle` and the `next_grid` command (#791).
    ///
    /// Same preset list as the grid pickers. The snap grid (the size the
    /// status bar shows) steps; the visible grid follows it unless the user
    /// set the two apart.
    pub(crate) fn cycle_grid_size(&mut self) -> Task<Message> {
        let sizes = crate::canvas::grid::GRID_SIZES_MM;
        let current = self.ui_state.grid_size_mm;
        let i = sizes
            .iter()
            .position(|s| (s - current).abs() < f32::EPSILON)
            .unwrap_or(sizes.len() - 1);
        let next = sizes[(i + 1) % sizes.len()];
        let linked = (self.ui_state.visible_grid_mm - current).abs() < f32::EPSILON;
        let mut tasks =
            vec![
                self.update(Message::Dock(DockMessage::Panel(PanelMsg::SetGridSize(
                    next,
                )))),
            ];
        if linked {
            tasks.push(self.update(Message::Dock(DockMessage::Panel(
                PanelMsg::SetVisibleGridSize(next),
            ))));
        }
        self.interaction_state.pcb_canvas.visible_grid_mm = self.ui_state.visible_grid_mm as f64;
        Task::batch(tasks)
    }

    pub(super) fn handle_menu_view_command(&mut self, msg: &MenuMessage) -> Option<Task<Message>> {
        match msg {
            MenuMessage::ZoomFit => {
                if self.has_active_schematic() {
                    self.interaction_state.active_canvas_mut().fit_to_paper();
                    self.interaction_state.active_canvas_mut().clear_bg_cache();
                    self.interaction_state
                        .active_canvas_mut()
                        .clear_content_cache();
                } else if self.has_active_pcb() {
                    self.interaction_state.pcb_canvas.fit_to_board();
                    self.interaction_state.pcb_canvas.clear_bg_cache();
                    self.interaction_state.pcb_canvas.clear_content_cache();
                }
                Some(Task::none())
            }
            MenuMessage::ToggleGrid => {
                self.ui_state.grid_visible = !self.ui_state.grid_visible;
                self.interaction_state.active_canvas_mut().clear_bg_cache();
                self.interaction_state.pcb_canvas.clear_bg_cache();
                Some(Task::none())
            }
            MenuMessage::CycleGrid => Some(self.cycle_grid_size()),
            MenuMessage::ZoomIn | MenuMessage::ZoomOut => {
                let zoom_in = matches!(msg, MenuMessage::ZoomIn);
                if self.has_active_schematic() {
                    let canvas = self.interaction_state.active_canvas_mut();
                    if canvas.zoom_about_centre(zoom_in) {
                        canvas.clear_bg_cache();
                        canvas.clear_content_cache();
                    }
                } else if self.has_active_pcb() {
                    let canvas = &mut self.interaction_state.pcb_canvas;
                    if canvas.zoom_about_centre(zoom_in) {
                        canvas.clear_bg_cache();
                        canvas.clear_content_cache();
                    }
                }
                Some(Task::none())
            }
            _ => None,
        }
    }
}
