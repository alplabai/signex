//! Schematic/PCB canvas — wgpu rendering with Altium-style pan/zoom/grid.
//!
//! Uses `iced::widget::canvas::Program` with a 3-layer cache:
//! - background: grid, sheet border (cleared on theme/grid/zoom change)
//! - content: schematic elements (cleared on document edit)
//! - overlay: selection, cursor, wire-in-progress (cleared every frame)

mod camera;
pub mod grid;

use iced::event::Event;
use iced::mouse;
use iced::widget::canvas;
use iced::{Color, Rectangle, Renderer, Theme};

pub use camera::Camera;
pub use grid::GridState;

use crate::app::Message;

// ─── Canvas State (per-canvas mutable state) ──────────────────

#[derive(Debug, Default)]
pub struct CanvasState {
    pub camera: Camera,
    pub grid: GridState,
    /// Is the user currently panning (right-click or middle-click drag)?
    panning: bool,
    /// Whether actual pan movement occurred (to distinguish click from drag).
    pan_moved: bool,
    /// Last cursor position during a pan (in screen pixels).
    last_pan_pos: Option<iced::Point>,
    /// Pending fit target — consumed on next update.
    pub pending_fit: Option<Rectangle>,
    /// Whether Ctrl is currently held (for multi-select).
    pub ctrl_held: bool,
    /// Drag-to-select: start position in world coordinates.
    select_drag_start: Option<(f64, f64)>,
    /// Drag-to-select: current end position in world coordinates.
    select_drag_end: Option<(f64, f64)>,
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
    /// Uses Cell so canvas::Program::update (&self) can consume it.
    pub pending_fit: std::cell::Cell<Option<Rectangle>>,
    /// Wire-in-progress points for rubber-band preview.
    pub wire_preview: Vec<signex_types::schematic::Point>,
    /// Whether currently in wire/bus drawing mode.
    pub drawing_mode: bool,
    /// Current tool name for preview display.
    pub tool_preview: Option<String>,
    /// Current draw mode for wire preview constraint (90°, 45°, free).
    pub draw_mode: crate::app::DrawMode,
}

impl SchematicCanvas {
    pub fn new() -> Self {
        let default_colors =
            signex_types::theme::canvas_colors(signex_types::theme::ThemeId::CatppuccinMocha);
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
            pending_fit: std::cell::Cell::new(None),
            wire_preview: Vec::new(),
            drawing_mode: false,
            tool_preview: None,
            draw_mode: crate::app::DrawMode::Ortho90,
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
        if let Some(ref sheet) = self.schematic
            && let Some(bounds) = sheet.content_bounds()
        {
            self.pending_fit.set(Some(Rectangle::new(
                iced::Point::new(bounds.min_x as f32, bounds.min_y as f32),
                iced::Size::new(bounds.width() as f32, bounds.height() as f32),
            )));
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
        // Transfer pending fit from SchematicCanvas to CanvasState (consumes it)
        if let Some(target) = self.pending_fit.take() {
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
                        canvas::Action::publish(Message::CanvasEvent(CanvasEvent::CursorMoved))
                            .and_capture(),
                    );
                }
                None
            }

