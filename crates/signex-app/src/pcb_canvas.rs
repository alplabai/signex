use iced::event::Event;
use iced::mouse;
use iced::widget::canvas;
use iced::{Color, Rectangle, Renderer, Theme};
use signex_gfx::primitive::circle::Circle as GfxCircle;
use signex_gfx::primitive::line::LineSegment;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::scene::{DirtyFlags, Scene};
use signex_renderer::pcb::{PcbRenderer, PcbSnapshot};
use signex_renderer::schematic::ViewRenderer;
use signex_renderer::theme::ResolvedTheme;

use crate::app::Message;
use crate::canvas::{Camera, CanvasEvent};

#[derive(Debug, Default)]
pub struct PcbCanvasState {
    pub camera: Camera,
    panning: bool,
    last_pan_pos: Option<iced::Point>,
    pub pending_fit: Option<Rectangle>,
}

pub struct PcbCanvas {
    pub bg_cache: canvas::Cache,
    pub content_cache: canvas::Cache,
    pub content_cache_camera: std::cell::Cell<(f32, f32, f32)>,
    pub pending_fit: std::cell::Cell<Option<Rectangle>>,
    pub grid_visible: bool,
    pub theme_bg: Color,
    pub theme_grid: Color,
    pub canvas_colors: signex_types::theme::CanvasColors,
    pub renderer_snapshot: Option<PcbSnapshot>,
    pub visible_grid_mm: f64,
}

impl PcbCanvas {
    pub fn new() -> Self {
        let colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::Signex);
        Self {
            bg_cache: canvas::Cache::default(),
            content_cache: canvas::Cache::default(),
            content_cache_camera: std::cell::Cell::new((0.0, 0.0, 1.0)),
            pending_fit: std::cell::Cell::new(None),
            grid_visible: true,
            theme_bg: signex_render::colors::to_iced(&colors.background),
            theme_grid: signex_render::colors::to_iced(&colors.grid),
            canvas_colors: colors,
            renderer_snapshot: None,
            visible_grid_mm: 2.54,
        }
    }

    pub fn active_renderer_snapshot(&self) -> Option<&PcbSnapshot> {
        self.renderer_snapshot.as_ref()
    }

    pub fn set_renderer_snapshot(&mut self, renderer_snapshot: Option<PcbSnapshot>) {
        self.renderer_snapshot = renderer_snapshot;
    }

    pub fn clear_bg_cache(&mut self) {
        self.bg_cache.clear();
    }

    pub fn clear_content_cache(&mut self) {
        self.content_cache.clear();
    }

    pub fn fit_to_board(&mut self) {
        if let Some(snapshot) = self.active_renderer_snapshot()
            && let Some(bounds) = renderer_snapshot_bounds(snapshot)
        {
            self.pending_fit.set(Some(Rectangle::new(
                iced::Point::new(bounds.x, bounds.y),
                iced::Size::new(bounds.width, bounds.height),
            )));
        }
    }

    pub fn set_theme_colors(&mut self, bg: Color, grid: Color) {
        self.theme_bg = bg;
        self.theme_grid = grid;
        self.bg_cache.clear();
    }
}

fn color_from_rgba(rgba: [f32; 4]) -> Color {
    Color::from_rgba(rgba[0], rgba[1], rgba[2], rgba[3])
}

fn world_to_screen(camera: &Camera, bounds: Rectangle, point: [f32; 2]) -> iced::Point {
    camera.world_to_screen(iced::Point::new(point[0], point[1]), bounds)
}

fn include_world_point(bounds: &mut Option<(f32, f32, f32, f32)>, x: f32, y: f32) {
    if let Some((min_x, min_y, max_x, max_y)) = bounds.as_mut() {
        *min_x = (*min_x).min(x);
        *min_y = (*min_y).min(y);
        *max_x = (*max_x).max(x);
        *max_y = (*max_y).max(y);
    } else {
        *bounds = Some((x, y, x, y));
    }
}

fn include_world_span(bounds: &mut Option<(f32, f32, f32, f32)>, x: f32, y: f32, radius: f32) {
    let r = radius.max(0.0);
    include_world_point(bounds, x - r, y - r);
    include_world_point(bounds, x + r, y + r);
}

