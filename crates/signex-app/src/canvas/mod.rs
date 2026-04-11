//! Schematic/PCB canvas — wgpu rendering with Altium-style pan/zoom/grid.
//!
//! Uses `iced::widget::canvas::Program` with a 3-layer cache:
//! - background: grid, sheet border (cleared on theme/grid/zoom change)
//! - content: schematic elements (cleared on document edit)
//! - overlay: selection, cursor, wire-in-progress (cleared every frame)

mod camera;
pub mod grid;

use iced::mouse;
use iced::widget::canvas;
use iced::event::Event;
use iced::{Color, Rectangle, Renderer, Theme};

pub use camera::Camera;
pub use grid::GridState;

use crate::app::Message;

// ─── Canvas State (per-canvas mutable state) ──────────────────

#[derive(Debug)]
pub struct CanvasState {
    pub camera: Camera,
    pub grid: GridState,
    /// Is the user currently panning (right-click or middle-click drag)?
    panning: bool,
    /// Last cursor position during a pan (in screen pixels).
    last_pan_pos: Option<iced::Point>,
    /// Pending fit target — consumed on next update.
    pub pending_fit: Option<Rectangle>,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            camera: Camera::default(),
            grid: GridState::default(),
            panning: false,
            last_pan_pos: None,
            pending_fit: None,
        }
    }
}

// ─── SchematicCanvas (the Program) ────────────────────────────

/// The canvas program that handles input and rendering.
/// Holds references to app state needed for drawing (theme colors, etc).
pub struct SchematicCanvas {
    pub bg_cache: canvas::Cache,
    pub content_cache: canvas::Cache,
    pub overlay_cache: canvas::Cache,
    pub grid_visible: bool,
    pub theme_bg: Color,
    pub theme_grid: Color,
    pub theme_paper: Color,
    pub canvas_colors: signex_types::theme::CanvasColors,
    /// Reference to the currently loaded schematic (if any).
    /// Set by the app when a file is opened.
    pub schematic: Option<signex_types::schematic::SchematicSheet>,
    /// Currently selected items — drives selection overlay rendering.
    pub selected: Vec<signex_types::schematic::SelectedItem>,
    /// Pending fit target to transfer to CanvasState.
    pub pending_fit: Option<Rectangle>,
}

impl SchematicCanvas {
    pub fn new() -> Self {
        let default_colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::CatppuccinMocha);
        Self {
            bg_cache: canvas::Cache::default(),
            content_cache: canvas::Cache::default(),
            overlay_cache: canvas::Cache::default(),
            grid_visible: true,
            theme_bg: Color::from_rgb8(0x1a, 0x1b, 0x2e),
            theme_grid: Color::from_rgb8(0x2d, 0x30, 0x60),
            theme_paper: Color::from_rgb8(0x1e, 0x20, 0x35),
            canvas_colors: default_colors,
            schematic: None,
            selected: Vec::new(),
            pending_fit: None,
        }
    }

    pub fn clear_overlay_cache(&mut self) {
        self.overlay_cache.clear();
    }

    pub fn clear_bg_cache(&mut self) {
        self.bg_cache.clear();
    }

    pub fn clear_content_cache(&mut self) {
        self.content_cache.clear();
    }

    /// Fit the camera to show the schematic content.
    pub fn fit_to_paper(&mut self) {
        if let Some(ref sheet) = self.schematic {
            if let Some(bounds) = sheet.content_bounds() {
                self.pending_fit = Some(Rectangle::new(
                    iced::Point::new(bounds.min_x as f32, bounds.min_y as f32),
                    iced::Size::new(bounds.width() as f32, bounds.height() as f32),
                ));
            }
        }
    }

    pub fn set_theme_colors(&mut self, bg: Color, grid: Color, paper: Color) {
        self.theme_bg = bg;
        self.theme_grid = grid;
        self.theme_paper = paper;
        self.bg_cache.clear();
    }
}

impl canvas::Program<Message> for SchematicCanvas {
    type State = CanvasState;

    fn update(
        &self,
        state: &mut CanvasState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        // Transfer pending fit from SchematicCanvas to CanvasState
        if let Some(target) = self.pending_fit {
            state.pending_fit = Some(target);
        }

        // Apply pending fit-to-content
        if let Some(target) = state.pending_fit.take() {
            state.camera.fit_rect(target, bounds);
            return Some(canvas::Action::publish(Message::CanvasEvent(
                CanvasEvent::CursorMoved,
            )));
        }

        match event {
            // ── Mouse scroll → zoom ──
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let scroll_y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / 50.0,
                };

                if let Some(cursor_pos) = cursor.position_in(bounds) {
                    state.camera.zoom_at(cursor_pos, scroll_y, bounds);
                    // Grid + content need redraw on zoom
                    return Some(
                        canvas::Action::publish(Message::CanvasEvent(
                            CanvasEvent::CursorMoved,
                        ))
                        .and_capture(),
                    );
                }
                None
            }

