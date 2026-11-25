//! Placement previews of the Elliptical Arc and Round Rectangle tools,
//! drawn from the same outline the placed shape uses.

use super::super::*;
use iced::widget::canvas;

impl SymbolCanvas<'_> {
    pub(in crate::library::editor::symbol::canvas) fn draw_curve_preview(
        &self,
        frame: &mut canvas::Frame,
        state: &CanvasState,
    ) {
        let (ghost, is_closed) = match (self.tool, state.rect_from, state.rect_cursor) {
            (SymbolTool::PlaceRoundRectangle, Some(a), Some(b)) => (
                SymbolGraphicKind::round_rectangle([a.0, a.1], [b.0, b.1]),
                true,
            ),
            (SymbolTool::PlaceEllipticalArc, ..) => match state.ellipse.preview() {
                Some(kind) => (kind, false),
                None => return,
            },
            _ => return,
        };
        let cam = self.camera;
        let (scale, ox, oy) = (cam.scale, cam.offset.x, cam.offset.y);
        let w2s =
            |p: [f64; 2]| iced::Point::new(ox + (p[0] as f32) * scale, oy - (p[1] as f32) * scale);
        let outline = ghost.flattened_outline();
        let Some((first, rest)) = outline.split_first() else {
            return;
        };
        let path = canvas::Path::new(|builder| {
            builder.move_to(w2s(*first));
            for p in rest {
                builder.line_to(w2s(*p));
            }
            if is_closed {
                builder.close();
            }
        });
        let preview_color = Color {
            a: 0.55,
            ..self.selected_color
        };
        frame.stroke(
            &path,
            canvas::Stroke::default()
                .with_color(preview_color)
                .with_width(self.preview_stroke_px(0.0, scale)),
        );
    }
}
