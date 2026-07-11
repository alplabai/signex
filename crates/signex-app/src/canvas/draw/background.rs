use super::super::*;

impl SchematicCanvas {
    /// Layer 1 — background fill, paper rectangle + border, and grid dots.
    pub(in crate::canvas) fn draw_background(
        &self,
        state: &CanvasState,
        renderer: &Renderer,
        bounds: Rectangle,
    ) -> canvas::Geometry {
        self.bg_cache.draw(renderer, bounds.size(), |frame| {
            // Fill background
            frame.fill_rectangle(iced::Point::ORIGIN, bounds.size(), self.theme_bg);

            // Draw paper rectangle using active paper size.
            let paper_tl = state
                .camera
                .world_to_screen(iced::Point::new(0.0, 0.0), bounds);
            let paper_br = state.camera.world_to_screen(
                iced::Point::new(self.paper_width_mm, self.paper_height_mm),
                bounds,
            );
            let paper_w = paper_br.x - paper_tl.x;
            let paper_h = paper_br.y - paper_tl.y;

            if paper_w > 0.0 && paper_h > 0.0 {
                frame.fill_rectangle(
                    paper_tl,
                    iced::Size::new(paper_w, paper_h),
                    self.theme_paper,
                );

                // Paper border
                let border = canvas::Path::rectangle(paper_tl, iced::Size::new(paper_w, paper_h));
                frame.stroke(
                    &border,
                    canvas::Stroke::default()
                        .with_color(self.theme_grid)
                        .with_width(1.0),
                );
            }

            // Draw grid — use visible_grid_mm so snap and visual grid are independent
            if self.grid_visible {
                grid::draw_grid(
                    frame,
                    &state.camera,
                    self.visible_grid_mm as f32,
                    bounds,
                    self.theme_grid,
                    self.paper_width_mm,
                    self.paper_height_mm,
                );
            }
        })
    }
}
