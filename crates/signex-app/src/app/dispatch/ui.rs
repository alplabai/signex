use iced::Task;

use super::super::*;

impl Signex {
    pub(super) fn dispatch_ui_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ThemeChanged(id) => {
                self.ui_state.theme_id = id;
                self.update_canvas_theme();
                self.finish_update()
            }
            Message::UnitCycled | Message::StatusBar(StatusBarRequest::CycleUnit) => {
                self.handle_unit_cycle_request();
                self.finish_update()
            }
            Message::GridToggle | Message::StatusBar(StatusBarRequest::ToggleGrid) => {
                self.ui_state.grid_visible = !self.ui_state.grid_visible;
                self.interaction_state.canvas.grid_visible = self.ui_state.grid_visible;
                self.interaction_state.pcb_canvas.grid_visible = self.ui_state.grid_visible;
                self.interaction_state.canvas.clear_bg_cache();
                self.interaction_state.pcb_canvas.clear_bg_cache();
                self.finish_update()
            }
            Message::DragStart(target) => {
                self.handle_layout_drag_started(target);
                self.finish_update()
            }
            Message::DragMove(x, y) => {
                self.handle_layout_drag_moved(x, y);
                // Altium parity: cursor leaving the main window during a
                // modal, floating-panel, or tab drag hands the content
                // off to the OS by spawning a detached window.
                let modal_detach = self.check_modal_auto_detach(x, y);
                let panel_detach = self.check_floating_panel_auto_detach(x, y);
                let tab_detach = self.check_tab_auto_detach(x, y);
                let finish = self.finish_update();
                if let Some(modal) = modal_detach {
                    Task::batch([finish, Task::done(Message::DetachModal(modal))])
                } else if let Some(idx) = panel_detach {
                    Task::batch([finish, Task::done(Message::DetachFloatingPanel(idx))])
                } else if let Some(idx) = tab_detach {
                    Task::batch([finish, Task::done(Message::UndockTab(idx))])
                } else {
                    finish
                }
            }
            Message::WindowResized(w, h) => {
                self.ui_state.window_size = (w, h);
                self.finish_update()
            }
            Message::DragEnd => {
                self.handle_layout_drag_finished();
                self.finish_update()
            }
            Message::GridCycle => {
                self.interaction_state.canvas.clear_bg_cache();
                self.finish_update()
            }
            Message::StatusBar(StatusBarRequest::ToggleSnap) => {
                self.ui_state.snap_enabled = !self.ui_state.snap_enabled;
                self.interaction_state.canvas.snap_enabled = self.ui_state.snap_enabled;
                self.finish_update()
            }
            Message::StatusBar(StatusBarRequest::TogglePanelList) => {
                self.dispatch_overlay_message(Message::TogglePanelList)
            }
            Message::CanvasEvent(event) => self.handle_canvas_interaction_event(event),
            _ => unreachable!("dispatch_ui_message received non-ui message"),
        }
    }
}
