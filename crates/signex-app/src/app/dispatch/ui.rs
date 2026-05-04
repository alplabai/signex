use iced::Task;

use super::super::*;

impl Signex {
    pub(super) fn dispatch_ui_message(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ThemeChanged(id) => {
                self.ui_state.theme_id = id;
                self.update_canvas_theme();
                crate::fonts::write_theme_pref(id);
                self.finish_update()
            }
            Message::UnitCycled | Message::StatusBar(StatusBarRequest::CycleUnit) => {
                self.handle_unit_cycle_request();
                self.finish_update()
            }
            Message::GridToggle | Message::StatusBar(StatusBarRequest::ToggleGrid) => {
                self.ui_state.grid_visible = !self.ui_state.grid_visible;
                self.interaction_state.active_canvas_mut().grid_visible =
                    self.ui_state.grid_visible;
                self.interaction_state.pcb_canvas.grid_visible = self.ui_state.grid_visible;
                self.interaction_state.active_canvas_mut().clear_bg_cache();
                self.interaction_state.pcb_canvas.clear_bg_cache();
                crate::fonts::write_grid_visible_pref(self.ui_state.grid_visible);
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
            Message::GridPickerOpen => {
                // v0.18.10 — only mount the picker when the active
                // tab is a Footprint editor; the schematic / PCB
                // grid systems aren't wired through this picker yet.
                let active_tab_kind = self
                    .document_state
                    .tabs
                    .get(self.document_state.active_tab)
                    .map(|t| t.kind.clone());
                let footprint_active = matches!(
                    active_tab_kind,
                    Some(crate::app::TabKind::FootprintEditor(_))
                );
                tracing::info!(
                    target: "signex::ui",
                    footprint_active = footprint_active,
                    last_mouse_pos = ?self.interaction_state.last_mouse_pos,
                    "GridPickerOpen received",
                );
                if footprint_active {
                    let (x, y) = self.interaction_state.last_mouse_pos;
                    self.interaction_state.grid_picker =
                        Some(crate::app::GridPickerState { x, y });
                }
                self.finish_update()
            }
            Message::GridPickerClose => {
                self.interaction_state.grid_picker = None;
                self.finish_update()
            }
            Message::GridPickerSelect(step_mm) => {
                self.interaction_state.grid_picker = None;
                if let Some(editor) = self.active_footprint_editor_mut() {
                    if step_mm > 0.0 && step_mm.is_finite() {
                        editor.state.snap_options.grid_step_mm = step_mm;
                        editor.canvas_cache.clear();
                    }
                }
                self.refresh_panel_ctx();
                self.finish_update()
            }
            Message::GridPropertiesOpen => {
                // Pre-populate from the active footprint editor's
                // current step. No-op for non-footprint tabs (the
                // modal would have nothing to drive).
                let active_step = self
                    .active_footprint_editor()
                    .map(|e| e.state.snap_options.grid_step_mm);
                tracing::info!(
                    target: "signex::ui",
                    active_step = ?active_step,
                    "GridPropertiesOpen received",
                );
                if let Some(step) = active_step {
                    let s = format!("{step:.4}");
                    self.ui_state.grid_properties = Some(crate::app::GridPropertiesState {
                        step_x_mm: s.clone(),
                        step_y_mm: s,
                        link_xy: true,
                    });
                }
                self.finish_update()
            }
            Message::GridPropertiesClose => {
                self.ui_state.grid_properties = None;
                self.finish_update()
            }
            Message::GridPropertiesSetStepX(value) => {
                if let Some(state) = self.ui_state.grid_properties.as_mut() {
                    state.step_x_mm = value;
                    if state.link_xy {
                        state.step_y_mm = state.step_x_mm.clone();
                    }
                }
                self.finish_update()
            }
            Message::GridPropertiesSetStepY(value) => {
                if let Some(state) = self.ui_state.grid_properties.as_mut() {
                    state.step_y_mm = value;
                    if state.link_xy {
                        state.step_x_mm = state.step_y_mm.clone();
                    }
                }
                self.finish_update()
            }
            Message::GridPropertiesToggleLink => {
                if let Some(state) = self.ui_state.grid_properties.as_mut() {
                    state.link_xy = !state.link_xy;
                    if state.link_xy {
                        // Re-link mirrors X into Y so re-enabling
                        // doesn't keep a desynced pair.
                        state.step_y_mm = state.step_x_mm.clone();
                    }
                }
                self.finish_update()
            }
            Message::GridPropertiesApply => {
                // Validate the X step (Y is taken from X for now —
                // single-axis steps in `SnapOptions`; the Y field
                // exists in the dialog for forward compatibility).
                let parsed_x = self
                    .ui_state
                    .grid_properties
                    .as_ref()
                    .and_then(|s| s.step_x_mm.trim().parse::<f64>().ok());
                if let Some(step) = parsed_x {
                    if step > 0.0 && step.is_finite() {
                        if let Some(editor) = self.active_footprint_editor_mut() {
                            editor.state.snap_options.grid_step_mm = step;
                            editor.canvas_cache.clear();
                        }
                    }
                }
                self.ui_state.grid_properties = None;
                self.refresh_panel_ctx();
                self.finish_update()
            }
            Message::StatusBar(StatusBarRequest::ToggleSnap) => {
                self.ui_state.snap_enabled = !self.ui_state.snap_enabled;
                self.interaction_state.active_canvas_mut().snap_enabled =
                    self.ui_state.snap_enabled;
                crate::fonts::write_snap_enabled_pref(self.ui_state.snap_enabled);
                self.finish_update()
            }
            Message::StatusBar(StatusBarRequest::TogglePanelList) => {
                self.dispatch_overlay_message(Message::TogglePanelList)
            }
            Message::StatusBar(StatusBarRequest::OpenPropertiesForSelection) => {
                self.dispatch_overlay_message(Message::Menu(MenuMessage::OpenPropertiesPanel))
            }
            Message::CanvasEvent(event) => {
                // First user gesture on the canvas dismisses the
                // first-run tour card (UX §4.3). Cheap inline check —
                // false branch when the card is already dismissed.
                if self.ui_state.first_run_tour_open {
                    self.ui_state.first_run_tour_open = false;
                    crate::fonts::write_first_run_tour_dismissed(true);
                }
                self.handle_canvas_interaction_event(event)
            }
            Message::CanvasEventInWindow { window_id, event } => {
                if self.ui_state.first_run_tour_open {
                    self.ui_state.first_run_tour_open = false;
                    crate::fonts::write_first_run_tour_dismissed(true);
                }
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
    ///
    /// **Invariant:** any `Task<Message>` returned by the handler runs
    /// AFTER the swap unwinds. If a handler ever chains
    /// `Task::done(Message::CanvasEvent(..))` expecting to land back
    /// in the window that produced the original event, it must use
    /// `Message::CanvasEventInWindow { window_id, .. }` instead — the
    /// plain `CanvasEvent` form always targets the main window.
    ///
    /// **Panic safety:** the swap is guarded with `catch_unwind` +
    /// `resume_unwind` so a panicking handler still restores both the
    /// canvas slot and `active_path` before the panic propagates.
    fn handle_canvas_event_in_window(
        &mut self,
        window_id: iced::window::Id,
        event: crate::canvas::CanvasEvent,
    ) -> iced::Task<Message> {
        use crate::app::state::WindowKind;
        use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

        // Main window → run the handler directly on the legacy canvas.
        if self.ui_state.main_window_id == Some(window_id) {
            return self.handle_canvas_interaction_event(event);
        }

        // Unknown window (closed mid-queue, or never had a canvas) —
        // drop the event. Previously fell through to the main canvas,
        // which could apply a stale undocked click to the main window
        // during the race between `SecondaryWindowClosed` and a queued
        // CanvasEvent.
        if !self.interaction_state.canvases.contains_key(&window_id) {
            return iced::Task::none();
        }

        // Resolve the target tab path. Non-tab windows (detached
        // modals, detached panels) can't host a canvas today, so an
        // event from one is nonsensical — drop it.
        let target_path = match self.ui_state.windows.get(&window_id) {
            Some(WindowKind::UndockedTab { path, .. }) => path.clone(),
            _ => {
                debug_assert!(
                    false,
                    "CanvasEventInWindow for a non-UndockedTab window: {window_id:?}"
                );
                return iced::Task::none();
            }
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

        // Run the handler. `AssertUnwindSafe` is needed because
        // `&mut self` isn't `UnwindSafe` by default — we're accepting
        // that a panicking handler may leave `self` in a partially
        // mutated state, which is fine as long as the swap itself
        // unwinds deterministically below.
        let task_result = catch_unwind(AssertUnwindSafe(|| {
            self.handle_canvas_interaction_event(event)
        }));

        // Always-run cleanup (runs on success, error return, or panic
        // via resume_unwind below): swap the canvases back and restore
        // the saved active_path. The per-window canvas (now in the
        // main slot) returns to the HashMap; the main canvas (held in
        // `swapped_canvas`) returns to the main slot.
        std::mem::swap(&mut self.interaction_state.canvas, &mut swapped_canvas);
        self.interaction_state
            .canvases
            .insert(window_id, swapped_canvas);
        self.document_state.active_path = saved_active_path;

        match task_result {
            Ok(task) => task,
            Err(payload) => resume_unwind(payload),
        }
    }
}