            // ── Left-click → select, tool action, or start drag-select ──
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(cursor_pos) = cursor.position_in(bounds) {
                    let world = state.camera.screen_to_world(cursor_pos, bounds);
                    // Start potential drag-to-select
                    state.select_drag_start = Some((world.x as f64, world.y as f64));
                    state.select_drag_end = None;
                    let evt = if state.ctrl_held {
                        CanvasEvent::CtrlClicked {
                            world_x: world.x as f64,
                            world_y: world.y as f64,
                        }
                    } else {
                        CanvasEvent::Clicked {
                            world_x: world.x as f64,
                            world_y: world.y as f64,
                        }
                    };
                    return Some(canvas::Action::publish(Message::CanvasEvent(evt)));
                }
                None
            }
            // ── Left-click release → finish drag-select ──
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let (Some(start), Some(end)) = (state.select_drag_start.take(), state.select_drag_end.take()) {
                    let dx = (end.0 - start.0).abs();
                    let dy = (end.1 - start.1).abs();
                    // Only trigger box select if dragged more than 2mm
                    if dx > 2.0 || dy > 2.0 {
                        return Some(canvas::Action::publish(Message::CanvasEvent(
                            CanvasEvent::BoxSelect {
                                x1: start.0.min(end.0),
                                y1: start.1.min(end.1),
                                x2: start.0.max(end.0),
                                y2: start.1.max(end.1),
                            },
                        )));
                    }
                } else {
                    state.select_drag_start = None;
                }
                None
            }

            // ── Keyboard events for Ctrl detection ──
            Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) => {
                state.ctrl_held = mods.command();
                None
            }

            // ── Right-click press → start pan or Active Bar dropdown ──
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    // Active Bar zone: top ~40px, centered
                    if pos.y < 40.0 {
                        // Calculate which Active Bar button was right-clicked
                        let bar_width: f32 = 338.0;
                        let bar_x = (bounds.width - bar_width) / 2.0;
                        let rel_x = pos.x - bar_x;
                        if rel_x >= 0.0 && rel_x < bar_width {
                            if let Some(menu) = active_bar_hit(rel_x) {
                                return Some(canvas::Action::publish(
                                    Message::ActiveBar(
                                        crate::active_bar::ActiveBarMsg::ToggleMenu(menu),
                                    ),
                                ));
                            }
                        }
                    }
                    state.panning = true;
                    state.pan_moved = false;
                    state.last_pan_pos = Some(pos);
                }
                Some(canvas::Action::capture())
            }
            // ── Middle-click press → start pan ──
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    state.panning = true;
                    state.pan_moved = false;
                    state.last_pan_pos = Some(pos);
                }
                Some(canvas::Action::capture())
            }

            // ── Right-click release → stop pan, context menu or cancel drawing ──
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right)) => {
                let did_pan = state.pan_moved;
                state.panning = false;
                state.pan_moved = false;
                state.last_pan_pos = None;
                if !did_pan {
                    if self.drawing_mode {
                        // Right-click cancels wire drawing (Altium behavior)
                        return Some(canvas::Action::publish(Message::CancelDrawing));
                    }
                    // Show context menu at screen position
                    if let Some(cursor_pos) = cursor.position_in(bounds) {
                        let screen_x = bounds.x + cursor_pos.x;
                        let screen_y = bounds.y + cursor_pos.y;
                        return Some(canvas::Action::publish(Message::ShowContextMenu(
                            screen_x, screen_y,
                        )));
                    }
                }
                Some(canvas::Action::capture())
            }
            // ── Middle-click release → stop pan ──
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)) => {
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
                            if dx.abs() > 2.0 || dy.abs() > 2.0 {
                                state.pan_moved = true;
                            }
                            state.camera.pan(dx, dy);
                        }
                        state.last_pan_pos = Some(cursor_pos);
                        // Panning changes camera offset → grid must redraw
                        return Some(canvas::Action::publish(Message::CanvasEvent(
                            CanvasEvent::CursorMoved,
                        )));
                    }

                    // Track drag-to-select
                    if state.select_drag_start.is_some() {
                        let world = state.camera.screen_to_world(cursor_pos, bounds);
                        state.select_drag_end = Some((world.x as f64, world.y as f64));
                    }

                    // Regular hover — update cursor position for status bar
                    let world = state.camera.screen_to_world(cursor_pos, bounds);
                    let zoom_pct = state.camera.zoom_percent();
                    return Some(canvas::Action::publish(Message::CanvasEvent(
                        CanvasEvent::CursorAt {
                            x: world.x,
                            y: world.y,
                            zoom_pct,
                        },
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
            frame.fill_rectangle(iced::Point::ORIGIN, bounds.size(), self.theme_bg);

            // Draw paper rectangle (A4 landscape: 297x210mm)
            let paper_tl = state
                .camera
                .world_to_screen(iced::Point::new(0.0, 0.0), bounds);
            let paper_br = state
                .camera
                .world_to_screen(iced::Point::new(297.0, 210.0), bounds);
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
                grid::draw_grid(frame, &state.camera, &state.grid, bounds, self.theme_grid);
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
        if !self.selected.is_empty()
            && let Some(ref sheet) = self.schematic
        {
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

                // Wire-in-progress rubber-band preview
                if self.drawing_mode && !self.wire_preview.is_empty() {
                    let wire_color = self.canvas_colors.wire;
                    let wire_color_iced = signex_render::colors::to_iced(&wire_color);
                    let preview_stroke = canvas::Stroke::default()
                        .with_color(wire_color_iced)
                        .with_width(1.5);

                    // Draw placed segments
                    for pair in self.wire_preview.windows(2) {
                        let p1 = state.camera.world_to_screen(
                            iced::Point::new(pair[0].x as f32, pair[0].y as f32),
                            bounds,
                        );
                        let p2 = state.camera.world_to_screen(
                            iced::Point::new(pair[1].x as f32, pair[1].y as f32),
                            bounds,
                        );
                        let seg = canvas::Path::line(p1, p2);
                        frame.stroke(&seg, preview_stroke);
                    }

                    // Rubber-band from last point to cursor (constrained by draw mode)
                    if let Some(last) = self.wire_preview.last() {
                        let cursor_world = state.camera.screen_to_world(cursor_pos, bounds);
                        let start = signex_types::schematic::Point::new(last.x, last.y);
                        let end = signex_types::schematic::Point::new(
                            cursor_world.x as f64,
                            cursor_world.y as f64,
                        );
                        let rubber_stroke = canvas::Stroke::default()
                            .with_color(Color { a: 0.6, ..wire_color_iced })
                            .with_width(1.0);

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
                                        start.x + d * sx, start.y + d * sy,
                                    );
                                    if adx > ady {
                                        vec![(start, diag_end), (diag_end, signex_types::schematic::Point::new(end.x, diag_end.y))]
                                    } else {
                                        vec![(start, diag_end), (diag_end, signex_types::schematic::Point::new(diag_end.x, end.y))]
                                    }
                                } else {
                                    let corner = signex_types::schematic::Point::new(end.x, start.y);
                                    vec![(start, corner), (corner, end)]
                                }
                            }
                        };

                        for (p1, p2) in &segments {
                            let s1 = state.camera.world_to_screen(
                                iced::Point::new(p1.x as f32, p1.y as f32), bounds,
                            );
                            let s2 = state.camera.world_to_screen(
                                iced::Point::new(p2.x as f32, p2.y as f32), bounds,
                            );
                            frame.stroke(&canvas::Path::line(s1, s2), rubber_stroke);
                        }
                    }
                }

                // Tool preview text at cursor (for Label, Component placement)
                if let Some(ref label) = self.tool_preview {
                    frame.fill_text(canvas::Text {
                        content: label.clone(),
                        position: iced::Point::new(cursor_pos.x + 12.0, cursor_pos.y - 12.0),
                        color: Color::from_rgba(1.0, 1.0, 1.0, 0.7),
                        size: iced::Pixels(11.0),
                        ..canvas::Text::default()
                    });
                }
            }

            // Drag-to-select rectangle
            if let (Some(start), Some(end)) = (state.select_drag_start, state.select_drag_end) {
                let s1 = state.camera.world_to_screen(
                    iced::Point::new(start.0 as f32, start.1 as f32), bounds,
                );
                let s2 = state.camera.world_to_screen(
                    iced::Point::new(end.0 as f32, end.1 as f32), bounds,
                );
                let x = s1.x.min(s2.x);
                let y = s1.y.min(s2.y);
                let w = (s2.x - s1.x).abs();
                let h = (s2.y - s1.y).abs();
                if w > 2.0 || h > 2.0 {
                    // Fill (semi-transparent blue)
                    frame.fill_rectangle(
                        iced::Point::new(x, y),
                        iced::Size::new(w, h),
                        Color::from_rgba(0.2, 0.4, 0.8, 0.15),
                    );
                    // Border (dashed blue)
                    let rect_path = canvas::Path::rectangle(
                        iced::Point::new(x, y),
                        iced::Size::new(w, h),
                    );
                    frame.stroke(
                        &rect_path,
                        canvas::Stroke::default()
                            .with_color(Color::from_rgba(0.3, 0.5, 1.0, 0.7))
                            .with_width(1.0),
                    );
                }
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

// ─── Active Bar hit detection for right-click ────────────────

/// Map a relative x position within the Active Bar to a dropdown menu.
/// Returns None for buttons without dropdowns (Select, Add Component).
fn active_bar_hit(x: f32) -> Option<crate::active_bar::ActiveBarMenu> {
    use crate::active_bar::ActiveBarMenu;
    // Each btn=23px (22+1spacing), sep=2px (1+1spacing), pad=4px
    // [Filter][+] | [Select][Move][Align] | [Wire][Power] | [Harness][Sheet][Port][Dir] | [Text][Shapes][NetColor]
    //   0     23  48   50    73    96     121  123   146   171  173    196   219   242   267  269   292    315
    let x = x - 4.0;
    let b = 23; // button width
    let s = 2;  // separator
    let xi = x as i32;
    if xi < 0 { return None; }
    // btn 0: Filter
    if xi < b { return Some(ActiveBarMenu::Filter); }
    // btn 1: Add Component (no dropdown)
    if xi < 2 * b { return None; }
    // sep
    let off = 2 * b + s;
    // btn 2: Select
    if xi >= off && xi < off + b { return Some(ActiveBarMenu::SelectMode); }
    // btn 3: Move
    if xi >= off + b && xi < off + 2 * b { return Some(ActiveBarMenu::Select); }
    // btn 4: Align
    if xi >= off + 2 * b && xi < off + 3 * b { return Some(ActiveBarMenu::Align); }
    // sep
    let off = off + 3 * b + s;
    // btn 5: Wire
    if xi >= off && xi < off + b { return Some(ActiveBarMenu::Wiring); }
    // btn 6: Power
    if xi >= off + b && xi < off + 2 * b { return Some(ActiveBarMenu::Power); }
    // sep
    let off = off + 2 * b + s;
    // btn 7: Harness
    if xi >= off && xi < off + b { return Some(ActiveBarMenu::Harness); }
    // btn 8: Sheet Symbol
    if xi >= off + b && xi < off + 2 * b { return Some(ActiveBarMenu::SheetSymbol); }
    // btn 9: Port
    if xi >= off + 2 * b && xi < off + 3 * b { return Some(ActiveBarMenu::Port); }
    // btn 10: Directives
    if xi >= off + 3 * b && xi < off + 4 * b { return Some(ActiveBarMenu::Directives); }
    // sep
    let off = off + 4 * b + s;
    // btn 11: Text
    if xi >= off && xi < off + b { return Some(ActiveBarMenu::TextTools); }
    // btn 12: Shapes
    if xi >= off + b && xi < off + 2 * b { return Some(ActiveBarMenu::Shapes); }
    // btn 13: Net Color
    if xi >= off + 2 * b && xi < off + 3 * b { return Some(ActiveBarMenu::NetColor); }
    None
}

// ─── Canvas events sent to the app ────────────────────────────

#[derive(Debug, Clone)]
pub enum CanvasEvent {
    CursorAt {
        x: f32,
        y: f32,
        zoom_pct: f64,
    },
    CursorMoved,
    FitAll,
    /// Left-click at world coordinates — triggers hit-testing or tool action.
    Clicked {
        world_x: f64,
        world_y: f64,
    },
    /// Ctrl+Left-click for multi-select.
    CtrlClicked {
        world_x: f64,
        world_y: f64,
    },
    /// Double-click at world coordinates.
    #[allow(dead_code)]
    DoubleClicked {
        world_x: f64,
        world_y: f64,
    },
    /// Box selection — select all items within the rectangle (world coords).
    BoxSelect {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
}