fn renderer_snapshot_bounds(snapshot: &PcbSnapshot) -> Option<Rectangle> {
    let mut bounds: Option<(f32, f32, f32, f32)> = None;

    for trace in &snapshot.traces {
        let half_w = (trace.width_mm * 0.5).max(0.02);
        include_world_span(&mut bounds, trace.p0[0], trace.p0[1], half_w);
        include_world_span(&mut bounds, trace.p1[0], trace.p1[1], half_w);
    }

    for via in &snapshot.vias {
        include_world_span(
            &mut bounds,
            via.center[0],
            via.center[1],
            (via.diameter_mm * 0.5).max(0.02),
        );
    }

    for pad in &snapshot.pads {
        include_world_span(
            &mut bounds,
            pad.center[0],
            pad.center[1],
            (pad.size_mm[0].max(pad.size_mm[1]) * 0.5).max(0.02),
        );
    }

    for zone in &snapshot.zones {
        for vertex in &zone.vertices {
            include_world_point(&mut bounds, vertex[0], vertex[1]);
        }
    }

    for rule_area in &snapshot.rule_areas {
        for vertex in &rule_area.vertices {
            include_world_point(&mut bounds, vertex[0], vertex[1]);
        }
    }

    for line in &snapshot.ratsnest_lines {
        include_world_point(&mut bounds, line.p0[0], line.p0[1]);
        include_world_point(&mut bounds, line.p1[0], line.p1[1]);
    }

    for marker in &snapshot.drc_markers {
        include_world_span(
            &mut bounds,
            marker.center[0],
            marker.center[1],
            marker.radius_mm.max(0.02),
        );
    }

    bounds.map(|(min_x, min_y, max_x, max_y)| {
        Rectangle::new(
            iced::Point::new(min_x, min_y),
            iced::Size::new((max_x - min_x).max(0.1), (max_y - min_y).max(0.1)),
        )
    })
}

fn draw_dashed_line(
    frame: &mut canvas::Frame,
    p0: iced::Point,
    p1: iced::Point,
    width: f32,
    color: Color,
) {
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    let length = (dx * dx + dy * dy).sqrt();
    if length <= 0.0001 {
        return;
    }

    let dash = 8.0;
    let gap = 5.0;
    let ux = dx / length;
    let uy = dy / length;
    let mut dist = 0.0;

    while dist < length {
        let seg_end = (dist + dash).min(length);
        let sp = iced::Point::new(p0.x + ux * dist, p0.y + uy * dist);
        let ep = iced::Point::new(p0.x + ux * seg_end, p0.y + uy * seg_end);
        let path = canvas::Path::line(sp, ep);
        frame.stroke(
            &path,
            canvas::Stroke::default()
                .with_width(width)
                .with_color(color),
        );
        dist += dash + gap;
    }
}

fn draw_lines(
    frame: &mut canvas::Frame,
    lines: &[LineSegment],
    camera: &Camera,
    bounds: Rectangle,
) {
    for line in lines {
        let p0 = world_to_screen(camera, bounds, line.p0);
        let p1 = world_to_screen(camera, bounds, line.p1);
        let width = (line.width * camera.scale).max(0.5);
        let color = color_from_rgba(line.color);

        if (line.style & 1) == 1 {
            draw_dashed_line(frame, p0, p1, width, color);
        } else {
            let path = canvas::Path::line(p0, p1);
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_width(width)
                    .with_color(color),
            );
        }
    }
}

fn draw_circles(
    frame: &mut canvas::Frame,
    circles: &[GfxCircle],
    camera: &Camera,
    bounds: Rectangle,
) {
    for circle in circles {
        let center = world_to_screen(camera, bounds, circle.center);
        let radius = (circle.radius * camera.scale).max(0.5);
        let stroke_width = (circle.stroke_width * camera.scale).max(0.5);
        let color = color_from_rgba(circle.color);
        let path = canvas::Path::circle(center, radius);

        if circle.stroke_width <= 0.0 {
            frame.fill(&path, color);
        } else {
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_width(stroke_width)
                    .with_color(color),
            );
        }
    }
}

fn draw_polygons(
    frame: &mut canvas::Frame,
    polygons: &[GpuPolygon],
    camera: &Camera,
    bounds: Rectangle,
) {
    for polygon in polygons {
        if polygon.vertices.len() < 3 {
            continue;
        }

        let points: Vec<iced::Point> = polygon
            .vertices
            .iter()
            .map(|vertex| world_to_screen(camera, bounds, *vertex))
            .collect();

        let path = canvas::Path::new(|builder| {
            builder.move_to(points[0]);
            for point in &points[1..] {
                builder.line_to(*point);
            }
            builder.close();
        });

        frame.fill(&path, color_from_rgba(polygon.fill_color));

        if let Some(stroke_color) = polygon.stroke_color {
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_width((polygon.stroke_width * camera.scale).max(0.5))
                    .with_color(color_from_rgba(stroke_color)),
            );
        }
    }
}

