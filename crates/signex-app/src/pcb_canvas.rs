use iced::event::Event;
use iced::mouse;
use iced::widget::canvas;
use iced::{Color, Rectangle, Renderer, Theme};

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
    pub render_snapshot: Option<signex_render::pcb::PcbRenderSnapshot>,
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
            render_snapshot: None,
            visible_grid_mm: 2.54,
        }
    }

    pub fn active_snapshot(&self) -> Option<&signex_render::pcb::PcbRenderSnapshot> {
        self.render_snapshot.as_ref()
    }

    pub fn set_render_snapshot(
        &mut self,
        render_snapshot: Option<signex_render::pcb::PcbRenderSnapshot>,
    ) {
        self.render_snapshot = render_snapshot;
    }

    pub fn clear_bg_cache(&mut self) {
        self.bg_cache.clear();
    }

    pub fn clear_content_cache(&mut self) {
        self.content_cache.clear();
    }

    pub fn fit_to_board(&mut self) {
        if let Some(snapshot) = self.active_snapshot()
            && let Some(bounds) = snapshot.content_bounds()
        {
            self.pending_fit.set(Some(Rectangle::new(
                iced::Point::new(bounds.min_x as f32, bounds.min_y as f32),
                iced::Size::new(bounds.width() as f32, bounds.height() as f32),
            )));
        }
    }

    pub fn set_theme_colors(&mut self, bg: Color, grid: Color) {
        self.theme_bg = bg;
        self.theme_grid = grid;
        self.bg_cache.clear();
    }
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

        let transform = signex_render::pcb::ScreenTransform {
            offset_x: state.camera.offset.x,
            offset_y: state.camera.offset.y,
            scale: state.camera.scale,
        };
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
            if let Some(snapshot) = self.active_snapshot() {
                signex_render::pcb::render_pcb(frame, snapshot, transform, &self.canvas_colors);
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
