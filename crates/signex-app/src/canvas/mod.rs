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
    pub _grid: GridState,
    /// Is the user currently panning (right-click or middle-click drag)?
    panning: bool,
    /// Whether actual pan movement occurred (to distinguish click from drag).
    pan_moved: bool,
    /// Last cursor position during a pan (in screen pixels).
    last_pan_pos: Option<iced::Point>,
    /// Pending fit target — consumed on next update.
    pub pending_fit: Option<Rectangle>,
    /// Whether Ctrl is currently held (for multi-select toggle).
    pub ctrl_held: bool,
    /// Whether Shift is currently held (for multi-select add).
    pub shift_held: bool,
    /// Drag-to-select: start position in world coordinates.
    select_drag_start: Option<(f64, f64)>,
    /// Drag-to-select: current end position in world coordinates.
    select_drag_end: Option<(f64, f64)>,
    // ─── Drag-to-move state ───
    /// True when the initial left-click landed on an already-selected item.
    click_on_selected: bool,
    /// World position where the move-drag started.
    move_origin: Option<(f64, f64)>,
    /// True once the threshold is exceeded and we're actively moving.
    move_dragging: bool,
    /// Current world position during a move-drag (for preview offset).
    move_current: Option<(f64, f64)>,
    // ─── Double-click detection ───
    /// Timestamp of last left-click (for double-click detection).
    last_click_time: Option<std::time::Instant>,
    /// World position of last left-click.
    last_click_world: Option<(f64, f64)>,
}

// ─── SchematicCanvas (the Program) ────────────────────────────

/// The canvas program that handles input and rendering.
/// Holds references to app state needed for drawing (theme colors, etc).
pub struct SchematicCanvas {
    pub bg_cache: canvas::Cache,
    pub content_cache: canvas::Cache,
    pub overlay_cache: canvas::Cache,
    /// Camera state when content_cache was last built — used to compute offset delta.
    pub content_cache_camera: std::cell::Cell<(f32, f32, f32)>, // (offset_x, offset_y, scale)
    /// Camera state as of the most recent draw, updated every frame including
    /// mid-pan. Overlays positioned relative to world coordinates (inline text
    /// editor, measurements) read this so they track pan/zoom in real time.
    pub live_camera: std::cell::Cell<(f32, f32, f32)>,
    pub grid_visible: bool,
    pub theme_bg: Color,
    pub theme_grid: Color,
    pub theme_paper: Color,
    pub canvas_colors: signex_types::theme::CanvasColors,
    /// Render-facing cache of the currently visible schematic.
    /// The app updates this from the active engine or active tab cache.
    pub render_cache: Option<signex_render::schematic::SchematicRenderCache>,
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
    /// Ghost label preview for port/label placement (follows cursor).
    pub ghost_label: Option<signex_types::schematic::Label>,
    /// Ghost power-port / symbol preview for placement (follows cursor).
    pub ghost_symbol: Option<signex_types::schematic::Symbol>,
    /// Ghost text-note preview for placement (follows cursor).
    pub ghost_text: Option<signex_types::schematic::TextNote>,
    /// When true, placement is paused (TAB pressed → pre-placement form
    /// open). The ghost freezes and canvas clicks don't place — the user
    /// interacts with the Properties panel until they confirm with OK.
    pub placement_paused: bool,
    /// Current draw mode for wire preview constraint (90°, 45°, free).
    pub draw_mode: crate::app::DrawMode,
    /// Whether snap-to-grid is enabled (for rubber-band cursor snapping).
    pub snap_enabled: bool,
    /// Grid size in mm for rubber-band cursor snapping AND visible grid rendering.
    pub snap_grid_mm: f64,
    /// Visible grid dot spacing in mm (independent of snap grid).
    pub visible_grid_mm: f64,
    /// Active paper width in mm (world units).
    pub paper_width_mm: f32,
    /// Active paper height in mm (world units).
    pub paper_height_mm: f32,
    /// When true, non-selected items dim on the canvas (F9). Synced
    /// from `ui_state.auto_focus` so the renderer can compute a focus
    /// uuid set without reaching back into app state.
    pub auto_focus: bool,
    /// ERC violations to highlight on the canvas — Altium-style marker
    /// dots + primary-item halos. Synced from `ui_state.erc_violations`
    /// after each ERC run so the overlay renders without the canvas
    /// reaching back into app state.
    pub erc_markers: Vec<ErcMarker>,
    /// Armed net-color from the Active Bar palette. `Some(c)` with a
    /// non-zero alpha means the next wire click floods that colour
    /// onto the whole connected net; alpha 0 signals "clear one".
    /// Drives the pen cursor drawn over the canvas.
    pub pending_net_color: Option<signex_types::theme::Color>,
    /// Per-wire colour overrides consulted when drawing wires. Synced
    /// from `ui_state.wire_color_overrides` on every canvas rebuild.
    pub wire_color_overrides: std::collections::HashMap<uuid::Uuid, signex_types::theme::Color>,
    /// In-flight lasso polygon in world space. Synced from
    /// `ui_state.lasso_polygon` so the overlay draw can render the
    /// committed vertices + rubber-band to the cursor without
    /// reaching into app state.
    pub lasso_polygon: Option<Vec<signex_types::schematic::Point>>,
    /// In-flight 3-click arc (start, mid) while Tool::Arc is active.
    /// Mirrors `interaction_state.arc_points` for the preview draw.
    pub arc_points: Vec<signex_types::schematic::Point>,
    /// In-flight polyline vertices while Tool::Polyline is active.
    /// Mirrors `interaction_state.polyline_points`.
    pub polyline_points: Vec<signex_types::schematic::Point>,
    /// When the BringToFrontOf / SendToBackOf picker is armed, show
    /// the gray-X placement cursor so the user knows the next click
    /// is a reference pick, not a selection. Synced from
    /// `ui_state.reorder_picker.is_some()` at the top of each view().
    pub reorder_picker_armed: bool,
    /// Two-click shape anchor + which shape is being drawn. Used by
    /// the rubber-band preview for Line / Rectangle / Circle.
    pub shape_anchor: Option<(signex_types::schematic::Point, ShapePreviewKind)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapePreviewKind {
    Line,
    Rect,
    Circle,
}

/// Canvas-side projection of an ERC violation — just enough to draw
/// its marker without pulling the full Violation type into the render
/// crate.
#[derive(Debug, Clone)]
pub struct ErcMarker {
    pub x: f64,
    pub y: f64,
    pub severity: ErcMarkerSeverity,
    /// Uuid of the primary offending item (label, wire, symbol, …).
    /// Reserved for a future halo/highlight pass; unused today.
    #[allow(dead_code)]
    pub primary_uuid: Option<uuid::Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErcMarkerSeverity {
    Error,
    Warning,
    Info,
}

impl SchematicCanvas {
    pub fn active_render_cache(&self) -> Option<&signex_render::schematic::SchematicRenderCache> {
        self.render_cache.as_ref()
    }

    pub fn active_snapshot(&self) -> Option<&signex_render::schematic::SchematicRenderSnapshot> {
        self.render_cache.as_ref().map(|cache| cache.snapshot())
    }

