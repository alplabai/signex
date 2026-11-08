//! Graphite canvas zoom pill over the schematic canvas's bottom-right corner.
//! It sends the messages View > Zoom In, Zoom Out and Fit All send, so it adds
//! no behaviour of its own. The widget is `crate::canvas::zoom_pill`.

use iced::Element;
use iced::widget::Widget;

use super::*;
use crate::canvas::zoom_pill::{pill_size, view as pill};
use crate::menu_bar::MenuMessage;

/// Gap between the pill and the canvas edges.
const EDGE_GAP: f32 = 12.0;

/// Top-left of the pill for a canvas at `canvas`.
pub(in crate::app::view) fn pill_origin(canvas: iced::Rectangle) -> (f32, f32) {
    let (w, h) = pill_size();
    (
        canvas.x + canvas.width - EDGE_GAP - w,
        canvas.y + canvas.height - EDGE_GAP - h,
    )
}

impl Signex {
    /// The zoom pill over the main window's schematic canvas. An undocked
    /// window has no known canvas rectangle and the zoom messages act on
    /// the main canvas, so it gets none.
    pub(in crate::app::view) fn canvas_zoom_pill_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Option<Element<'_, Message>> {
        let tokens = &self.document_state.panel_ctx.tokens;
        if self.ui_state.main_window_id != Some(window_id) {
            return None;
        }
        self.document_state
            .engine_for_window(window_id, &self.ui_state)?;
        let zoom = self
            .interaction_state
            .canvas_for_window(window_id)
            .zoom_percent();
        let at = pill_origin(self.canvas_bounds());
        let pill = pill(
            tokens,
            zoom,
            Message::Menu(MenuMessage::ZoomOut),
            Message::Menu(MenuMessage::ZoomIn),
            Message::CanvasEvent(crate::canvas::CanvasEvent::FitAll),
        );
        // The `Translate` spans the window; only the pill may take the wheel.
        Some(super::super::translate::Translate::new(super::scroll_guard(pill), at).boxed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pill sits `EDGE_GAP` from the canvas's bottom-right corner.
    #[test]
    fn the_pill_hugs_the_bottom_right_corner() {
        let canvas = iced::Rectangle {
            x: 100.0,
            y: 80.0,
            width: 800.0,
            height: 600.0,
        };
        let (w, h) = pill_size();
        let (x, y) = pill_origin(canvas);
        assert_eq!(x + w, 900.0 - EDGE_GAP);
        assert_eq!(y + h, 680.0 - EDGE_GAP);
    }
}
