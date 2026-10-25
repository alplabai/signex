//! Click-click Move / Move Selection (#271): arming from the Active Bar and
//! resolving the picking press. The canvas owns the floating gesture and the
//! drop commits through `CanvasEvent::MoveSelected`, so this file only holds
//! the app-side arm.

use super::super::super::*;
use crate::canvas::MovePick;

impl Signex {
    /// Cancel an armed or floating click-click move on every window's
    /// canvas. The canvas drops a float whose epoch no longer matches, so
    /// this also covers a move already floating (tab switch, reload, undo,
    /// tool change).
    pub(crate) fn cancel_move_pick(&mut self) {
        self.ui_state.move_pick = None;
        let cancel = |canvas: &mut crate::canvas::CanvasSlot| {
            canvas.move_pick = None;
            canvas.move_epoch += 1;
        };
        cancel(&mut self.interaction_state.canvas);
        for canvas in self.interaction_state.canvases.values_mut() {
            cancel(canvas);
        }
    }

    /// Arm `pick` on the app state and the active canvas, then switch to the
    /// Select tool. Move Selection needs a selection to lift; with none it
    /// logs a hint and stays unarmed.
    pub(in crate::app) fn arm_move_pick(&mut self, pick: MovePick) {
        if pick == MovePick::Selection && self.interaction_state.active_canvas().selected.is_empty()
        {
            crate::diagnostics::log_warning("Select something to move first");
            return;
        }
        self.clear_transient_schematic_tool_state();
        self.interaction_state.current_tool = Tool::Select;
        self.ui_state.move_pick = Some(pick);
        let canvas = self.interaction_state.active_canvas_mut();
        canvas.move_pick = Some(pick);
        canvas.clear_overlay_cache();
        crate::diagnostics::log_info(match pick {
            MovePick::Object => "Click an object to move it (Esc to cancel)",
            MovePick::Selection => {
                "Click a reference point, then click where it should go (Esc to cancel)"
            }
        });
    }

    /// The canvas picked up the move at `(world_x, world_y)`: consume the arm
    /// and, for Move, make the object under the cursor the only selection.
    pub(crate) fn handle_canvas_move_picked(&mut self, world_x: f64, world_y: f64) {
        let pick = self.ui_state.move_pick.take();
        self.interaction_state.active_canvas_mut().move_pick = None;
        if pick == Some(MovePick::Object) {
            let filters = &self.interaction_state.selection_filters;
            let hit = self.active_render_snapshot().and_then(|snapshot| {
                crate::schematic_runtime::hit_test::hit_test(snapshot, world_x, world_y)
                    .filter(|h| crate::app::passes_filter(h, snapshot, filters))
            });
            if let Some(hit) = hit {
                self.interaction_state.active_canvas_mut().selected = vec![hit];
                self.update_selection_info();
            } else {
                // The canvas already floats; with nothing to lift the old
                // selection would move instead, so cancel the float.
                self.interaction_state.active_canvas_mut().move_epoch += 1;
            }
        }
        self.interaction_state
            .active_canvas_mut()
            .clear_overlay_cache();
    }
}
