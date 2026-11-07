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
                .zoom_at(iced::Point::new(sx, sy), delta, editor.zoom_step, viewport)
            {
                editor.canvas_cache.clear();
            }
        }
        SymbolEditorMsg::ZoomFactor { sx, sy, factor } => {
            if editor
                .camera
                .zoom_by_factor(iced::Point::new(sx, sy), factor)
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
        SymbolViewRequest::Fit => fit_symbol(editor, viewport),
        // A fresh tab of a symbol that has content opens fitted, not as a
        // speck at 100% around the origin; an empty symbol keeps the origin.
        SymbolViewRequest::CenterOrigin if has_content(editor.primitive()) => {
            fit_symbol(editor, viewport);
        }
        SymbolViewRequest::CenterOrigin => {
            editor.camera = crate::canvas::Camera::default();
            editor.camera.offset = iced::Point::new(width * 0.5, height * 0.5);
        }
    }
    editor.canvas_cache.clear();
}

/// Fit a symbol that has content into a placeholder `width` x `height`
/// viewport. The canvas replaces it with the measured size on its first
/// redraw, so the tab opens fitted even before (or without) a measurement.
pub(crate) fn fit_to_viewport(editor: &mut SymEditor, width: f32, height: f32) {
    if has_content(editor.primitive()) {
        let viewport = iced::Rectangle {
            x: 0.0,
            y: 0.0,
            width,
            height,
        };
        fit_symbol(editor, viewport);
    }
}

/// Whether the symbol has anything to fit.
fn has_content(sym: &signex_library::Symbol) -> bool {
    !sym.pins.is_empty() || !sym.graphics.is_empty()
}

/// Largest scale a fit may pick: 400%, so a one-pin symbol stays a symbol.
const FIT_MAX_SCALE: f32 = signex_types::schematic::SCHEMATIC_ZOOM_100_SCALE * 4.0;
/// Margin kept clear on every side of a fit.
const FIT_MARGIN: f32 = 72.0;
/// Extra room kept clear at the top for the floating Active Bar.
pub(crate) const FIT_TOP_CLEARANCE: f32 = 56.0;

/// Fit the symbol's bounding box into `viewport`, capped at 400% and
/// centred below the floating Active Bar.
fn fit_symbol(editor: &mut SymEditor, viewport: iced::Rectangle) {
    let (min_x, min_y, max_x, max_y) = symbol_bbox(editor.primitive());
    let world_w = (max_x - min_x).max(1.0) as f32;
    let world_h = (max_y - min_y).max(1.0) as f32;
    let avail_w = (viewport.width - FIT_MARGIN * 2.0).max(1.0);
    let avail_h = (viewport.height - FIT_MARGIN * 2.0 - FIT_TOP_CLEARANCE).max(1.0);
    let camera = &mut editor.camera;
    camera.scale = (avail_w / world_w)
        .min(avail_h / world_h)
        .clamp(crate::canvas::Camera::MIN_SCALE, FIT_MAX_SCALE);
    let centre_x = (min_x as f32 + world_w / 2.0) * camera.scale;
    let centre_y = (-(max_y as f32) + world_h / 2.0) * camera.scale;
    camera.offset.x = viewport.width / 2.0 - centre_x;
    camera.offset.y = (viewport.height + FIT_TOP_CLEARANCE) / 2.0 - centre_y;
}
