use super::super::*;

impl SchematicCanvas {
    /// Two-click shape rubber-band (line / rect / circle).
    pub(in crate::canvas) fn draw_shape_preview(
        &self,
        frame: &mut canvas::Frame,
        state: &CanvasState,
        bounds: Rectangle,
        cursor_pos: iced::Point,
    ) {
        // Two-click shape rubber-band — line from anchor to
        // cursor, or rect / circle sized by the cursor offset.
        // Commits on the second click via the tool's branch
        // in CanvasEvent::Clicked.
        if let Some((anchor, kind)) = self.shape_anchor {
            let cursor_world = state.camera.screen_to_world(cursor_pos, bounds);
            let (snap_x, snap_y) = if self.snap_enabled && self.snap_grid_mm > 0.0 {
                let g = self.snap_grid_mm;
                (
                    (cursor_world.x as f64 / g).round() * g,
                    (cursor_world.y as f64 / g).round() * g,
                )
            } else {
                (cursor_world.x as f64, cursor_world.y as f64)
            };
            let p_a = state
                .camera
                .world_to_screen(iced::Point::new(anchor.x as f32, anchor.y as f32), bounds);
            let p_b = state
                .camera
                .world_to_screen(iced::Point::new(snap_x as f32, snap_y as f32), bounds);
            let accent = Color::from_rgb(0.94, 0.74, 0.28);
            let stroke = canvas::Stroke::default().with_color(accent).with_width(1.5);
            match kind {
                crate::canvas::ShapePreviewKind::Line => {
                    frame.stroke(&canvas::Path::line(p_a, p_b), stroke);
                }
                crate::canvas::ShapePreviewKind::Rect => {
                    let x0 = p_a.x.min(p_b.x);
                    let y0 = p_a.y.min(p_b.y);
                    let w = (p_a.x - p_b.x).abs();
                    let h = (p_a.y - p_b.y).abs();
                    frame.stroke(
                        &canvas::Path::rectangle(
                            iced::Point::new(x0, y0),
                            iced::Size::new(w.max(0.1), h.max(0.1)),
                        ),
                        stroke,
                    );
                }
                crate::canvas::ShapePreviewKind::Circle => {
                    let dx = p_b.x - p_a.x;
                    let dy = p_b.y - p_a.y;
                    let r = (dx * dx + dy * dy).sqrt().max(0.5);
                    frame.stroke(&canvas::Path::circle(p_a, r), stroke);
                    // Small center dot for Altium-style feedback.
                    frame.fill(&canvas::Path::circle(p_a, 2.0), accent);
                }
            }
        }
    }

    /// Polyline-in-progress preview.
    pub(in crate::canvas) fn draw_polyline_preview(
        &self,
        frame: &mut canvas::Frame,
        state: &CanvasState,
        bounds: Rectangle,
        cursor_pos: iced::Point,
    ) {
        // Polyline-in-progress preview — solid segments between
        // committed vertices plus a dashed rubber-band to the
        // snapped cursor. Commits on Enter or double-click.
        if !self.polyline_points.is_empty() {
            let accent = Color::from_rgb(0.94, 0.74, 0.28);
            let stroke = canvas::Stroke::default().with_color(accent).with_width(1.5);
            for pair in self.polyline_points.windows(2) {
                let p1 = state
                    .camera
                    .world_to_screen(iced::Point::new(pair[0].x as f32, pair[0].y as f32), bounds);
                let p2 = state
                    .camera
                    .world_to_screen(iced::Point::new(pair[1].x as f32, pair[1].y as f32), bounds);
                frame.stroke(&canvas::Path::line(p1, p2), stroke);
            }
            if let Some(last) = self.polyline_points.last() {
                let cursor_world = state.camera.screen_to_world(cursor_pos, bounds);
                let (snap_x, snap_y) = if self.snap_enabled && self.snap_grid_mm > 0.0 {
                    let g = self.snap_grid_mm;
                    (
                        (cursor_world.x as f64 / g).round() * g,
                        (cursor_world.y as f64 / g).round() * g,
                    )
                } else {
                    (cursor_world.x as f64, cursor_world.y as f64)
                };
                let p1 = state
                    .camera
                    .world_to_screen(iced::Point::new(last.x as f32, last.y as f32), bounds);
                let p2 = state
                    .camera
                    .world_to_screen(iced::Point::new(snap_x as f32, snap_y as f32), bounds);
                let dashed = canvas::Stroke::default()
                    .with_color(Color { a: 0.6, ..accent })
                    .with_width(1.0);
                frame.stroke(&canvas::Path::line(p1, p2), dashed);
            }
        }
    }

