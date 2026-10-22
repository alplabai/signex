//! Symbol editor — pan / zoom / fit / cursor camera update logic.

use super::{SymEditor, symbol_bbox};
use crate::app::SymbolViewRequest;
use crate::library::messages::SymbolEditorMsg;

pub(super) fn apply_symbol_camera(editor: &mut SymEditor, msg: SymbolEditorMsg) {
    match msg {
        SymbolEditorMsg::Pan { dx, dy } => {
            editor.camera.pan(dx, dy);
            editor.canvas_cache.clear();
        }
        SymbolEditorMsg::Zoom { sx, sy, delta } => {
            let viewport = iced::Rectangle {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            };
            if editor
                .camera
                .zoom_at(iced::Point::new(sx, sy), delta, viewport)
            {
                editor.canvas_cache.clear();
            }
        }
        // Queue only: the canvas reports its real size next event and
        // `ViewportMeasured` does the fit against it.
        SymbolEditorMsg::Fit => {
            editor.pending_view = Some(SymbolViewRequest::Fit);
            editor.canvas_cache.clear();
        }
        SymbolEditorMsg::ViewportMeasured { width, height, .. } => {
            apply_pending_view(editor, width, height);
        }
        SymbolEditorMsg::CursorAt { x_mm, y_mm } => {
            editor.cursor_mm = match (x_mm, y_mm) {
                (Some(x), Some(y)) => Some((x, y)),
                _ => None,
            };
        }
        _ => {}
    }
}

/// Apply the queued camera request against the canvas's measured size.
/// A no-op when nothing is queued (a second measurement can arrive
/// before the first one's message clears the canvas flag).
fn apply_pending_view(editor: &mut SymEditor, width: f32, height: f32) {
    let Some(request) = editor.pending_view.take() else {
        return;
    };
    let viewport = iced::Rectangle {
        x: 0.0,
        y: 0.0,
        width,
        height,
    };
    match request {
        SymbolViewRequest::Fit => {
            let (min_x, min_y, max_x, max_y) = symbol_bbox(editor.primitive());
            let world_rect = iced::Rectangle {
                x: min_x as f32,
                y: -(max_y as f32),
                width: (max_x - min_x).max(1.0) as f32,
                height: (max_y - min_y).max(1.0) as f32,
            };
            editor.camera.fit_rect(world_rect, viewport);
        }
        SymbolViewRequest::CenterOrigin => {
            editor.camera = crate::canvas::Camera::default();
            editor.camera.offset = iced::Point::new(width * 0.5, height * 0.5);
        }
    }
    editor.canvas_cache.clear();
}
