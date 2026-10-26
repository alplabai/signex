use signex_output::template::{FontStyle, FramePrimitive, FrameTextAnchor};

use super::super::*;

impl SchematicCanvas<'_> {
    /// Layer 1 — background fill, paper rectangle + border, grid dots, and the
    /// sheet frame + title block.
    pub(in crate::canvas) fn draw_background(
        &self,
        renderer: &Renderer,
        bounds: Rectangle,
    ) -> canvas::Geometry {
        let cam = self.camera();
        self.bg_cache.draw(renderer, bounds.size(), |frame| {
            // Fill background
            frame.fill_rectangle(iced::Point::ORIGIN, bounds.size(), self.prefs.theme_bg);

            // Draw paper rectangle using active paper size.
            let paper_tl = cam.world_to_screen(iced::Point::new(0.0, 0.0), bounds);
            let paper_br = cam.world_to_screen(
                iced::Point::new(self.paper_width_mm, self.paper_height_mm),
                bounds,
            );
            let paper_w = paper_br.x - paper_tl.x;
            let paper_h = paper_br.y - paper_tl.y;

            if paper_w > 0.0 && paper_h > 0.0 {
                frame.fill_rectangle(
                    paper_tl,
                    iced::Size::new(paper_w, paper_h),
                    self.prefs.theme_paper,
                );

                // Paper border
                let border = canvas::Path::rectangle(paper_tl, iced::Size::new(paper_w, paper_h));
                frame.stroke(
                    &border,
                    canvas::Stroke::default()
                        .with_color(self.prefs.theme_grid)
                        .with_width(1.0),
                );
            }

            // Draw grid — use visible_grid_mm so snap and visual grid are independent
            if self.prefs.grid_visible {
                grid::draw_grid(
                    frame,
                    &cam,
                    self.prefs.visible_grid_mm as f32,
                    bounds,
                    self.prefs.theme_grid,
                    self.paper_width_mm,
                    self.paper_height_mm,
                    self.prefs.grid_style,
                );
            }

            let frame_color = crate::render_config::to_iced(&self.prefs.canvas_colors.body);
            draw_sheet_frame(frame, &cam, bounds, &self.sheet_frame, frame_color);
        })
    }
}

/// Draw the lowered sheet frame (#1079): the same primitives the PDF draws,
/// mapped from sheet mm to screen pixels.
fn draw_sheet_frame(
    frame: &mut canvas::Frame,
    cam: &Camera,
    bounds: Rectangle,
    primitives: &[FramePrimitive],
    color: Color,
) {
    let at = |x: f64, y: f64| cam.world_to_screen(iced::Point::new(x as f32, y as f32), bounds);
    // Hairlines stay visible when zoomed out and thin when zoomed in.
    let stroke = |width_mm: f64| {
        canvas::Stroke::default()
            .with_color(color)
            .with_width((width_mm as f32 * cam.scale).clamp(1.0, 3.0))
    };
    for primitive in primitives {
        match primitive {
            FramePrimitive::Line { from, to, width_mm } => {
                frame.stroke(
                    &canvas::Path::line(at(from.0, from.1), at(to.0, to.1)),
                    stroke(*width_mm),
                );
            }
            FramePrimitive::Rect {
                x,
                y,
                w,
                h,
                width_mm,
            } => {
                let top_left = at(*x, *y);
                let bottom_right = at(x + w, y + h);
                let size =
                    iced::Size::new(bottom_right.x - top_left.x, bottom_right.y - top_left.y);
                frame.stroke(&canvas::Path::rectangle(top_left, size), stroke(*width_mm));
            }
            FramePrimitive::Text {
                x,
                y,
                size_mm,
                style,
                anchor,
                text,
            } => {
                let size_px = *size_mm as f32 * cam.scale;
                // Too small to read: skip rather than draw a smear.
                if size_px < 3.0 {
                    continue;
                }
                let (align_x, align_y) = match anchor {
                    FrameTextAnchor::TopLeft => (
                        iced::alignment::Horizontal::Left,
                        iced::alignment::Vertical::Top,
                    ),
                    FrameTextAnchor::Center => (
                        iced::alignment::Horizontal::Center,
                        iced::alignment::Vertical::Center,
                    ),
                };
                let weight = match style {
                    FontStyle::Bold | FontStyle::BoldItalic => iced::font::Weight::Bold,
                    FontStyle::Normal | FontStyle::Italic => iced::font::Weight::Normal,
                };
                frame.fill_text(canvas::Text {
                    content: text.clone(),
                    position: at(*x, *y),
                    color,
                    size: iced::Pixels(size_px),
                    font: iced::Font {
                        weight,
                        ..crate::render_config::IOSEVKA
                    },
                    align_x: align_x.into(),
                    align_y,
                    ..canvas::Text::default()
                });
            }
        }
    }
}
