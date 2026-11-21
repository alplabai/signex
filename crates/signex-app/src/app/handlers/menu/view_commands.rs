use iced::Task;

use crate::dock::DockMessage;
use crate::panels::PanelMsg;

use super::super::super::*;

impl Signex {
    /// Step the grid to the next preset — shared by `UiMsg::GridCycle` and the
    /// `next_grid` command, which View > Cycle Grid Size runs (#791).
    ///
    /// Same preset list as the grid pickers. The snap grid (the size the
    /// status bar shows) steps; the visible grid follows it unless the user
    /// set the two apart.
    pub(crate) fn cycle_grid_size(&mut self, forward: bool) -> Task<Message> {
        let sizes = crate::canvas::grid::GRID_SIZES_MM;
        let current = self.snap_grid_in_view();
        let i = sizes
            .iter()
            .position(|s| (s - current).abs() < f32::EPSILON)
            .unwrap_or(sizes.len() - 1);
        let next = if forward {
            sizes[(i + 1) % sizes.len()]
        } else {
            sizes[(i + sizes.len() - 1) % sizes.len()]
        };
        self.set_snap_grid(next)
    }

    /// Snap to the user's fast grid `index` (`fast_grid_1` / `fast_grid_2`),
    /// set in Preferences (#1672).
    pub(crate) fn pick_fast_grid(&mut self, index: usize) -> Task<Message> {
        match self.ui_state.fast_grids_mm.get(index) {
            Some(&size) => self.set_snap_grid(size),
            None => Task::none(),
        }
    }

    /// `cycle_fast_grid`: step to the next fast grid, wrapping; from a grid
    /// that is not a fast grid, land on fast grid 1.
    pub(crate) fn cycle_fast_grid(&mut self) -> Task<Message> {
        let fast = self.ui_state.fast_grids_mm;
        let current = self.snap_grid_in_view();
        let next = fast
            .iter()
            .position(|s| (s - current).abs() < f32::EPSILON)
            .map_or(0, |i| (i + 1) % fast.len());
        self.set_snap_grid(fast[next])
    }

    /// The snap grid of the document in view: a board's own, else the
    /// app-wide one.
    fn snap_grid_in_view(&self) -> f32 {
        self.active_pcb().map_or(self.ui_state.grid_size_mm, |board| {
            signex_engine::pcb::placement::grid_step_mm(board) as f32
        })
    }

    /// Set the snap grid. On a board it is the board's own, one undo step.
    /// Elsewhere the visible grid follows unless the user set the two apart.
    /// The board's Grid popover picks through it too (#2460).
    pub(crate) fn set_snap_grid(&mut self, next: f32) -> Task<Message> {
        if self.has_active_pcb() {
            self.set_pcb_grid_preset(next);
            return Task::none();
        }
        let current = self.ui_state.grid_size_mm;
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
            MenuMessage::ZoomIn | MenuMessage::ZoomOut => {
                let zoom_in = matches!(msg, MenuMessage::ZoomIn);
                let step = self.ui_state.effective_zoom_step();
                if self.has_active_schematic() {
                    let canvas = self.interaction_state.active_canvas_mut();
                    if canvas.zoom_about_centre(zoom_in, step) {
                        canvas.clear_bg_cache();
                        canvas.clear_content_cache();
                    }
                } else if self.has_active_pcb() {
                    let canvas = &mut self.interaction_state.pcb_canvas;
                    if canvas.zoom_about_centre(zoom_in, step) {
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