            // ── Left-click → select ──
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(cursor_pos) = cursor.position_in(bounds) {
                    let world = state.camera.screen_to_world(cursor_pos, bounds);
                    return Some(canvas::Action::publish(Message::CanvasEvent(
                        CanvasEvent::Clicked {
                            world_x: world.x as f64,
                            world_y: world.y as f64,
                        },
                    )));
                }
                None
            }

            // ── Right-click press → start pan ──
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right))
            | Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    state.panning = true;
                    state.last_pan_pos = Some(pos);
                }
                Some(canvas::Action::capture())
            }

            // ── Right-click release → stop pan ──
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right))
            | Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)) => {
                state.panning = false;
                state.last_pan_pos = None;
                Some(canvas::Action::capture())
            }

            // ── Mouse move ──
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(cursor_pos) = cursor.position_in(bounds) {
                    // Pan if right/middle button held
                    if state.panning {
                        if let Some(last) = state.last_pan_pos {
                            let dx = cursor_pos.x - last.x;
                            let dy = cursor_pos.y - last.y;
                            state.camera.pan(dx, dy);
                        }
                        state.last_pan_pos = Some(cursor_pos);
                        // Panning changes camera offset → grid must redraw
                        return Some(canvas::Action::publish(Message::CanvasEvent(
                            CanvasEvent::CursorMoved,
                        )));
                    }

                    // Regular hover — update cursor position for status bar (no cache clear)
                    let world = state.camera.screen_to_world(cursor_pos, bounds);
                    let zoom_pct = state.camera.zoom_percent();
                    return Some(canvas::Action::publish(Message::CanvasEvent(
                        CanvasEvent::CursorAt { x: world.x, y: world.y, zoom_pct },
                    )));
                }
                None
            }

            _ => None,
        }
    }

    fn draw(
        &self,
        state: &CanvasState,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut layers = Vec::with_capacity(4);

        // Layer 1: background (grid + paper)
        let bg = self.bg_cache.draw(renderer, bounds.size(), |frame| {
            // Fill background
            frame.fill_rectangle(
                iced::Point::ORIGIN,
                bounds.size(),
                self.theme_bg,
            );

            // Draw paper rectangle (A4 landscape: 297x210mm)
            let paper_tl = state.camera.world_to_screen(
                iced::Point::new(0.0, 0.0),
                bounds,
            );
            let paper_br = state.camera.world_to_screen(
                iced::Point::new(297.0, 210.0),
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

            // Draw grid
            if self.grid_visible {
                grid::draw_grid(
                    frame,
                    &state.camera,
                    &state.grid,
                    bounds,
                    self.theme_grid,
                );
            }
        });
        layers.push(bg);

        // Layer 2: content (schematic elements)
        let content = self.content_cache.draw(renderer, bounds.size(), |frame| {
            if let Some(ref sheet) = self.schematic {
                let transform = signex_render::schematic::ScreenTransform {
                    offset_x: state.camera.offset.x,
                    offset_y: state.camera.offset.y,
                    scale: state.camera.scale,
                };
                signex_render::schematic::render_schematic(
                    frame,
                    sheet,
                    &transform,
                    &self.canvas_colors,
                    bounds,
                );
            }
        });
        layers.push(content);

        // Layer 3: selection overlay (cached, cleared on selection change)
        if !self.selected.is_empty() {
            if let Some(ref sheet) = self.schematic {
                let sel_overlay = self.overlay_cache.draw(renderer, bounds.size(), |frame| {
                    let transform = signex_render::schematic::ScreenTransform {
                        offset_x: state.camera.offset.x,
                        offset_y: state.camera.offset.y,
                        scale: state.camera.scale,
                    };
                    signex_render::schematic::selection::draw_selection_overlay(
                        frame,
                        sheet,
                        &self.selected,
                        &transform,
                    );
                });
                layers.push(sel_overlay);
            }
        }

        // Layer 4: overlay (cursor crosshair — redrawn every frame)
        let overlay = {
            let mut frame = canvas::Frame::new(renderer, bounds.size());

            if let Some(cursor_pos) = cursor.position_in(bounds) {
                let crosshair_color = Color::from_rgba8(255, 255, 255, 0.3);
                let h_line = canvas::Path::line(
                    iced::Point::new(0.0, cursor_pos.y),
                    iced::Point::new(bounds.width, cursor_pos.y),
                );
                let v_line = canvas::Path::line(
                    iced::Point::new(cursor_pos.x, 0.0),
                    iced::Point::new(cursor_pos.x, bounds.height),
                );
                let stroke = canvas::Stroke::default()
                    .with_color(crosshair_color)
                    .with_width(0.5);
                frame.stroke(&h_line, stroke);
                frame.stroke(&v_line, stroke);
            }

            frame.into_geometry()
        };
        layers.push(overlay);

        layers
    }

    fn mouse_interaction(
        &self,
        state: &CanvasState,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.panning {
            mouse::Interaction::Grabbing
        } else {
            mouse::Interaction::default()
        }
    }
}

// ─── Canvas events sent to the app ────────────────────────────

#[derive(Debug, Clone)]
pub enum CanvasEvent {
    CursorAt { x: f32, y: f32, zoom_pct: f64 },
    CursorMoved,
    FitAll,
    /// Left-click at world coordinates — triggers hit-testing.
    Clicked { world_x: f64, world_y: f64 },
}