fn draw_scene(frame: &mut canvas::Frame, scene: &Scene, camera: &Camera, bounds: Rectangle) {
    draw_lines(frame, &scene.lines, camera, bounds);
    draw_circles(frame, &scene.circles, camera, bounds);
    draw_polygons(frame, &scene.polygons, camera, bounds);

    draw_lines(frame, &scene.overlay_lines, camera, bounds);
    draw_circles(frame, &scene.overlay_circles, camera, bounds);
    draw_polygons(frame, &scene.overlay_polygons, camera, bounds);
}

impl canvas::Program<Message> for PcbCanvas {
    type State = PcbCanvasState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if let Some(target) = self.pending_fit.take() {
            state.pending_fit = Some(target);
        }

        if let Some(target) = state.pending_fit.take() {
            state.camera.fit_rect(target, bounds);
            return Some(canvas::Action::publish(Message::CanvasEvent(
                CanvasEvent::CursorMoved,
            )));
        }

        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let scroll_y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / 50.0,
                };
                if let Some(cursor_pos) = cursor.position_in(bounds)
                    && state.camera.zoom_at(cursor_pos, scroll_y, bounds)
                {
                    return Some(
                        canvas::Action::publish(Message::CanvasEvent(CanvasEvent::CursorMoved))
                            .and_capture(),
                    );
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(button)) => {
                if matches!(button, mouse::Button::Right | mouse::Button::Middle) {
                    state.panning = true;
                    state.last_pan_pos = cursor.position_in(bounds);
                    return Some(canvas::Action::capture());
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(button)) => {
                if matches!(button, mouse::Button::Right | mouse::Button::Middle) {
                    state.panning = false;
                    state.last_pan_pos = None;
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(cursor_pos) = cursor.position_in(bounds) {
                    if state.panning
                        && let Some(last_pan_pos) = state.last_pan_pos
                    {
                        state
                            .camera
                            .pan(cursor_pos.x - last_pan_pos.x, cursor_pos.y - last_pan_pos.y);
                        state.last_pan_pos = Some(cursor_pos);
                        return Some(
                            canvas::Action::publish(Message::CanvasEvent(CanvasEvent::CursorMoved))
                                .and_capture(),
                        );
                    }

                    let world = state.camera.screen_to_world(cursor_pos, bounds);
                    return Some(canvas::Action::publish(Message::CanvasEvent(
                        CanvasEvent::CursorAt {
                            x: world.x,
                            y: world.y,
                            zoom_pct: state.camera.zoom_percent(),
                        },
                    )));
                }
            }
            _ => {}
        }

        None
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let bg = self.bg_cache.draw(renderer, bounds.size(), |frame| {
            frame.fill_rectangle(iced::Point::ORIGIN, bounds.size(), self.theme_bg);

            if self.grid_visible {
                let step = (self.visible_grid_mm as f32 * state.camera.scale).max(8.0);
                let mut x = state.camera.offset.x.rem_euclid(step) - step;
                while x <= bounds.width + step {
                    let path = canvas::Path::line(
                        iced::Point::new(x, 0.0),
                        iced::Point::new(x, bounds.height),
                    );
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_color(Color {
                                a: 0.18,
                                ..self.theme_grid
                            })
                            .with_width(0.5),
                    );
                    x += step;
                }

                let mut y = state.camera.offset.y.rem_euclid(step) - step;
                while y <= bounds.height + step {
                    let path = canvas::Path::line(
                        iced::Point::new(0.0, y),
                        iced::Point::new(bounds.width, y),
                    );
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_color(Color {
                                a: 0.18,
                                ..self.theme_grid
                            })
                            .with_width(0.5),
                    );
                    y += step;
                }
            }
        });

        let (cached_offset_x, cached_offset_y, cached_scale) = self.content_cache_camera.get();
        let camera_matches_cache = (cached_offset_x - state.camera.offset.x).abs() < 0.01
            && (cached_offset_y - state.camera.offset.y).abs() < 0.01
            && (cached_scale - state.camera.scale).abs() < 0.0001;
        if !camera_matches_cache {
            self.content_cache.clear();
        }
        let content = self.content_cache.draw(renderer, bounds.size(), |frame| {
            self.content_cache_camera.set((
                state.camera.offset.x,
                state.camera.offset.y,
                state.camera.scale,
            ));
            if let Some(snapshot) = self.active_renderer_snapshot() {
                let mut scene = Scene::default();
                let theme = ResolvedTheme::from_canvas_colors(self.canvas_colors);
                PcbRenderer::build_scene(
                    snapshot,
                    &theme,
                    DirtyFlags::LINES
                        | DirtyFlags::CIRCLES
                        | DirtyFlags::POLYGONS
                        | DirtyFlags::OVERLAY,
                    &mut scene,
                );
                draw_scene(frame, &scene, &state.camera, bounds);
            }
        });

        vec![bg, content]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.panning {
            return mouse::Interaction::Grabbing;
        }
        if cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}
