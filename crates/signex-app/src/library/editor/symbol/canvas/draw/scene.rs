//! Scene overlay — resize handles for the currently-selected
//! graphic(s), shown only in the Select tool. Extracted verbatim from
//! `Program::draw`; the symbol body itself renders via
//! `draw_symbol_with_renderer` (still in the parent `canvas` module).

use super::super::*;
use iced::Size;
use iced::widget::canvas;

impl SymbolCanvas<'_> {
    /// Corner + edge-midpoint resize handles for selected graphics.
    pub(in crate::library::editor::symbol::canvas) fn draw_resize_handles(
        &self,
        frame: &mut canvas::Frame,
    ) {
        let cam = self.camera;
        let scale = cam.scale;
        let ox = cam.offset.x;
        let oy = cam.offset.y;
        let w2s = |x: f64, y: f64| -> iced::Point {
            iced::Point::new(ox + (x as f32) * scale, oy - (y as f32) * scale)
        };
        // Resize handles for placed graphics — visible in the Select tool
        // only for the currently-selected graphic(s) so the canvas isn't
        // cluttered when nothing is selected.
        // Corner handles (squares, half=3 px) are visually larger than edge
        // midpoint handles (squares, half=2 px) so the user can tell them
        // apart at a glance.
        if self.tool == SymbolTool::Select {
            for idx in 0..self.symbol.graphics.len() {
                if !is_graphic_selected(&self.selected, idx) {
                    continue;
                }
                if !state::graphic_on_part(&self.symbol.graphics[idx], self.active_part) {
                    continue;
                }
                for (handle, pos) in state::graphic_handles(self.symbol, idx) {
                    let p = w2s(pos[0], pos[1]);
                    let is_corner = matches!(handle, state::GraphicHandle::RectCorner(_));
                    let half = if is_corner { 3.0_f32 } else { 2.0_f32 };
                    let top_left = iced::Point::new(p.x - half, p.y - half);
                    let size = Size::new(half * 2.0, half * 2.0);
                    let path = canvas::Path::rectangle(top_left, size);
                    frame.fill(&path, self.bg_color);
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_color(self.selected_color)
                            .with_width(stroke_px_at_zoom(SYMBOL_HANDLE_STROKE_PX_AT_100, scale)),
                    );
                }
            }
        }
    }
}
