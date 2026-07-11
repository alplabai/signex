//! Backdrop layers — background fill, the adaptive minor/major grid
//! (Dots / SmallCrosses / Lines styles), and the world-origin
//! crosshair. Drawn first (bottom of the z-stack). Extracted verbatim
//! from `Program::draw`.

use super::super::*;
use iced::Rectangle;
use iced::widget::canvas;

impl SymbolCanvas<'_> {
    /// Background fill for the whole viewport.
    pub(in crate::library::editor::symbol::canvas) fn draw_background(
        &self,
        frame: &mut canvas::Frame,
        bounds: Rectangle,
    ) {
        frame.fill_rectangle(iced::Point::ORIGIN, bounds.size(), self.bg_color);
    }

    /// Adaptive minor/major grid, honouring the View ▸ Grid style.
    pub(in crate::library::editor::symbol::canvas) fn draw_grid(
        &self,
        frame: &mut canvas::Frame,
        bounds: Rectangle,
    ) {
        let cam = self.camera;
        let scale = cam.scale;
        let ox = cam.offset.x;
        let oy = cam.offset.y;
        let w2s = |x: f64, y: f64| -> iced::Point {
            iced::Point::new(ox + (x as f32) * scale, oy - (y as f32) * scale)
        };
        // Grid — read spacing from the global panel_ctx so the
        // schematic + library editors share the View ▸ Grid
        // setting.
        let (min_x, min_y, max_x, max_y) = self.bbox();
        if self.grid_visible {
            let g = self.grid_size_mm.max(0.001);
            // Adaptive minor step: find the smallest 5^k * g whose
            // on-screen spacing is ≥ 6 px.  Mirrors draw_grid() in
            // canvas/grid.rs so zoom behaviour is identical between
            // the schematic and symbol editors.
            const MIN_PX: f32 = 6.0;
            let mut minor_mm = g as f32;
            for _ in 0..4 {
                if minor_mm * scale >= MIN_PX {
                    break;
                }
                minor_mm *= 5.0;
            }
            let minor_screen = minor_mm * scale;
            // Cross-fade the minor level in as zoom increases.
            let minor_alpha = ((minor_screen - MIN_PX) / MIN_PX).clamp(0.0, 1.0);
            if minor_alpha > 0.0 {
                let minor_world = minor_mm as f64;
                let major_mm = minor_mm * 5.0;
                let major_world = major_mm as f64;
                // Visible world bounds (y-up: vy0 = screen-bottom, vy1 = screen-top).
                let pad = 6.0 * minor_world;
                let (vx0, vy0) = world_unsnapped(self, 0.0, bounds.height, bounds);
                let (vx1, vy1) = world_unsnapped(self, bounds.width, 0.0, bounds);
                let world_x0 = (min_x - pad).min(vx0);
                let world_x1 = (max_x + pad).max(vx1);
                let world_y0 = (min_y - pad).min(vy0);
                let world_y1 = (max_y + pad).max(vy1);

                let cols = ((world_x1 - world_x0) / minor_world).abs() as i64 + 1;
                let rows = ((world_y1 - world_y0) / minor_world).abs() as i64 + 1;
                if cols * rows < 60_000 {
                    let dot_color = iced::Color {
                        a: self.grid_color.a * minor_alpha,
                        ..self.grid_color
                    };
                    let dot_radius = (minor_screen * 0.06).clamp(0.5, 1.6);
                    let cross_arm = (minor_screen * 0.18).clamp(1.5, 4.0);
                    let grid_style = crate::render_config::symbol_grid_style();
                    let minor_stroke = canvas::Stroke::default()
                        .with_color(dot_color)
                        .with_width(0.6);

                    // Precompute vertical span shared by both Lines and major.
                    let top_sy = w2s(0.0, world_y1).y.max(0.0);
                    let bot_sy = w2s(0.0, world_y0).y.min(bounds.height);
                    let left_sx = w2s(world_x0, 0.0).x.max(0.0);
                    let right_sx = w2s(world_x1, 0.0).x.min(bounds.width);

                    if matches!(grid_style, crate::render_config::GridStyle::Lines) {
                        // Lines style: full minor grid lines across the canvas.
                        let mut gx = (world_x0 / minor_world).floor() * minor_world;
                        while gx <= world_x1 {
                            let sx = w2s(gx, 0.0).x;
                            if sx >= 0.0 && sx <= bounds.width && top_sy < bot_sy {
                                frame.stroke(
                                    &canvas::Path::line(
                                        iced::Point::new(sx, top_sy),
                                        iced::Point::new(sx, bot_sy),
                                    ),
                                    minor_stroke,
                                );
                            }
                            gx += minor_world;
                        }
                        let mut gy = (world_y0 / minor_world).floor() * minor_world;
                        while gy <= world_y1 {
                            let sy = w2s(0.0, gy).y;
                            if sy >= 0.0 && sy <= bounds.height && left_sx < right_sx {
                                frame.stroke(
                                    &canvas::Path::line(
                                        iced::Point::new(left_sx, sy),
                                        iced::Point::new(right_sx, sy),
                                    ),
                                    minor_stroke,
                                );
                            }
                            gy += minor_world;
                        }
                    } else {
                        // Dots / SmallCrosses: per-point glyphs.
                        let mut gx = (world_x0 / minor_world).floor() * minor_world;
                        while gx <= world_x1 {
                            let mut gy = (world_y0 / minor_world).floor() * minor_world;
                            while gy <= world_y1 {
                                let p = w2s(gx, gy);
                                if p.x >= -cross_arm
                                    && p.x <= bounds.width + cross_arm
                                    && p.y >= -cross_arm
                                    && p.y <= bounds.height + cross_arm
                                {
                                    match grid_style {
                                        crate::render_config::GridStyle::Dots => {
                                            frame.fill(
                                                &canvas::Path::circle(p, dot_radius),
                                                dot_color,
                                            );
                                        }
                                        crate::render_config::GridStyle::SmallCrosses => {
                                            frame.stroke(
                                                &canvas::Path::line(
                                                    iced::Point::new(p.x - cross_arm, p.y),
                                                    iced::Point::new(p.x + cross_arm, p.y),
                                                ),
                                                minor_stroke,
                                            );
                                            frame.stroke(
                                                &canvas::Path::line(
                                                    iced::Point::new(p.x, p.y - cross_arm),
                                                    iced::Point::new(p.x, p.y + cross_arm),
                                                ),
                                                minor_stroke,
                                            );
                                        }
                                        crate::render_config::GridStyle::Lines => unreachable!(),
                                    }
                                }
                                gy += minor_world;
                            }
                            gx += minor_world;
                        }
                    }

                    // Major accent lines — always full lines regardless of
                    // the minor style, fading in/out with zoom so the grid
                    // never looks cluttered at extremes.
                    let major_screen = major_mm * scale;
                    let major_alpha_in = ((major_screen - 24.0) / 24.0).clamp(0.0, 1.0);
                    let major_alpha_out = ((400.0 - major_screen) / 200.0).clamp(0.0, 1.0);
                    let major_alpha = self.grid_color.a * 0.35 * major_alpha_in * major_alpha_out;
                    if major_alpha > 0.005 {
                        let major_color = iced::Color {
                            a: major_alpha,
                            ..self.grid_color
                        };
                        let major_stroke = canvas::Stroke::default()
                            .with_color(major_color)
                            .with_width(0.5);
                        let mut mx = (world_x0 / major_world).floor() * major_world;
                        while mx <= world_x1 {
                            let sx = w2s(mx, 0.0).x;
                            if sx >= 0.0 && sx <= bounds.width && top_sy < bot_sy {
                                frame.stroke(
                                    &canvas::Path::line(
                                        iced::Point::new(sx, top_sy),
                                        iced::Point::new(sx, bot_sy),
                                    ),
                                    major_stroke,
                                );
                            }
                            mx += major_world;
                        }
                        let mut my = (world_y0 / major_world).floor() * major_world;
                        while my <= world_y1 {
                            let sy = w2s(0.0, my).y;
                            if sy >= 0.0 && sy <= bounds.height && left_sx < right_sx {
                                frame.stroke(
                                    &canvas::Path::line(
                                        iced::Point::new(left_sx, sy),
                                        iced::Point::new(right_sx, sy),
                                    ),
                                    major_stroke,
                                );
                            }
                            my += major_world;
                        }
                    }
                }
            }
        }
    }

    /// Crosshair + dot marking world (0, 0).
    pub(in crate::library::editor::symbol::canvas) fn draw_origin_marker(
        &self,
        frame: &mut canvas::Frame,
        bounds: Rectangle,
    ) {
        let cam = self.camera;
        let scale = cam.scale;
        let ox = cam.offset.x;
        let oy = cam.offset.y;
        let w2s = |x: f64, y: f64| -> iced::Point {
            iced::Point::new(ox + (x as f32) * scale, oy - (y as f32) * scale)
        };
        // Origin marker at world (0, 0) — no default box/pin when a
        // symbol is created, so this gives a stable visual anchor.
        let origin = w2s(0.0, 0.0);
        let marker_half = text_size_px_from_mm(ORIGIN_MARKER_MM, scale).clamp(4.0, 18.0);
        if origin.x >= -marker_half
            && origin.x <= bounds.width + marker_half
            && origin.y >= -marker_half
            && origin.y <= bounds.height + marker_half
        {
            let h = canvas::Path::line(
                iced::Point::new(origin.x - marker_half, origin.y),
                iced::Point::new(origin.x + marker_half, origin.y),
            );
            let v = canvas::Path::line(
                iced::Point::new(origin.x, origin.y - marker_half),
                iced::Point::new(origin.x, origin.y + marker_half),
            );
            frame.stroke(
                &h,
                canvas::Stroke::default()
                    .with_color(self.axis_color)
                    .with_width(stroke_px_at_zoom(SYMBOL_AXIS_STROKE_PX_AT_100, scale)),
            );
            frame.stroke(
                &v,
                canvas::Stroke::default()
                    .with_color(self.axis_color)
                    .with_width(stroke_px_at_zoom(SYMBOL_AXIS_STROKE_PX_AT_100, scale)),
            );
            frame.fill(
                &canvas::Path::circle(origin, (marker_half * 0.14).clamp(1.0, 2.5)),
                self.axis_color,
            );
        }
    }
}