    pub fn new() -> Self {
        let default_colors =
            signex_types::theme::canvas_colors(signex_types::theme::ThemeId::Signex);
        Self {
            bg_cache: canvas::Cache::default(),
            content_cache: canvas::Cache::default(),
            overlay_cache: canvas::Cache::default(),
            content_cache_camera: std::cell::Cell::new((0.0, 0.0, 1.0)),
            live_camera: std::cell::Cell::new((0.0, 0.0, 1.0)),
            grid_visible: true,
            theme_bg: {
                let c = &default_colors.background;
                Color::from_rgb8(c.r, c.g, c.b)
            },
            theme_grid: {
                let c = &default_colors.grid;
                Color::from_rgb8(c.r, c.g, c.b)
            },
            theme_paper: {
                let c = &default_colors.paper;
                Color::from_rgb8(c.r, c.g, c.b)
            },
            canvas_colors: default_colors,
            render_cache: None,
            selected: Vec::new(),
            pending_fit: std::cell::Cell::new(None),
            wire_preview: Vec::new(),
            drawing_mode: false,
            tool_preview: None,
            ghost_label: None,
            ghost_symbol: None,
            ghost_text: None,
            placement_paused: false,
            draw_mode: crate::app::DrawMode::Ortho90,
            snap_enabled: true,
            snap_grid_mm: 2.54,
            visible_grid_mm: 2.54,
            paper_width_mm: 297.0,
            paper_height_mm: 210.0,
            auto_focus: false,
            erc_markers: Vec::new(),
            pending_net_color: None,
            wire_color_overrides: std::collections::HashMap::new(),
            lasso_polygon: None,
            arc_points: Vec::new(),
            polyline_points: Vec::new(),
            reorder_picker_armed: false,
            shape_anchor: None,
        }
    }

    /// Compute the "focus" uuid set when auto_focus is on — members of
    /// the current selection. Returns None when auto_focus is off; the
    /// renderer then draws every item at full alpha.
    fn auto_focus_set(&self) -> Option<std::collections::HashSet<uuid::Uuid>> {
        if !self.auto_focus {
            return None;
        }
        let mut set = std::collections::HashSet::new();
        for item in &self.selected {
            set.insert(item.uuid);
        }
        Some(set)
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

    pub fn set_render_cache(
        &mut self,
        render_cache: Option<signex_render::schematic::SchematicRenderCache>,
    ) {
        self.render_cache = render_cache;
    }

    /// Fit the camera to show the schematic content.
    pub fn fit_to_paper(&mut self) {
        if let Some(snapshot) = self.active_snapshot()
            && let Some(bounds) = snapshot.content_bounds()
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
                    let changed = state.camera.zoom_at(cursor_pos, scroll_y, bounds);
                    if !changed {
                        return None;
                    }
                    // Grid + content need redraw on zoom
                    return Some(
                        canvas::Action::publish(Message::CanvasEvent(CanvasEvent::CursorMoved))
                            .and_capture(),
                    );
                }
                None
            }

            // ── Left-click → select, tool action, start drag-select, or start drag-move ──
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(cursor_pos) = cursor.position_in(bounds) {
                    let world = state.camera.screen_to_world(cursor_pos, bounds);
                    let wx = world.x as f64;
                    let wy = world.y as f64;

                    // Double-click detection (300ms, 3mm threshold)
                    let now = std::time::Instant::now();
                    if let (Some(last_time), Some(last_pos)) =
                        (state.last_click_time, state.last_click_world)
                    {
                        let dt = now.duration_since(last_time);
                        let dist = ((wx - last_pos.0).powi(2) + (wy - last_pos.1).powi(2)).sqrt();
                        if dt.as_millis() < 300 && dist < 3.0 {
                            state.last_click_time = None;
                            state.last_click_world = None;
                            state.select_drag_start = None;
                            state.click_on_selected = false;
                            return Some(canvas::Action::publish(Message::CanvasEvent(
                                CanvasEvent::DoubleClicked {
                                    world_x: wx,
                                    world_y: wy,
                                    screen_x: cursor_pos.x,
                                    screen_y: cursor_pos.y,
                                },
                            )));
                        }
                    }
                    state.last_click_time = Some(now);
                    state.last_click_world = Some((wx, wy));

                    // Classify the click target:
                    //   - hit an already-selected item  → defer click, prepare drag
                    //   - hit an unselected item       → publish click (selects it), prepare drag
                    //   - hit empty space              → publish click, start box-select
                    // Altium-style: clicking and dragging on an unselected item
                    // should immediately select-and-drag in one gesture.
                    let (on_selected, on_unselected_item) = if !self.drawing_mode {
                        if let Some(snapshot) = self.active_snapshot() {
                            if let Some(hit) =
                                signex_render::schematic::hit_test::hit_test(snapshot, wx, wy)
                            {
                                let sel = self.selected.iter().any(|s| s.uuid == hit.uuid);
                                (sel, !sel)
                            } else {
                                (false, false)
                            }
                        } else {
                            (false, false)
                        }
                    } else {
                        (false, false)
                    };

                    if on_selected && !state.ctrl_held {
                        // Defer click — prepare for potential drag-to-move
                        state.click_on_selected = true;
                        state.move_origin = Some((wx, wy));
                        state.move_dragging = false;
                        state.move_current = None;
                        state.select_drag_start = None;
                        return Some(canvas::Action::capture());
                    }

                    if on_unselected_item && !state.ctrl_held {
                        // Publish the click (which selects the item via HitAt)
                        // AND prepare drag state. If the user crosses the drag
                        // threshold before mouse-up, the motion handler promotes
                        // this into a move gesture without a second click.
                        // `and_capture()` keeps subsequent mouse events flowing
                        // through this program so the drag detection stays live.
                        state.click_on_selected = true;
                        state.move_origin = Some((wx, wy));
                        state.move_dragging = false;
                        state.move_current = None;
                        state.select_drag_start = None;
                        return Some(
                            canvas::Action::publish(Message::CanvasEvent(CanvasEvent::Clicked {
                                world_x: wx,
                                world_y: wy,
                            }))
                            .and_capture(),
                        );
                    }

                    // Normal click — publish immediately, start potential box-select
                    state.click_on_selected = false;
                    state.move_origin = None;
                    state.move_dragging = false;
                    // Don't track box-select during drawing mode (avoids spurious BoxSelect events)
                    if !self.drawing_mode {
                        state.select_drag_start = Some((wx, wy));
                        state.select_drag_end = None;
                    } else {
                        state.select_drag_start = None;
                        state.select_drag_end = None;
                    }
                    // Ctrl+Click toggles selection (add if missing, remove if
                    // present). Shift+Click adds to selection (Altium-style).
                    let evt = if state.ctrl_held || state.shift_held {
                        CanvasEvent::CtrlClicked {
                            world_x: wx,
                            world_y: wy,
                        }
                    } else {
                        CanvasEvent::Clicked {
                            world_x: wx,
                            world_y: wy,
                        }
                    };
                    return Some(canvas::Action::publish(Message::CanvasEvent(evt)));
                }
                None
            }
            // ── Left-click release → finish drag-select, drag-move, or deferred click ──
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                // Case 1: Drag-to-move in progress → commit the move
                if state.move_dragging {
                    if let (Some(origin), Some(current)) = (state.move_origin, state.move_current) {
                        let dx = current.0 - origin.0;
                        let dy = current.1 - origin.1;
                        state.move_dragging = false;
                        state.move_origin = None;
                        state.move_current = None;
                        state.click_on_selected = false;
                        if dx.abs() > 0.01 || dy.abs() > 0.01 {
                            return Some(canvas::Action::publish(Message::CanvasEvent(
                                CanvasEvent::MoveSelected { dx, dy },
                            )));
                        }
                    }
                    return None;
                }

                // Case 2: Click was on selected item but didn't drag → deferred click
                if state.click_on_selected {
                    state.click_on_selected = false;
                    if let Some(origin) = state.move_origin.take() {
                        return Some(canvas::Action::publish(Message::CanvasEvent(
                            CanvasEvent::Clicked {
                                world_x: origin.0,
                                world_y: origin.1,
                            },
                        )));
                    }
                    return None;
                }

