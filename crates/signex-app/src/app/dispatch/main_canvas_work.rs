//! What main's canvas has under way while an undocked window's handler runs.
//! Main's slot sits in a local then, so what the handler ended is cleared
//! from it afterwards.

use super::super::*;

/// Main's wire, dimension and paused annotation tool before a window's
/// handler ran.
pub(super) struct MainCanvasWork {
    has_wire: bool,
    has_dimension: bool,
    has_annotation_pause: bool,
}

impl Signex {
    pub(super) fn main_canvas_work(&self) -> MainCanvasWork {
        let state = &self.interaction_state;
        MainCanvasWork {
            has_wire: state.wire_drawing && state.wire_window.is_none(),
            has_dimension: state.dimension_points_window() == Some(None),
            has_annotation_pause: state.paused_annotation_window() == Some(None),
        }
    }

    /// Clear main's slot of the work the window's handler ended. Runs with
    /// main's canvas, panel context and active sheet back in place.
    pub(super) fn end_main_canvas_work(&mut self, before: &MainCanvasWork) {
        let after = self.main_canvas_work();
        if before.has_wire && !after.has_wire {
            self.interaction_state.canvas.wire_preview.clear();
            self.interaction_state.canvas.drawing_mode = false;
        }
        if before.has_dimension && !after.has_dimension {
            self.drop_dimension_form_draft();
            self.sync_annotation_draft();
            self.interaction_state.canvas.clear_overlay_cache();
        }
        let state = &mut self.interaction_state;
        let is_still_paused = matches!(
            state.placement_pause,
            Some(crate::app::state::SheetPlacementPause::Sheet { window: None, .. })
        );
        if before.has_annotation_pause && !is_still_paused {
            let canvas = &mut state.canvas;
            canvas.placement_paused = false;
            canvas.tool_preview = None;
            canvas.annotation_draft = None;
            canvas.clear_overlay_cache();
        }
    }
}