    /// Arc-in-progress preview.
    pub(in crate::canvas) fn draw_arc_preview(
        &self,
        frame: &mut canvas::Frame,
        state: &CanvasState,
        bounds: Rectangle,
        cursor_pos: iced::Point,
    ) {
        // Arc-in-progress preview — draw committed spans
        // between consecutive clicks. With 0 clicks: nothing.
        // 1 click: a dashed line to the cursor (start → current).
        // 2 clicks: a dashed 3-point curve (start → mid → cursor).
        if !self.arc_points.is_empty() {
            let accent = Color::from_rgb(0.94, 0.74, 0.28);
            let cursor_world = state.camera.screen_to_world(cursor_pos, bounds);
            let (snap_x, snap_y) = if self.snap_enabled && self.snap_grid_mm > 0.0 {
                let g = self.snap_grid_mm;
                (
                    (cursor_world.x as f64 / g).round() * g,
                    (cursor_world.y as f64 / g).round() * g,
                )
            } else {
                (cursor_world.x as f64, cursor_world.y as f64)
            };
            let dashed = canvas::Stroke::default()
                .with_color(Color { a: 0.6, ..accent })
                .with_width(1.0);
            // Draw committed anchors.
            for p in &self.arc_points {
                let sp = state
                    .camera
                    .world_to_screen(iced::Point::new(p.x as f32, p.y as f32), bounds);
                let ring = canvas::Path::circle(sp, 4.0);
                frame.stroke(
                    &ring,
                    canvas::Stroke::default().with_color(accent).with_width(1.5),
                );
            }
            // Rubber-band from last anchor to cursor.
            if let Some(last) = self.arc_points.last() {
                let p1 = state
                    .camera
                    .world_to_screen(iced::Point::new(last.x as f32, last.y as f32), bounds);
                let p2 = state
                    .camera
                    .world_to_screen(iced::Point::new(snap_x as f32, snap_y as f32), bounds);
                frame.stroke(&canvas::Path::line(p1, p2), dashed);
            }
        }
    }

    /// Lasso-in-progress preview.
    pub(in crate::canvas) fn draw_lasso_preview(
        &self,
        frame: &mut canvas::Frame,
        state: &CanvasState,
        bounds: Rectangle,
        cursor_pos: iced::Point,
    ) {
        // Lasso-in-progress preview — solid segments between
        // vertices plus a rubber-band dashed line from the
        // last vertex to the cursor. Same colour as the
        // selection accent so it reads as "selection in
        // progress".
        if let Some(lasso) = &self.lasso_polygon
            && !lasso.is_empty()
        {
            // Use the selection overlay colour so lasso
            // reads as "selection in progress" — same as
            // the box-select rubber-band.
            let accent = Color::from_rgb(0.24, 0.62, 0.97);
            let stroke = canvas::Stroke::default().with_color(accent).with_width(1.5);
            // Segments between committed vertices.
            for pair in lasso.windows(2) {
                let p1 = state
                    .camera
                    .world_to_screen(iced::Point::new(pair[0].x as f32, pair[0].y as f32), bounds);
                let p2 = state
                    .camera
                    .world_to_screen(iced::Point::new(pair[1].x as f32, pair[1].y as f32), bounds);
                frame.stroke(&canvas::Path::line(p1, p2), stroke);
            }
            // Rubber-band from last vertex → cursor (snapped).
            if let Some(last) = lasso.last() {
                let cursor_world = state.camera.screen_to_world(cursor_pos, bounds);
                let (snap_x, snap_y) = if self.snap_enabled && self.snap_grid_mm > 0.0 {
                    let g = self.snap_grid_mm;
                    (
                        (cursor_world.x as f64 / g).round() * g,
                        (cursor_world.y as f64 / g).round() * g,
                    )
                } else {
                    (cursor_world.x as f64, cursor_world.y as f64)
                };
                let p1 = state
                    .camera
                    .world_to_screen(iced::Point::new(last.x as f32, last.y as f32), bounds);
                let p2 = state
                    .camera
                    .world_to_screen(iced::Point::new(snap_x as f32, snap_y as f32), bounds);
                let dashed = canvas::Stroke::default()
                    .with_color(Color { a: 0.6, ..accent })
                    .with_width(1.0);
                frame.stroke(&canvas::Path::line(p1, p2), dashed);
            }
            // Small circle at the first vertex to hint
            // "click here to close the polygon".
            if lasso.len() >= 3 {
                let first = lasso[0];
                let p = state
                    .camera
                    .world_to_screen(iced::Point::new(first.x as f32, first.y as f32), bounds);
                let ring = canvas::Path::circle(p, 5.0);
                frame.stroke(
                    &ring,
                    canvas::Stroke::default().with_color(accent).with_width(1.5),
                );
            }
        }
    }

