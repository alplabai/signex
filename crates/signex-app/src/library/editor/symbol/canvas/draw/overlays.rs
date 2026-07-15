//! Top overlays — the rubber-band box selection and the line /
//! circle / arc multi-click placement previews. Drawn last (top of
//! the z-stack).

use super::super::*;
use iced::Color;
use iced::Size;
use iced::widget::canvas;

impl SymbolCanvas<'_> {
    /// Rubber-band box selection overlay (Window blue / Crossing green).
    pub(in crate::library::editor::symbol::canvas) fn draw_box_select_overlay(
        &self,
        frame: &mut canvas::Frame,
        state: &CanvasState,
    ) {
        let cam = self.camera;
        let scale = cam.scale;
        let ox = cam.offset.x;
        let oy = cam.offset.y;
        let w2s = |x: f64, y: f64| -> iced::Point {
            iced::Point::new(ox + (x as f32) * scale, oy - (y as f32) * scale)
        };
        if let (Some((ox, oy)), Some((cx, cy))) =
            (state.box_select_origin, state.box_select_current)
        {
            let p0 = w2s(ox, oy);
            let p1 = w2s(cx, cy);
            let left = p0.x.min(p1.x);
            let top = p0.y.min(p1.y);
            let width = (p1.x - p0.x).abs();
            let height = (p1.y - p0.y).abs();

            // Direction determines colour:
            //   cx > ox  (L→R) = Window selection  → blue fill + blue outline
            //   cx < ox  (R→L) = Crossing selection → green fill + green outline
            let is_crossing = cx < ox;
            let (fill_color, stroke_color) = if is_crossing {
                (
                    Color::from_rgba(0.1, 0.85, 0.25, 0.08),
                    Color::from_rgba(0.1, 0.85, 0.25, 0.90),
                )
            } else {
                (
                    Color::from_rgba(0.15, 0.45, 0.95, 0.08),
                    Color::from_rgba(0.15, 0.45, 0.95, 0.90),
                )
            };
            let rect_origin = iced::Point::new(left, top);
            let rect_size = Size::new(width, height);
            frame.fill_rectangle(rect_origin, rect_size, fill_color);
            frame.stroke(
                &canvas::Path::rectangle(rect_origin, rect_size),
                canvas::Stroke::default()
                    .with_color(stroke_color)
                    .with_width(1.5),
            );
        }
    }

    /// Two-click rectangle placement preview — a rubber-band outline
    /// spanning the committed first corner and the live cursor.
    pub(in crate::library::editor::symbol::canvas) fn draw_rect_preview(
        &self,
        frame: &mut canvas::Frame,
        state: &CanvasState,
    ) {
        let cam = self.camera;
        let scale = cam.scale;
        let ox = cam.offset.x;
        let oy = cam.offset.y;
        let w2s = |x: f64, y: f64| -> iced::Point {
            iced::Point::new(ox + (x as f32) * scale, oy - (y as f32) * scale)
        };
        if let (Some((fx, fy)), Some((cx, cy))) = (state.rect_from, state.rect_cursor) {
            let p0 = w2s(fx, fy);
            let p1 = w2s(cx, cy);
            // Screen-space top-left + size (y is flipped, so min/max in
            // screen coords, not world coords).
            let left = p0.x.min(p1.x);
            let top = p0.y.min(p1.y);
            let width = (p1.x - p0.x).abs();
            let height = (p1.y - p0.y).abs();
            let preview_color = Color {
                a: 0.55,
                ..self.selected_color
            };
            let rect_origin = iced::Point::new(left, top);
            let rect_size = Size::new(width, height);
            // Faint fill so the covered area reads at a glance.
            frame.fill_rectangle(
                rect_origin,
                rect_size,
                Color {
                    a: 0.10,
                    ..self.selected_color
                },
            );
            // Start-corner dot so the user can see the anchor.
            frame.fill(&canvas::Path::circle(p0, 3.0), preview_color);
            frame.stroke(
                &canvas::Path::rectangle(rect_origin, rect_size),
                canvas::Stroke::default()
                    .with_color(preview_color)
                    .with_width(stroke_px_at_zoom(SYMBOL_GRAPHIC_STROKE_PX_AT_100, scale)),
            );
        }
    }

    /// Two-click line placement preview.
    pub(in crate::library::editor::symbol::canvas) fn draw_line_preview(
        &self,
        frame: &mut canvas::Frame,
        state: &CanvasState,
    ) {
        let cam = self.camera;
        let scale = cam.scale;
        let ox = cam.offset.x;
        let oy = cam.offset.y;
        let w2s = |x: f64, y: f64| -> iced::Point {
            iced::Point::new(ox + (x as f32) * scale, oy - (y as f32) * scale)
        };
        if let (Some((fx, fy)), Some((cx, cy))) = (state.line_from, state.line_cursor) {
            let p0 = w2s(fx, fy);
            let p1 = w2s(cx, cy);
            let preview_color = Color {
                a: 0.55,
                ..self.selected_color
            };
            // Start-point dot so the user can see the anchor.
            frame.fill(&canvas::Path::circle(p0, 3.0), preview_color);
            frame.stroke(
                &canvas::Path::line(p0, p1),
                canvas::Stroke::default()
                    .with_color(preview_color)
                    .with_width(stroke_px_at_zoom(SYMBOL_GRAPHIC_STROKE_PX_AT_100, scale))
                    .with_line_cap(canvas::LineCap::Round),
            );
        }
    }

    /// Two-click circle placement preview.
    pub(in crate::library::editor::symbol::canvas) fn draw_circle_preview(
        &self,
        frame: &mut canvas::Frame,
        state: &CanvasState,
    ) {
        let cam = self.camera;
        let scale = cam.scale;
        let ox = cam.offset.x;
        let oy = cam.offset.y;
        let w2s = |x: f64, y: f64| -> iced::Point {
            iced::Point::new(ox + (x as f32) * scale, oy - (y as f32) * scale)
        };
        if let (Some((cx, cy)), Some((cur_x, cur_y))) = (state.circle_center, state.circle_cursor) {
            let center_p = w2s(cx, cy);
            let dx = cur_x - cx;
            let dy = cur_y - cy;
            let radius_world = (dx * dx + dy * dy).sqrt().max(0.1);
            let radius_screen = (radius_world as f32) * scale;
            let preview_color = Color {
                a: 0.55,
                ..self.selected_color
            };
            // Center dot.
            frame.fill(&canvas::Path::circle(center_p, 3.0), preview_color);
            // Radius line to the cursor.
            let cursor_p = w2s(cur_x, cur_y);
            frame.stroke(
                &canvas::Path::line(center_p, cursor_p),
                canvas::Stroke::default()
                    .with_color(preview_color)
                    .with_width(1.0),
            );
            // Circle outline at current radius.
            frame.stroke(
                &canvas::Path::circle(center_p, radius_screen),
                canvas::Stroke::default()
                    .with_color(preview_color)
                    .with_width(stroke_px_at_zoom(SYMBOL_GRAPHIC_STROKE_PX_AT_100, scale)),
            );
        }
    }

    /// Three-click arc placement preview.
    pub(in crate::library::editor::symbol::canvas) fn draw_arc_preview(
        &self,
        frame: &mut canvas::Frame,
        state: &CanvasState,
    ) {
        let cam = self.camera;
        let scale = cam.scale;
        let ox = cam.offset.x;
        let oy = cam.offset.y;
        let w2s = |x: f64, y: f64| -> iced::Point {
            iced::Point::new(ox + (x as f32) * scale, oy - (y as f32) * scale)
        };
        if let Some((cx, cy)) = state.arc_center {
            let center_p = w2s(cx, cy);
            let preview_color = Color {
                a: 0.55,
                ..self.selected_color
            };
            frame.fill(&canvas::Path::circle(center_p, 3.0), preview_color);
            if let Some((radius, start_deg)) = state.arc_radius_start {
                // Phase 2: radius and start angle are committed.
                let radius_screen = (radius as f32) * scale;
                // Faint full-circle ghost to show the radius.
                let faint = Color {
                    a: 0.18,
                    ..self.selected_color
                };
                frame.stroke(
                    &canvas::Path::circle(center_p, radius_screen),
                    canvas::Stroke::default().with_color(faint).with_width(1.0),
                );
                // Start-angle endpoint dot.
                let start_rad = start_deg.to_radians();
                let sp = w2s(cx + radius * start_rad.cos(), cy + radius * start_rad.sin());
                frame.fill(&canvas::Path::circle(sp, 3.0), preview_color);
                // Line from center to cursor (end-angle preview).
                if let Some((cur_x, cur_y)) = state.arc_cursor {
                    let cursor_p = w2s(cur_x, cur_y);
                    frame.stroke(
                        &canvas::Path::line(center_p, cursor_p),
                        canvas::Stroke::default()
                            .with_color(preview_color)
                            .with_width(1.0),
                    );
                    // Arc sweep from start to cursor angle.
                    // canvas::path::Arc lives in screen space (y-down), so we
                    // negate the world-space angles to compensate for the y-flip
                    // applied by w2s: screen_angle = -world_angle.
                    // Use the unwrapped end angle from state to avoid the ±180°
                    // discontinuity that raw atan2 would introduce.
                    let end_deg = state.arc_end_deg_unwrapped.unwrap_or_else(|| {
                        let dx = cur_x - cx;
                        let dy = cur_y - cy;
                        dy.atan2(dx).to_degrees()
                    });
                    let arc_path = canvas::Path::new(|builder| {
                        builder.arc(canvas::path::Arc {
                            center: center_p,
                            radius: radius_screen,
                            start_angle: iced::Radians(-(start_deg as f32).to_radians()),
                            end_angle: iced::Radians(-(end_deg as f32).to_radians()),
                        });
                    });
                    frame.stroke(
                        &arc_path,
                        canvas::Stroke::default()
                            .with_color(preview_color)
                            .with_width(stroke_px_at_zoom(SYMBOL_GRAPHIC_STROKE_PX_AT_100, scale)),
                    );
                }
            } else if let Some((cur_x, cur_y)) = state.arc_cursor {
                // Phase 1: just show the radius line to the cursor.
                let cursor_p = w2s(cur_x, cur_y);
                frame.stroke(
                    &canvas::Path::line(center_p, cursor_p),
                    canvas::Stroke::default()
                        .with_color(preview_color)
                        .with_width(1.0),
                );
            }
        }
    }
}
