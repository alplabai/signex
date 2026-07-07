//! Symbol editor — pan / zoom / fit / cursor camera update logic.

use super::{SymEditor, symbol_bbox};
use crate::library::messages::PrimitiveEditorMsg;

pub(super) fn apply_symbol_camera(editor: &mut SymEditor, msg: PrimitiveEditorMsg) {
    match msg {
        PrimitiveEditorMsg::SymbolPan { dx, dy } => {
            editor.camera.pan(dx, dy);
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolZoom { sx, sy, delta } => {
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
        PrimitiveEditorMsg::SymbolFit => {
            let (min_x, min_y, max_x, max_y) = symbol_bbox(editor.primitive());
            let world_rect = iced::Rectangle {
                x: min_x as f32,
                y: -(max_y as f32),
                width: (max_x - min_x).max(1.0) as f32,
                height: (max_y - min_y).max(1.0) as f32,
            };
            let viewport = iced::Rectangle {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 500.0,
            };
            editor.camera.fit_rect(world_rect, viewport);
            editor.canvas_cache.clear();
        }
        PrimitiveEditorMsg::SymbolCursorAt { x_mm, y_mm } => {
            editor.cursor_mm = match (x_mm, y_mm) {
                (Some(x), Some(y)) => Some((x, y)),
                _ => None,
            };
        }
        _ => {}
    }
}