                // Case 3: Box-select drag
                if let (Some(start), Some(end)) =
                    (state.select_drag_start.take(), state.select_drag_end.take())
                {
                    let dx = (end.0 - start.0).abs();
                    let dy = (end.1 - start.1).abs();
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
                state.shift_held = mods.shift();
                None
            }

            // ── Escape cancels any in-progress drag ──
            Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                ..
            }) => {
                if state.move_dragging || state.click_on_selected {
                    state.move_dragging = false;
                    state.click_on_selected = false;
                    state.move_origin = None;
                    state.move_current = None;
                    return Some(canvas::Action::capture());
                }
                None
            }

            // ── Right-click press → cancel drag, else start pan or Active Bar dropdown ──
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                // Abort an in-progress drag — matches Altium / Esc behavior.
                if state.move_dragging || state.click_on_selected {
                    state.move_dragging = false;
                    state.click_on_selected = false;
                    state.move_origin = None;
                    state.move_current = None;
                    return Some(canvas::Action::capture());
                }
                if let Some(pos) = cursor.position_in(bounds) {
                    // Active Bar zone: top ~46px, centered (bar 36px + 4 top margin + slack)
                    if pos.y < 46.0 {
                        // Calculate which Active Bar button was right-clicked
                        let bar_width: f32 = crate::active_bar::BAR_WIDTH_PX;
                        let bar_x = (bounds.width - bar_width) / 2.0;
                        let rel_x = pos.x - bar_x;
                        if rel_x >= 0.0
                            && rel_x < bar_width
                            && let Some(menu) = active_bar_hit(rel_x)
                        {
                            return Some(canvas::Action::publish(Message::ActiveBar(
                                crate::active_bar::ActiveBarMsg::ToggleMenu(menu),
                            )));
                        }
                        // Prevent panning when right-clicking in the Active Bar zone
                        return Some(canvas::Action::capture());
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
                        return Some(
                            canvas::Action::publish(Message::CanvasEvent(CanvasEvent::CursorMoved))
                                .and_capture(),
                        );
                    }

                    // Track drag-to-move (selected items)
                    if state.click_on_selected {
                        let world = state.camera.screen_to_world(cursor_pos, bounds);
                        let wx = world.x as f64;
                        let wy = world.y as f64;
                        if let Some(origin) = state.move_origin {
                            let dist = ((wx - origin.0).powi(2) + (wy - origin.1).powi(2)).sqrt();
                            if dist > 1.0 {
                                // Exceeded threshold — switch to move mode
                                state.move_dragging = true;
                            }
                        }
                        if state.move_dragging {
                            // Snap the *delta*, not the absolute cursor. This keeps
                            // the dragged item on-grid during the live preview
                            // (origin may be off-grid, but offset stays a grid
                            // multiple so object_start + offset stays on grid).
                            let (mx, my) = if let (Some(origin), true) = (
                                state.move_origin,
                                self.snap_enabled && self.snap_grid_mm > 0.0,
                            ) {
                                let g = self.snap_grid_mm;
                                let dx = ((wx - origin.0) / g).round() * g;
                                let dy = ((wy - origin.1) / g).round() * g;
                                (origin.0 + dx, origin.1 + dy)
                            } else {
                                (wx, wy)
                            };
                            state.move_current = Some((mx, my));
                        }
                    }

                    // Track drag-to-select
                    if state.select_drag_start.is_some() && !state.click_on_selected {
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
        });
        layers.push(bg);

        // Layer 2: content (schematic elements)
        let live_transform = signex_render::schematic::ScreenTransform {
            offset_x: state.camera.offset.x,
            offset_y: state.camera.offset.y,
            scale: state.camera.scale,
        };
        // Publish the live camera every frame so world-anchored overlays
        // (inline editor) can track pan/zoom without waiting on cache rebuilds.
        self.live_camera.set((
            state.camera.offset.x,
            state.camera.offset.y,
            state.camera.scale,
        ));
        let (cached_offset_x, cached_offset_y, cached_scale) = self.content_cache_camera.get();
        let camera_matches_cache = (cached_offset_x - state.camera.offset.x).abs() < 0.01
            && (cached_offset_y - state.camera.offset.y).abs() < 0.01
            && (cached_scale - state.camera.scale).abs() < 0.0001;

        // If mid-drag, build a snapshot with selected items shifted so the
        // "move" visually lifts the originals out of place and places them at
        // the cursor — matches Altium's behavior where the dragged objects
        // travel with the mouse and nothing stays behind at the old location.
        let drag_offset = if state.move_dragging {
            state
                .move_origin
                .zip(state.move_current)
                .map(|((ox, oy), (cx, cy))| (cx - ox, cy - oy))
        } else {
            None
        };

        let shifted_snapshot =
            if let (Some((dx, dy)), Some(snap)) = (drag_offset, self.active_snapshot()) {
                if !self.selected.is_empty() {
                    Some(shift_snapshot_for_selection(snap, &self.selected, dx, dy))
                } else {
                    None
                }
            } else {
                None
            };
        let effective_snapshot: Option<&signex_render::schematic::SchematicRenderSnapshot> =
            shifted_snapshot.as_ref().or_else(|| self.active_snapshot());

        let focus_set = self.auto_focus_set();
        let focus_ref = focus_set.as_ref();
        let content = if state.panning || drag_offset.is_some() {
            let mut frame = canvas::Frame::new(renderer, bounds.size());
            if let Some(snapshot) = effective_snapshot {
                signex_render::schematic::render_schematic(
                    &mut frame,
                    snapshot,
                    &live_transform,
                    &self.canvas_colors,
                    bounds,
                    focus_ref,
                    Some(&self.wire_color_overrides),
                );
            }
            frame.into_geometry()
        } else {
            if !camera_matches_cache {
                self.content_cache.clear();
            }
            self.content_cache.draw(renderer, bounds.size(), |frame| {
                self.content_cache_camera.set((
                    state.camera.offset.x,
                    state.camera.offset.y,
                    state.camera.scale,
                ));
                if let Some(snapshot) = effective_snapshot {
                    signex_render::schematic::render_schematic(
                        frame,
                        snapshot,
                        &live_transform,
                        &self.canvas_colors,
                        bounds,
                        focus_ref,
                        Some(&self.wire_color_overrides),
                    );
                }
            })
        };
        layers.push(content);

        // Layer 2.5: AutoFocus dim — when F9 is on and a selection
        // exists, fade everything outside the selection bbox + margin
        // with a translucent dark overlay. Uses four rects forming a
        // frame around the bbox, so 2D paths can express the hole
        // without compositing modes.
        if self.auto_focus
            && !self.selected.is_empty()
            && let Some(snapshot) = effective_snapshot
        {
            use signex_types::schematic::SelectedKind;
            let mut xs: Vec<f32> = Vec::new();
            let mut ys: Vec<f32> = Vec::new();
            let mut push_pt = |x: f64, y: f64, r: f32| {
                xs.push(x as f32 - r);
                xs.push(x as f32 + r);
                ys.push(y as f32 - r);
                ys.push(y as f32 + r);
            };
            for item in &self.selected {
                match item.kind {
                    SelectedKind::Symbol
                    | SelectedKind::SymbolRefField
                    | SelectedKind::SymbolValField => {
                        if let Some(s) = snapshot.symbols.iter().find(|s| s.uuid == item.uuid) {
                            push_pt(s.position.x, s.position.y, 8.0);
                        }
                    }
                    SelectedKind::Wire => {
                        if let Some(w) = snapshot.wires.iter().find(|w| w.uuid == item.uuid) {
                            push_pt(w.start.x, w.start.y, 1.0);
                            push_pt(w.end.x, w.end.y, 1.0);
                        }
                    }
                    SelectedKind::Bus => {
                        if let Some(b) = snapshot.buses.iter().find(|b| b.uuid == item.uuid) {
                            push_pt(b.start.x, b.start.y, 1.0);
                            push_pt(b.end.x, b.end.y, 1.0);
                        }
                    }
                    SelectedKind::Label => {
                        if let Some(l) = snapshot.labels.iter().find(|l| l.uuid == item.uuid) {
                            push_pt(l.position.x, l.position.y, 4.0);
                        }
                    }
                    SelectedKind::Junction | SelectedKind::NoConnect => {
                        if let Some(j) = snapshot.junctions.iter().find(|j| j.uuid == item.uuid) {
                            push_pt(j.position.x, j.position.y, 1.0);
                        } else if let Some(nc) =
                            snapshot.no_connects.iter().find(|n| n.uuid == item.uuid)
                        {
                            push_pt(nc.position.x, nc.position.y, 1.0);
                        }
                    }
                    SelectedKind::TextNote => {
                        if let Some(tn) = snapshot.text_notes.iter().find(|t| t.uuid == item.uuid) {
                            push_pt(tn.position.x, tn.position.y, 6.0);
                        }
                    }
                    SelectedKind::ChildSheet => {
                        if let Some(cs) = snapshot.child_sheets.iter().find(|c| c.uuid == item.uuid)
                        {
                            push_pt(cs.position.x, cs.position.y, 0.0);
                            push_pt(cs.position.x + cs.size.0, cs.position.y + cs.size.1, 0.0);
                        }
                    }
                    SelectedKind::Drawing => {
                        use signex_types::schematic::SchDrawing;
                        if let Some(d) = snapshot.drawings.iter().find(|d| {
                            let u = match d {
                                SchDrawing::Line { uuid, .. }
                                | SchDrawing::Rect { uuid, .. }
                                | SchDrawing::Circle { uuid, .. }
                                | SchDrawing::Arc { uuid, .. }
                                | SchDrawing::Polyline { uuid, .. } => *uuid,
                            };
                            u == item.uuid
                        }) {
                            match d {
                                SchDrawing::Line { start, end, .. } => {
                                    push_pt(start.x, start.y, 1.0);
                                    push_pt(end.x, end.y, 1.0);
                                }
                                SchDrawing::Rect { start, end, .. } => {
                                    push_pt(start.x, start.y, 1.0);
                                    push_pt(end.x, end.y, 1.0);
                                }
                                SchDrawing::Circle { center, radius, .. } => {
                                    push_pt(center.x, center.y, *radius as f32);
                                }
                                SchDrawing::Arc {
                                    start, mid, end, ..
                                } => {
                                    push_pt(start.x, start.y, 1.0);
                                    push_pt(mid.x, mid.y, 1.0);
                                    push_pt(end.x, end.y, 1.0);
                                }
                                SchDrawing::Polyline { points, .. } => {
                                    for p in points {
                                        push_pt(p.x, p.y, 1.0);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            if !xs.is_empty() && !ys.is_empty() {
                let min_x = xs.iter().cloned().fold(f32::INFINITY, f32::min);
                let max_x = xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                let min_y = ys.iter().cloned().fold(f32::INFINITY, f32::min);
                let max_y = ys.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                let p_min = state
                    .camera
                    .world_to_screen(iced::Point::new(min_x, min_y), bounds);
                let p_max = state
                    .camera
                    .world_to_screen(iced::Point::new(max_x, max_y), bounds);
                let margin = 30.0_f32;
                let sx0 = p_min.x.min(p_max.x) - margin;
                let sy0 = p_min.y.min(p_max.y) - margin;
                let sx1 = p_min.x.max(p_max.x) + margin;
                let sy1 = p_min.y.max(p_max.y) + margin;
                let dim = iced::Color::from_rgba(0.05, 0.05, 0.08, 0.55);
                let dim_frame = {
                    let mut f = canvas::Frame::new(renderer, bounds.size());
                    let bw = bounds.width;
                    let bh = bounds.height;
                    // Top
                    f.fill_rectangle(
                        iced::Point::new(0.0, 0.0),
                        iced::Size::new(bw, sy0.max(0.0)),
                        dim,
                    );
                    // Bottom
                    f.fill_rectangle(
                        iced::Point::new(0.0, sy1.min(bh)),
                        iced::Size::new(bw, (bh - sy1).max(0.0)),
                        dim,
                    );
                    // Left
                    let mid_h = (sy1.min(bh) - sy0.max(0.0)).max(0.0);
                    f.fill_rectangle(
                        iced::Point::new(0.0, sy0.max(0.0)),
                        iced::Size::new(sx0.max(0.0), mid_h),
                        dim,
                    );
                    // Right
                    f.fill_rectangle(
                        iced::Point::new(sx1.min(bw), sy0.max(0.0)),
                        iced::Size::new((bw - sx1).max(0.0), mid_h),
                        dim,
                    );
                    f.into_geometry()
                };
                layers.push(dim_frame);
            }
        }

        // Layer 3: selection overlay — always uses live camera (redrawn each frame)
        // During drag we use the shifted snapshot so the selection rectangle
        // travels with the dragged items instead of staying behind.
        if !self.selected.is_empty()
            && let Some(snapshot) = effective_snapshot
        {
            // During a drag we can't rely on overlay_cache — it must redraw
            // with the shifted positions every frame.
            let draw_overlay = |frame: &mut canvas::Frame| {
                let transform = signex_render::schematic::ScreenTransform {
                    offset_x: state.camera.offset.x,
                    offset_y: state.camera.offset.y,
                    scale: state.camera.scale,
                };
                signex_render::schematic::selection::draw_selection_overlay(
                    frame,
                    snapshot,
                    &self.selected,
                    &transform,
                );
                // Altium-style ERC markers: filled circle + concentric
                // stroke at each violation's world position, colored by
                // severity. Drawn in the overlay layer so they sit on
                // top of wires and symbols but under the selection
                // highlight.
                for m in &self.erc_markers {
                    let (sx, sy) = transform.world_to_screen(m.x, m.y);
                    let (fill, stroke) = match m.severity {
                        ErcMarkerSeverity::Error => (
                            Color::from_rgba(0.95, 0.25, 0.25, 0.6),
                            Color::from_rgb(0.95, 0.25, 0.25),
                        ),
                        ErcMarkerSeverity::Warning => (
                            Color::from_rgba(0.98, 0.72, 0.20, 0.6),
                            Color::from_rgb(0.98, 0.72, 0.20),
                        ),
                        ErcMarkerSeverity::Info => (
                            Color::from_rgba(0.30, 0.55, 0.85, 0.55),
                            Color::from_rgb(0.30, 0.55, 0.85),
                        ),
                    };
                    // Outer soft halo so the marker is visible at low zoom.
                    let halo = canvas::Path::circle(iced::Point::new(sx, sy), 16.0);
                    frame.fill(&halo, Color::from_rgba(fill.r, fill.g, fill.b, 0.18));
                    // Core dot — filled bright + hard stroke so it stays
                    // legible over wires and text.
                    let dot = canvas::Path::circle(iced::Point::new(sx, sy), 7.0);
                    frame.fill(&dot, fill);
                    frame.stroke(
                        &dot,
                        canvas::Stroke::default().with_color(stroke).with_width(2.0),
                    );
                    // Cross-hair inside the dot (Altium's "X" marker).
                    let cross_len = 4.0_f32;
                    let v1 = canvas::Path::line(
                        iced::Point::new(sx - cross_len, sy - cross_len),
                        iced::Point::new(sx + cross_len, sy + cross_len),
                    );
                    let v2 = canvas::Path::line(
                        iced::Point::new(sx - cross_len, sy + cross_len),
                        iced::Point::new(sx + cross_len, sy - cross_len),
                    );
                    let white = Color::WHITE;
                    frame.stroke(
                        &v1,
                        canvas::Stroke::default().with_color(white).with_width(1.5),
                    );
                    frame.stroke(
                        &v2,
                        canvas::Stroke::default().with_color(white).with_width(1.5),
                    );
                }
            };
            if drag_offset.is_some() {
                let mut frame = canvas::Frame::new(renderer, bounds.size());
                draw_overlay(&mut frame);
                layers.push(frame.into_geometry());
            } else {
                let sel_overlay = self
                    .overlay_cache
                    .draw(renderer, bounds.size(), draw_overlay);
                layers.push(sel_overlay);
            }
        }

        // Layer 4: overlay (cursor crosshair — redrawn every frame)
        let overlay = {
            let mut frame = canvas::Frame::new(renderer, bounds.size());

            if let Some(cursor_pos) = cursor.position_in(bounds) {
                // Snap cursor visuals to the grid so they match where
                // the click will commit.
                let cursor_pos = if self.snap_enabled && self.snap_grid_mm > 0.0 {
                    let w = state.camera.screen_to_world(cursor_pos, bounds);
                    let g = self.snap_grid_mm as f32;
                    let snapped_w = iced::Point::new((w.x / g).round() * g, (w.y / g).round() * g);
                    state.camera.world_to_screen(snapped_w, bounds)
                } else {
                    cursor_pos
                };

                // Altium-style placement crosshair: a cyan diagonal
                // X at the cursor, ~28 px across (double the earlier
                // +). Same shape, size, and colour everywhere a
                // placement / tool mode is active so the cursor
                // affordance is uniform.
                let placement_active = self.pending_net_color.is_some()
                    || self.lasso_polygon.is_some()
                    || self.drawing_mode
                    || self.tool_preview.is_some()
                    || self.ghost_label.is_some()
                    || self.ghost_symbol.is_some()
                    || self.ghost_text.is_some()
                    || !self.arc_points.is_empty()
                    || !self.polyline_points.is_empty()
                    || self.reorder_picker_armed
                    || self.shape_anchor.is_some();
                if placement_active {
                    let len = 14.0_f32;
                    let a = canvas::Path::line(
                        iced::Point::new(cursor_pos.x - len, cursor_pos.y - len),
                        iced::Point::new(cursor_pos.x + len, cursor_pos.y + len),
                    );
                    let b = canvas::Path::line(
                        iced::Point::new(cursor_pos.x - len, cursor_pos.y + len),
                        iced::Point::new(cursor_pos.x + len, cursor_pos.y - len),
                    );
                    // Plain gray X — no outline. Neutral so it reads
                    // on both the dark canvas background and the
                    // yellow paper fill without competing with the
                    // theme's accent colours.
                    let stroke = canvas::Stroke::default()
                        .with_color(Color::from_rgba(0.55, 0.55, 0.58, 0.9))
                        .with_width(1.5);
                    frame.stroke(&a, stroke);
                    frame.stroke(&b, stroke);
                }

                // Net-color pen affordance — a diagonal "pencil" mark
                // anchored to the cursor, filled with the armed color.
                // iced's mouse cursor set only exposes Crosshair, so we
                // paint our own pencil glyph on the canvas to make the
                // mode visually obvious.
                if let Some(c) = self.pending_net_color {
                    let body = if c.a == 0 {
                        // Clear-mode sentinel — render a grey pencil so
                        // the user still sees the armed state.
                        Color::from_rgb(0.75, 0.75, 0.75)
                    } else {
                        Color::from_rgb8(c.r, c.g, c.b)
                    };
                    let tip = iced::Point::new(cursor_pos.x + 4.0, cursor_pos.y + 4.0);
                    let butt = iced::Point::new(cursor_pos.x + 22.0, cursor_pos.y + 22.0);
                    // Shaft (fat colored line)
                    let shaft = canvas::Path::line(tip, butt);
                    frame.stroke(
                        &shaft,
                        canvas::Stroke::default().with_color(body).with_width(6.0),
                    );
                    // Dark outline for contrast on light backgrounds
                    frame.stroke(
                        &shaft,
                        canvas::Stroke::default()
                            .with_color(Color::from_rgba(0.0, 0.0, 0.0, 0.45))
                            .with_width(1.0),
                    );
                    // Small triangle at tip to look like a pencil nib
                    let nib = canvas::Path::new(|b| {
                        b.move_to(tip);
                        b.line_to(iced::Point::new(tip.x + 3.0, tip.y - 2.0));
                        b.line_to(iced::Point::new(tip.x - 2.0, tip.y + 3.0));
                        b.close();
                    });
                    frame.fill(&nib, Color::from_rgb(0.15, 0.15, 0.15));
                }

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
                    let p_a = state.camera.world_to_screen(
                        iced::Point::new(anchor.x as f32, anchor.y as f32),
                        bounds,
                    );
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

                // Polyline-in-progress preview — solid segments between
                // committed vertices plus a dashed rubber-band to the
                // snapped cursor. Commits on Enter or double-click.
                if !self.polyline_points.is_empty() {
                    let accent = Color::from_rgb(0.94, 0.74, 0.28);
                    let stroke = canvas::Stroke::default().with_color(accent).with_width(1.5);
                    for pair in self.polyline_points.windows(2) {
                        let p1 = state.camera.world_to_screen(
                            iced::Point::new(pair[0].x as f32, pair[0].y as f32),
                            bounds,
                        );
                        let p2 = state.camera.world_to_screen(
                            iced::Point::new(pair[1].x as f32, pair[1].y as f32),
                            bounds,
                        );
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
                        let p1 = state.camera.world_to_screen(
                            iced::Point::new(last.x as f32, last.y as f32),
                            bounds,
                        );
                        let p2 = state.camera.world_to_screen(
                            iced::Point::new(snap_x as f32, snap_y as f32),
                            bounds,
                        );
                        let dashed = canvas::Stroke::default()
                            .with_color(Color { a: 0.6, ..accent })
                            .with_width(1.0);
                        frame.stroke(&canvas::Path::line(p1, p2), dashed);
                    }
                }

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
                        let p1 = state.camera.world_to_screen(
                            iced::Point::new(last.x as f32, last.y as f32),
                            bounds,
                        );
                        let p2 = state.camera.world_to_screen(
                            iced::Point::new(snap_x as f32, snap_y as f32),
                            bounds,
                        );
                        frame.stroke(&canvas::Path::line(p1, p2), dashed);
                    }
                }

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
                        let p1 = state.camera.world_to_screen(
                            iced::Point::new(pair[0].x as f32, pair[0].y as f32),
                            bounds,
                        );
                        let p2 = state.camera.world_to_screen(
                            iced::Point::new(pair[1].x as f32, pair[1].y as f32),
                            bounds,
                        );
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
                        let p1 = state.camera.world_to_screen(
                            iced::Point::new(last.x as f32, last.y as f32),
                            bounds,
                        );
                        let p2 = state.camera.world_to_screen(
                            iced::Point::new(snap_x as f32, snap_y as f32),
                            bounds,
                        );
                        let dashed = canvas::Stroke::default()
                            .with_color(Color { a: 0.6, ..accent })
                            .with_width(1.0);
                        frame.stroke(&canvas::Path::line(p1, p2), dashed);
                    }
                    // Small circle at the first vertex to hint
                    // "click here to close the polygon".
                    if lasso.len() >= 3 {
                        let first = lasso[0];
                        let p = state.camera.world_to_screen(
                            iced::Point::new(first.x as f32, first.y as f32),
                            bounds,
                        );
                        let ring = canvas::Path::circle(p, 5.0);
                        frame.stroke(
                            &ring,
                            canvas::Stroke::default().with_color(accent).with_width(1.5),
                        );
                    }
                }

                // Wire-in-progress rubber-band preview
                if self.drawing_mode && !self.wire_preview.is_empty() {
                    let wire_color = self.canvas_colors.wire;
                    let wire_color_iced = signex_render::colors::to_iced(&wire_color);
                    // Match the placed-wire stroke width (0.15 mm in world),
                    // scaled by camera. Previously fixed 1.5 px which looked
                    // thin at higher zooms.
                    let placed_width = (state.camera.scale * 0.15).max(1.0);
                    let preview_stroke = canvas::Stroke::default()
                        .with_color(wire_color_iced)
                        .with_width(placed_width);

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
                                    let corner =
                                        signex_types::schematic::Point::new(end.x, start.y);
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
                                                signex_types::schematic::Point::new(
                                                    end.x, diag_end.y,
                                                ),
                                            ),
                                        ]
                                    } else {
                                        vec![
                                            (start, diag_end),
                                            (
                                                diag_end,
                                                signex_types::schematic::Point::new(
                                                    diag_end.x, end.y,
                                                ),
                                            ),
                                        ]
                                    }
                                } else {
                                    let corner =
                                        signex_types::schematic::Point::new(end.x, start.y);
                                    vec![(start, corner), (corner, end)]
                                }
                            }
                        };

                        for (p1, p2) in &segments {
                            let s1 = state.camera.world_to_screen(
                                iced::Point::new(p1.x as f32, p1.y as f32),
                                bounds,
                            );
                            let s2 = state.camera.world_to_screen(
                                iced::Point::new(p2.x as f32, p2.y as f32),
                                bounds,
                            );
                            frame.stroke(&canvas::Path::line(s1, s2), rubber_stroke);
                        }
                    }
                }

                // Ghost power-port symbol preview at cursor position.
                // While placement is paused (TAB → properties form open),
                // hide the ghosts so the user isn't distracted by a preview
                // that can't be committed until they confirm.
                if let Some(ref ghost_sym) = self.ghost_symbol
                    && !self.placement_paused
                {
                    let cursor_world = state.camera.screen_to_world(cursor_pos, bounds);
                    let (sx, sy) = if self.snap_enabled && self.snap_grid_mm > 0.0 {
                        let g = self.snap_grid_mm;
                        (
                            (cursor_world.x as f64 / g).round() * g,
                            (cursor_world.y as f64 / g).round() * g,
                        )
                    } else {
                        (cursor_world.x as f64, cursor_world.y as f64)
                    };
                    let mut preview = ghost_sym.clone();
                    preview.position = signex_types::schematic::Point::new(sx, sy);
                    let ghost_transform = signex_render::schematic::ScreenTransform {
                        offset_x: state.camera.offset.x,
                        offset_y: state.camera.offset.y,
                        scale: state.camera.scale,
                    };
                    let ghost_color = Color::from_rgba(0.3, 0.8, 1.0, 0.7);
                    signex_render::schematic::draw_power_port_preview(
                        &mut frame,
                        &preview,
                        &ghost_transform,
                        ghost_color,
                    );
                }

                // Ghost text-note preview at cursor position.
                if let Some(ref ghost_tn) = self.ghost_text
                    && !self.placement_paused
                {
                    let cursor_world = state.camera.screen_to_world(cursor_pos, bounds);
                    let (sx, sy) = if self.snap_enabled && self.snap_grid_mm > 0.0 {
                        let g = self.snap_grid_mm;
                        (
                            (cursor_world.x as f64 / g).round() * g,
                            (cursor_world.y as f64 / g).round() * g,
                        )
                    } else {
                        (cursor_world.x as f64, cursor_world.y as f64)
                    };
                    let mut preview = ghost_tn.clone();
                    preview.position = signex_types::schematic::Point::new(sx, sy);
                    let ghost_transform = signex_render::schematic::ScreenTransform {
                        offset_x: state.camera.offset.x,
                        offset_y: state.camera.offset.y,
                        scale: state.camera.scale,
                    };
                    let ghost_color = Color::from_rgba(0.3, 0.8, 1.0, 0.7);
                    signex_render::schematic::text::draw_text_note(
                        &mut frame,
                        &preview,
                        &ghost_transform,
                        ghost_color,
                    );
                }

                // Ghost label/port preview at cursor position
                if let Some(ref ghost) = self.ghost_label
                    && !self.placement_paused
                {
                    let cursor_world = state.camera.screen_to_world(cursor_pos, bounds);
                    let snap_world = if self.snap_enabled && self.snap_grid_mm > 0.0 {
                        let g = self.snap_grid_mm;
                        (
                            (cursor_world.x as f64 / g).round() * g,
                            (cursor_world.y as f64 / g).round() * g,
                        )
                    } else {
                        (cursor_world.x as f64, cursor_world.y as f64)
                    };
                    let mut preview_label = ghost.clone();
                    preview_label.position =
                        signex_types::schematic::Point::new(snap_world.0, snap_world.1);
                    let ghost_transform = signex_render::schematic::ScreenTransform {
                        offset_x: state.camera.offset.x,
                        offset_y: state.camera.offset.y,
                        scale: state.camera.scale,
                    };
                    let ghost_color = Color::from_rgba(0.3, 0.8, 1.0, 0.7);
                    let ghost_fill = Color::from_rgba(0.3, 0.8, 1.0, 0.15);
                    signex_render::schematic::label::draw_label(
                        &mut frame,
                        &preview_label,
                        &ghost_transform,
                        ghost_color,
                        ghost_fill,
                    );
                }

                // Tool-specific cursor marker: a bright X that locks onto
                // the grid dot the next click will commit to (when snap is
                // enabled), so the user can see exactly where the wire/bus
                // endpoint will land. Altium's placement tag follows the
                // snapped point, not the raw cursor.
                //
                // Only show the X for "line-drawing" tools (wire/bus/etc.)
                // that DON'T already have a ghost preview of what's being
                // placed — the ghost shows the click target for those,
                // and doubling up with an X clutters the cursor.
                let has_ghost = self.ghost_label.is_some()
                    || self.ghost_symbol.is_some()
                    || self.ghost_text.is_some();
                if let Some(ref label) = self.tool_preview
                    && !has_ghost
                {
                    let snapped_screen = if self.snap_enabled && self.snap_grid_mm > 0.0 {
                        let world = state.camera.screen_to_world(cursor_pos, bounds);
                        let g = self.snap_grid_mm as f32;
                        let sx = (world.x / g).round() * g;
                        let sy = (world.y / g).round() * g;
                        state
                            .camera
                            .world_to_screen(iced::Point::new(sx, sy), bounds)
                    } else {
                        cursor_pos
                    };
                    // No cyan X here — the unified gray placement X
                    // painted earlier already sits at the cursor for
                    // every tool. Keep only the tool-name chip.
                    // Tool-name tag beside the marker. Dark text on a
                    // semi-opaque light chip so it reads on any canvas bg.
                    let tag_x = snapped_screen.x + 14.0;
                    let tag_y = snapped_screen.y - 16.0;
                    let tag_w = (label.chars().count() as f32) * 7.0 + 10.0;
                    let tag_h = 16.0;
                    let chip = canvas::Path::rectangle(
                        iced::Point::new(tag_x - 2.0, tag_y - 2.0),
                        iced::Size::new(tag_w, tag_h),
                    );
                    frame.fill(&chip, Color::from_rgba(0.0, 0.0, 0.0, 0.65));
                    frame.fill_text(canvas::Text {
                        content: label.clone(),
                        position: iced::Point::new(tag_x + 3.0, tag_y + 1.0),
                        color: Color::from_rgba(1.0, 1.0, 1.0, 0.95),
                        size: iced::Pixels(11.0),
                        font: signex_render::IOSEVKA,
                        ..canvas::Text::default()
                    });
                }
            }

            // Drag-to-move: the content layer already renders selected items
            // at the dragged offset (via shifted_snapshot). Here we just handle
            // the symbol-field anchor→moved guide line, which the content
            // render doesn't draw on its own.
            if state.move_dragging
                && let (Some(origin), Some(current)) = (state.move_origin, state.move_current)
            {
                let dx = (current.0 - origin.0) as f32;
                let dy = (current.1 - origin.1) as f32;
                if let Some(render_cache) = self.active_render_cache() {
                    let preview = render_cache.prepared_preview();
                    for sel in &self.selected {
                        if matches!(
                            sel.kind,
                            signex_types::schematic::SelectedKind::SymbolRefField
                                | signex_types::schematic::SelectedKind::SymbolValField
                        ) {
                            let anchor_pos = preview.symbol_position(sel.uuid);
                            let moved_pos = match sel.kind {
                                signex_types::schematic::SelectedKind::SymbolRefField => {
                                    preview.symbol_reference_position(sel.uuid)
                                }
                                signex_types::schematic::SelectedKind::SymbolValField => {
                                    preview.symbol_value_position(sel.uuid)
                                }
                                _ => None,
                            };

                            if let (Some((anchor_x, anchor_y)), Some((field_x, field_y))) =
                                (anchor_pos, moved_pos)
                            {
                                let anchor = state
                                    .camera
                                    .world_to_screen(iced::Point::new(anchor_x, anchor_y), bounds);
                                let moved = state.camera.world_to_screen(
                                    iced::Point::new(field_x + dx, field_y + dy),
                                    bounds,
                                );

                                let guide = canvas::Path::line(anchor, moved);
                                frame.stroke(
                                    &guide,
                                    canvas::Stroke::default()
                                        .with_color(Color::from_rgb(0.3, 0.7, 1.0))
                                        .with_width(1.0),
                                );

                                let anchor_circle = canvas::Path::circle(anchor, 3.0);
                                frame.fill(&anchor_circle, Color::from_rgb(0.3, 0.7, 1.0));
                            }
                        }
                    }
                }

                // Altium-style connection-point markers: small thin X at every
                // pin / wire-end / junction position of the dragged objects.
                let snapshot_live = self.active_snapshot();
                if let Some(snap) = snapshot_live {
                    let x_color = Color::from_rgb(1.0, 0.3, 0.3);
                    let x_stroke = canvas::Stroke::default()
                        .with_color(x_color)
                        .with_width(1.0);
                    let draw_x = |frame: &mut canvas::Frame, screen: iced::Point| {
                        let r = 4.0;
                        frame.stroke(
                            &canvas::Path::line(
                                iced::Point::new(screen.x - r, screen.y - r),
                                iced::Point::new(screen.x + r, screen.y + r),
                            ),
                            x_stroke,
                        );
                        frame.stroke(
                            &canvas::Path::line(
                                iced::Point::new(screen.x - r, screen.y + r),
                                iced::Point::new(screen.x + r, screen.y - r),
                            ),
                            x_stroke,
                        );
                    };
                    for sel in &self.selected {
                        use signex_types::schematic::{Point, SelectedKind};
                        let dxf = dx as f64;
                        let dyf = dy as f64;
                        match sel.kind {
                            SelectedKind::Wire => {
                                if let Some(w) = snap.wires.iter().find(|w| w.uuid == sel.uuid) {
                                    for p in [w.start, w.end] {
                                        let s = state.camera.world_to_screen(
                                            iced::Point::new(
                                                (p.x + dxf) as f32,
                                                (p.y + dyf) as f32,
                                            ),
                                            bounds,
                                        );
                                        draw_x(&mut frame, s);
                                    }
                                }
                            }
                            SelectedKind::Bus => {
                                if let Some(b) = snap.buses.iter().find(|b| b.uuid == sel.uuid) {
                                    for p in [b.start, b.end] {
                                        let s = state.camera.world_to_screen(
                                            iced::Point::new(
                                                (p.x + dxf) as f32,
                                                (p.y + dyf) as f32,
                                            ),
                                            bounds,
                                        );
                                        draw_x(&mut frame, s);
                                    }
                                }
                            }
                            SelectedKind::Junction => {
                                if let Some(j) = snap.junctions.iter().find(|j| j.uuid == sel.uuid)
                                {
                                    let s = state.camera.world_to_screen(
                                        iced::Point::new(
                                            (j.position.x + dxf) as f32,
                                            (j.position.y + dyf) as f32,
                                        ),
                                        bounds,
                                    );
                                    draw_x(&mut frame, s);
                                }
                            }
                            SelectedKind::Label => {
                                if let Some(l) = snap.labels.iter().find(|l| l.uuid == sel.uuid) {
                                    let s = state.camera.world_to_screen(
                                        iced::Point::new(
                                            (l.position.x + dxf) as f32,
                                            (l.position.y + dyf) as f32,
                                        ),
                                        bounds,
                                    );
                                    draw_x(&mut frame, s);
                                }
                            }
                            SelectedKind::NoConnect => {
                                if let Some(nc) =
                                    snap.no_connects.iter().find(|n| n.uuid == sel.uuid)
                                {
                                    let s = state.camera.world_to_screen(
                                        iced::Point::new(
                                            (nc.position.x + dxf) as f32,
                                            (nc.position.y + dyf) as f32,
                                        ),
                                        bounds,
                                    );
                                    draw_x(&mut frame, s);
                                }
                            }
                            SelectedKind::Symbol => {
                                if let Some(sym) = snap.symbols.iter().find(|s| s.uuid == sel.uuid)
                                    && let Some(lib_sym) = snap.lib_symbols.get(&sym.lib_id)
                                {
                                    // Build a shifted copy so instance_transform
                                    // uses the dragged position.
                                    let mut shifted = sym.clone();
                                    shifted.position =
                                        Point::new(sym.position.x + dxf, sym.position.y + dyf);
                                    for lp in &lib_sym.pins {
                                        if lp.unit != 0 && lp.unit != sym.unit {
                                            continue;
                                        }
                                        let p = &lp.pin;
                                        let (wx, wy) = signex_render::schematic::instance_transform(
                                            &shifted,
                                            &p.position,
                                        );
                                        let s = state.camera.world_to_screen(
                                            iced::Point::new(wx as f32, wy as f32),
                                            bounds,
                                        );
                                        draw_x(&mut frame, s);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Drag-to-select rectangle
            if let (Some(start), Some(end)) = (state.select_drag_start, state.select_drag_end) {
                let s1 = state
                    .camera
                    .world_to_screen(iced::Point::new(start.0 as f32, start.1 as f32), bounds);
                let s2 = state
                    .camera
                    .world_to_screen(iced::Point::new(end.0 as f32, end.1 as f32), bounds);
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
                    let rect_path =
                        canvas::Path::rectangle(iced::Point::new(x, y), iced::Size::new(w, h));
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
        // Single cursor shape across every canvas mode: default arrow
        // for idle, native pan / move cursors while dragging. No
        // mode-specific shape swaps (the OS Crosshair is white + tiny
        // on Windows so invisible on yellow paper anyway). Visual
        // feedback for armed modes comes from overlay glyphs — the
        // net-colour pencil and the lasso polygon preview.
        if state.panning {
            mouse::Interaction::Grabbing
        } else if state.move_dragging {
            mouse::Interaction::Move
        } else {
            mouse::Interaction::default()
        }
    }
}

// ─── Active Bar hit detection for right-click ────────────────

/// Map a relative x position within the Active Bar to a dropdown menu.
/// Returns None for buttons without dropdowns (Select, Add Component).
/// Clone the snapshot and translate the world position of every item that
/// appears in `selection` by `(dx, dy)`. Used during drag-to-move so the live
/// render shows selected objects at the cursor position while the originals
/// are not drawn at their pre-drag location.
fn shift_snapshot_for_selection(
    snap: &signex_render::schematic::SchematicRenderSnapshot,
    selection: &[signex_types::schematic::SelectedItem],
    dx: f64,
    dy: f64,
) -> signex_render::schematic::SchematicRenderSnapshot {
    use signex_types::schematic::{Point, SelectedKind};

    let is_selected = |uuid: uuid::Uuid, kind: SelectedKind| -> bool {
        selection.iter().any(|s| s.uuid == uuid && s.kind == kind)
    };
    let shift = |p: Point| -> Point { Point::new(p.x + dx, p.y + dy) };

    let mut out = snap.clone();
    for w in out.wires.iter_mut() {
        if is_selected(w.uuid, SelectedKind::Wire) {
            w.start = shift(w.start);
            w.end = shift(w.end);
        }
    }
    for b in out.buses.iter_mut() {
        if is_selected(b.uuid, SelectedKind::Bus) {
            b.start = shift(b.start);
            b.end = shift(b.end);
        }
    }
    for be in out.bus_entries.iter_mut() {
        if is_selected(be.uuid, SelectedKind::BusEntry) {
            be.position = shift(be.position);
        }
    }
    for j in out.junctions.iter_mut() {
        if is_selected(j.uuid, SelectedKind::Junction) {
            j.position = shift(j.position);
        }
    }
    for nc in out.no_connects.iter_mut() {
        if is_selected(nc.uuid, SelectedKind::NoConnect) {
            nc.position = shift(nc.position);
        }
    }
    for l in out.labels.iter_mut() {
        if is_selected(l.uuid, SelectedKind::Label) {
            l.position = shift(l.position);
        }
    }
    for tn in out.text_notes.iter_mut() {
        if is_selected(tn.uuid, SelectedKind::TextNote) {
            tn.position = shift(tn.position);
        }
    }
    for sym in out.symbols.iter_mut() {
        if is_selected(sym.uuid, SelectedKind::Symbol) {
            // Whole-symbol drag: anchor + both fields travel together.
            sym.position = shift(sym.position);
            if let Some(rt) = &mut sym.ref_text {
                rt.position = shift(rt.position);
            }
            if let Some(vt) = &mut sym.val_text {
                vt.position = shift(vt.position);
            }
        } else {
            // Field-only drag: shift just the selected field; the symbol
            // body stays put so the user can reposition the label
            // independently (Altium convention).
            if is_selected(sym.uuid, SelectedKind::SymbolRefField)
                && let Some(rt) = &mut sym.ref_text
            {
                rt.position = shift(rt.position);
            }
            if is_selected(sym.uuid, SelectedKind::SymbolValField)
                && let Some(vt) = &mut sym.val_text
            {
                vt.position = shift(vt.position);
            }
        }
    }
    for cs in out.child_sheets.iter_mut() {
        if is_selected(cs.uuid, SelectedKind::ChildSheet) {
            cs.position = shift(cs.position);
        }
    }
    use signex_types::schematic::SchDrawing;
    for d in out.drawings.iter_mut() {
        let uuid = match d {
            SchDrawing::Line { uuid, .. }
            | SchDrawing::Rect { uuid, .. }
            | SchDrawing::Circle { uuid, .. }
            | SchDrawing::Arc { uuid, .. }
            | SchDrawing::Polyline { uuid, .. } => *uuid,
        };
        if !is_selected(uuid, SelectedKind::Drawing) {
            continue;
        }
        match d {
            SchDrawing::Line { start, end, .. } => {
                *start = shift(*start);
                *end = shift(*end);
            }
            SchDrawing::Rect { start, end, .. } => {
                *start = shift(*start);
                *end = shift(*end);
            }
            SchDrawing::Circle { center, .. } => {
                *center = shift(*center);
            }
            SchDrawing::Arc {
                start, mid, end, ..
            } => {
                *start = shift(*start);
                *mid = shift(*mid);
                *end = shift(*end);
            }
            SchDrawing::Polyline { points, .. } => {
                for p in points {
                    *p = shift(*p);
                }
            }
        }
    }
    out
}

fn active_bar_hit(x: f32) -> Option<crate::active_bar::ActiveBarMenu> {
    use crate::active_bar::ActiveBarMenu;
    // Each btn=29px (28 cell + 1 spacing), sep=2px, pad=4px.
    // [Filter][Move] | [Select][Align] | [Wire][Power] | [Harness][Sheet][Port][Dir] | [Text][Shapes][NetColor]
    let x = x - 4.0;
    let b = 29; // button width (cell + spacing)
    let s = 2; // separator
    let xi = x as i32;
    if xi < 0 {
        return None;
    }
    // btn 0: Filter
    if xi < b {
        return Some(ActiveBarMenu::Filter);
    }
    // btn 1: Move
    if xi < 2 * b {
        return Some(ActiveBarMenu::Select);
    }
    // sep
    let off = 2 * b + s;
    // btn 2: Select
    if xi >= off && xi < off + b {
        return Some(ActiveBarMenu::SelectMode);
    }
    // btn 3: Align
    if xi >= off + b && xi < off + 2 * b {
        return Some(ActiveBarMenu::Align);
    }
    // sep
    let off = off + 2 * b + s;
    // btn 4: Wire
    if xi >= off && xi < off + b {
        return Some(ActiveBarMenu::Wiring);
    }
    // btn 5: Power
    if xi >= off + b && xi < off + 2 * b {
        return Some(ActiveBarMenu::Power);
    }
    // sep
    let off = off + 2 * b + s;
    // btn 6: Harness
    if xi >= off && xi < off + b {
        return Some(ActiveBarMenu::Harness);
    }
    // btn 7: Sheet Symbol
    if xi >= off + b && xi < off + 2 * b {
        return Some(ActiveBarMenu::SheetSymbol);
    }
    // btn 8: Port
    if xi >= off + 2 * b && xi < off + 3 * b {
        return Some(ActiveBarMenu::Port);
    }
    // btn 9: Directives
    if xi >= off + 3 * b && xi < off + 4 * b {
        return Some(ActiveBarMenu::Directives);
    }
    // sep
    let off = off + 4 * b + s;
    // btn 10: Text
    if xi >= off && xi < off + b {
        return Some(ActiveBarMenu::TextTools);
    }
    // btn 11: Shapes
    if xi >= off + b && xi < off + 2 * b {
        return Some(ActiveBarMenu::Shapes);
    }
    // btn 12: Net Color
    if xi >= off + 2 * b && xi < off + 3 * b {
        return Some(ActiveBarMenu::NetColor);
    }
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
        screen_x: f32,
        screen_y: f32,
    },
    /// Box selection — select all items within the rectangle (world coords).
    BoxSelect {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
    /// Drag-move completed — move all selected items by delta (world coords).
    MoveSelected {
        dx: f64,
        dy: f64,
    },
}
