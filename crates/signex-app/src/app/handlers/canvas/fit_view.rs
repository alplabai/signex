//! Camera framing for a box of world points: the rectangle a fit request
//! carries, and zoom-to-selection (#1256).

use super::super::super::*;

/// Smallest side of a fitted box, so a lone item does not zoom to a dot.
const MIN_FIT_SIDE_MM: f32 = 16.0;
/// Breathing room around the framed extent.
const FIT_MARGIN_MM: f32 = 2.0;

/// The viewport rectangle that frames `[min_x, min_y, max_x, max_y]`.
pub(crate) fn fit_rect(bounds: [f64; 4]) -> iced::Rectangle {
    let [x0, y0, x1, y1] = bounds.map(|v| v as f32);
    let (w, h) = (
        (x1 - x0 + 2.0 * FIT_MARGIN_MM).max(MIN_FIT_SIDE_MM),
        (y1 - y0 + 2.0 * FIT_MARGIN_MM).max(MIN_FIT_SIDE_MM),
    );
    iced::Rectangle {
        x: (x0 + x1) / 2.0 - w / 2.0,
        y: (y0 + y1) / 2.0 - h / 2.0,
        width: w,
        height: h,
    }
}

impl Signex {
    /// Frame the active sheet's selection. Nothing selected is a no-op that
    /// says so in the Messages panel.
    pub(super) fn fit_selection(&mut self) {
        if !self.has_active_schematic() {
            crate::diagnostics::log_info("Zoom to selection works on schematic sheets");
            return;
        }
        let selected = self.interaction_state.active_canvas().selected.clone();
        if selected.is_empty() {
            crate::diagnostics::log_info("Zoom to selection: nothing is selected");
            return;
        }
        let bounds = self.active_render_snapshot().and_then(|snapshot| {
            crate::schematic_runtime::selection::selection_bounds(
                snapshot,
                &selected,
                &self.interaction_state.active_canvas().designators,
            )
        });
        let Some(b) = bounds else {
            crate::diagnostics::log_info("Zoom to selection: the selection has no extent here");
            return;
        };
        self.interaction_state
            .active_canvas_mut()
            .pending_fit
            .set(Some(fit_rect([b.min_x, b.min_y, b.max_x, b.max_y])));
        self.clear_active_camera_caches();
    }
}
