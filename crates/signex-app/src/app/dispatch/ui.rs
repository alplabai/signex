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
                self.interaction_state.active_canvas_mut().grid_visible = self.ui_state.grid_visible;
                self.interaction_state.pcb_canvas.grid_visible = self.ui_state.grid_visible;
                self.interaction_state.active_canvas_mut().clear_bg_cache();
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
                self.interaction_state.active_canvas_mut().clear_bg_cache();
                self.finish_update()
            }
            Message::StatusBar(StatusBarRequest::ToggleSnap) => {
                self.ui_state.snap_enabled = !self.ui_state.snap_enabled;
                self.interaction_state.active_canvas_mut().snap_enabled = self.ui_state.snap_enabled;
                self.finish_update()
            }
            Message::StatusBar(StatusBarRequest::TogglePanelList) => {
                self.dispatch_overlay_message(Message::TogglePanelList)
            }
            Message::CanvasEvent(event) => self.handle_canvas_interaction_event(event),
            Message::CanvasEventInWindow { window_id, event } => {
                self.handle_canvas_event_in_window(window_id, event)
            }
            _ => unreachable!("dispatch_ui_message received non-ui message"),
        }
    }

    /// Handle a `CanvasEvent` that originated in a non-main window.
    ///
    /// The canvas event handlers assume `self.interaction_state.canvas`
    /// is the live target. To avoid rewriting hundreds of call sites,
    /// we temporarily swap the per-window canvas into the main slot
    /// (and point `document_state.active_path` at the window's tab
    /// path so engine lookups resolve correctly), run the handler, and
    /// swap back. Writes to other sub-fields of `interaction_state` /
    /// `document_state` still occur — the user experience is that the
    /// non-main window behaves like "the active window" for the
    /// duration of its event.
    fn handle_canvas_event_in_window(
        &mut self,
        window_id: iced::window::Id,
        event: crate::canvas::CanvasEvent,
    ) -> iced::Task<Message> {
        use crate::app::state::WindowKind;

        // Main window or an unknown window → just run the handler on
        // the legacy canvas.
        if self.ui_state.main_window_id == Some(window_id)
            || !self.interaction_state.canvases.contains_key(&window_id)
        {
            return self.handle_canvas_interaction_event(event);
        }

        // Resolve the target tab path. If the window isn't an undocked
        // tab (detached modal, detached panel), the canvas event makes
        // no sense — drop it.
        let target_path = match self.ui_state.windows.get(&window_id) {
            Some(WindowKind::UndockedTab { path, .. }) => path.clone(),
            _ => return iced::Task::none(),
        };

        // Swap the per-window canvas into the main slot and retarget
        // `active_path` so the handler's engine + canvas accesses hit
        // the window's tab.
        let mut swapped_canvas = self
            .interaction_state
            .canvases
            .remove(&window_id)
            .expect("canvases entry checked above");
        std::mem::swap(&mut self.interaction_state.canvas, &mut swapped_canvas);
        let saved_active_path = self.document_state.active_path.take();
        self.document_state.active_path = Some(target_path);

        let task = self.handle_canvas_interaction_event(event);

        // Restore: the user's event may have mutated the swapped
        // canvas (now in the main slot). Put it back in the HashMap
        // and bring the original main canvas home.
        std::mem::swap(&mut self.interaction_state.canvas, &mut swapped_canvas);
        self.interaction_state
            .canvases
            .insert(window_id, swapped_canvas);
        self.document_state.active_path = saved_active_path;
        task
    }
}