    /// Wire-in-progress rubber-band, constrained by the active draw mode.
    pub(in crate::canvas) fn draw_wire_preview(
        &self,
        frame: &mut canvas::Frame,
        state: &CanvasState,
        bounds: Rectangle,
        cursor_pos: iced::Point,
    ) {
        // Wire-in-progress rubber-band preview
        if self.drawing_mode && !self.wire_preview.is_empty() {
            let wire_color = self.canvas_colors.wire;
            let wire_color_iced = crate::render_config::to_iced(&wire_color);
            // Match the placed-wire stroke width (0.15 mm in world),
            // scaled by camera. Previously fixed 1.5 px which looked
            // thin at higher zooms.
            let placed_width = (state.camera.scale * 0.15).max(1.0);
            let preview_stroke = canvas::Stroke::default()
                .with_color(wire_color_iced)
                .with_width(placed_width);

            // Draw placed segments
            for pair in self.wire_preview.windows(2) {
                let p1 = state
                    .camera
                    .world_to_screen(iced::Point::new(pair[0].x as f32, pair[0].y as f32), bounds);
                let p2 = state
                    .camera
                    .world_to_screen(iced::Point::new(pair[1].x as f32, pair[1].y as f32), bounds);
                let seg = canvas::Path::line(p1, p2);
                frame.stroke(&seg, preview_stroke);
            }

            // Rubber-band from last point to cursor (constrained by draw mode)
            if let Some(last) = self.wire_preview.last() {
                let cursor_world = state.camera.screen_to_world(cursor_pos, bounds);
                // Snap cursor to grid so the rubber-band preview matches what will be placed
                let (snap_x, snap_y) = if self.snap_enabled && self.snap_grid_mm > 0.0 {
                    let g = self.snap_grid_mm;
                    (
                        (cursor_world.x as f64 / g).round() * g,
                        (cursor_world.y as f64 / g).round() * g,
                    )
                } else {
                    (cursor_world.x as f64, cursor_world.y as f64)
                };
                let start = signex_types::schematic::Point::new(last.x, last.y);
                let end = signex_types::schematic::Point::new(snap_x, snap_y);
                let rubber_stroke = canvas::Stroke::default()
                    .with_color(Color {
                        a: 0.7,
                        ..wire_color_iced
                    })
                    .with_width(placed_width);

                // Compute constrained segments based on draw mode
                let segments = match self.draw_mode {
                    crate::app::DrawMode::FreeAngle => {
                        vec![(start, end)]
                    }
                    crate::app::DrawMode::Ortho90 => {
                        let dx = end.x - start.x;
                        let dy = end.y - start.y;
                        if dx.abs() < 0.01 || dy.abs() < 0.01 {
                            vec![(start, end)]
                        } else {
                            let corner = signex_types::schematic::Point::new(end.x, start.y);
                            vec![(start, corner), (corner, end)]
                        }
                    }
                    crate::app::DrawMode::Angle45 => {
                        let dx = end.x - start.x;
                        let dy = end.y - start.y;
                        let adx = dx.abs();
                        let ady = dy.abs();
                        if adx < 0.01 || ady < 0.01 {
                            vec![(start, end)]
                        } else if (adx - ady).abs() < adx * 0.4 {
                            let d = adx.min(ady);
                            let sx = if dx > 0.0 { 1.0 } else { -1.0 };
                            let sy = if dy > 0.0 { 1.0 } else { -1.0 };
                            let diag_end = signex_types::schematic::Point::new(
                                start.x + d * sx,
                                start.y + d * sy,
                            );
                            if adx > ady {
                                vec![
                                    (start, diag_end),
                                    (
                                        diag_end,
                                        signex_types::schematic::Point::new(end.x, diag_end.y),
                                    ),
                                ]
                            } else {
                                vec![
                                    (start, diag_end),
                                    (
                                        diag_end,
                                        signex_types::schematic::Point::new(diag_end.x, end.y),
                                    ),
                                ]
                            }
                        } else {
                            let corner = signex_types::schematic::Point::new(end.x, start.y);
                            vec![(start, corner), (corner, end)]
                        }
                    }
                };

                for (p1, p2) in &segments {
                    let s1 = state
                        .camera
                        .world_to_screen(iced::Point::new(p1.x as f32, p1.y as f32), bounds);
                    let s2 = state
                        .camera
                        .world_to_screen(iced::Point::new(p2.x as f32, p2.y as f32), bounds);
                    frame.stroke(&canvas::Path::line(s1, s2), rubber_stroke);
                }
            }
        }
    }
}
