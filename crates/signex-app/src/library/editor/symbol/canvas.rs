//! Symbol-tab interactive canvas.
//!
//! The canvas reads the typed [`signex_library::Symbol`] primitive
//! directly. The body rectangle is derived from `Symbol.graphics`
//! (first `Rectangle` graphic), or defaults to a
//! `[-5.08, -2.54] .. [5.08, 2.54]` rectangle when the primitive
//! carries no body geometry yet.
//!
//! World-space convention mirrors the schematic editor: Standard y-axis
//! (positive going up; on screen y goes down so we flip). The
//! camera ([`crate::canvas::Camera`]) handles pan/zoom; the user
//! pans with right- or middle-button drag and zooms with the wheel.
//! Press Home (or click the Fit button) to fit the symbol bbox to
//! the viewport — also the implicit state on tab open.
//!
//! Background colour, grid size + visibility, snap, and the cursor
//! coordinate readout follow the same Altium-parity surface as the
//! schematic canvas: bg + grid colour come from the active theme's
//! `CanvasColors`; grid spacing follows `panel_ctx.grid_size_mm`;
//! the unit ([`signex_types::coord::Unit`]) drives the status
//! footer. Sheet colour is per-tab (Altium "Document Options")
//! and shifts the bg fill alpha so the user can pick Black / White
//! / Dark Gray / Light Gray / Cream per-symbol library.

use iced::Color;
use iced::Rectangle;
use iced::Renderer;
use iced::Size;
use iced::Theme;
use iced::event::Event;
use iced::mouse;
use iced::widget::canvas;
use signex_library::{Symbol, SymbolGraphicKind, SymbolPin};

use super::state::{self, GraphicHandle, SymbolSelection};

/// The actions a [`SymbolCanvas`] can emit upward.
#[derive(Debug, Clone, Copy)]
pub enum CanvasAction {
    AddPin {
        x: f64,
        y: f64,
    },
    /// Stamp a default-sized rectangle (10 × 5 mm) centred on
    /// `(x, y)`. Drag-to-resize lands in a follow-up — for the
    /// first cut the rectangle is committed in one click and
    /// the user can later edit the corners via the Properties
    /// panel (or move/delete via the Select tool).
    AddRectangle {
        x: f64,
        y: f64,
    },
    /// Stamp a 5 mm horizontal line starting at `(x, y)` going
    /// right.
    AddLine {
        x: f64,
        y: f64,
    },
    /// Stamp a circle of radius 2 mm centred on `(x, y)`.
    AddCircle {
        x: f64,
        y: f64,
    },
    /// Stamp a default 2 mm-radius arc centred on `(x, y)` sweeping
    /// 0°→90° (quadrant arc).
    AddArc {
        x: f64,
        y: f64,
    },
    /// Stamp a default text label "Text" anchored at `(x, y)`.
    AddText {
        x: f64,
        y: f64,
    },
    Select(SymbolSelection),
    Deselect,
    Move {
        x: f64,
        y: f64,
    },
    /// Drag-to-resize a graphic handle. Fired continuously while the
    /// user drags the handle of a placed graphic in the Select tool.
    MoveGraphicHandle {
        idx: usize,
        handle: GraphicHandle,
        x: f64,
        y: f64,
    },
    DeleteSelected,
    // ── View / camera ──
    /// Pan the camera by `(dx, dy)` screen pixels. Fired by right-
    /// or middle-button drag.
    Pan {
        dx: f32,
        dy: f32,
    },
    /// Zoom centred on `(sx, sy)` (canvas-local pixels). Positive
    /// `delta` zooms in.
    Zoom {
        sx: f32,
        sy: f32,
        delta: f32,
    },
    /// Fit the symbol bbox into the viewport (Home key).
    Fit,
    /// Cursor world position update — drives the status footer.
    /// `None` clears the readout when the cursor leaves bounds.
    CursorAt {
        x_mm: Option<f64>,
        y_mm: Option<f64>,
    },
}

/// Canvas tools — Altium-style `Tool` enum scoped to this surface.
/// Mirrors the SchLib Place menu: Pin / Line / Rectangle / Ellipse
/// (Circle) / Arc / Text are the working tools; `Polygon` /
/// `RoundRectangle` / `Bezier` / `Image` etc. live on the Active
/// Bar as stubs and are deferred to v0.9.x.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolTool {
    Select,
    AddPin,
    PlaceRectangle,
    PlaceLine,
    PlaceCircle,
    PlaceArc,
    PlaceText,
}

impl SymbolTool {
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            SymbolTool::Select => "Select",
            SymbolTool::AddPin => "Add Pin",
            SymbolTool::PlaceRectangle => "Rectangle",
            SymbolTool::PlaceLine => "Line",
            SymbolTool::PlaceCircle => "Ellipse",
            SymbolTool::PlaceArc => "Arc",
            SymbolTool::PlaceText => "Text",
        }
    }
}

/// Canvas-program ephemeral state — drag + pan tracking.
#[derive(Debug, Default)]
pub struct CanvasState {
    /// True when the user is mid-drag of the currently-selected pin.
    pub dragging: bool,
    /// `(graphic_idx, handle)` while the user drags a graphic resize
    /// handle. `None` outside of a handle drag. Mutually exclusive
    /// with `dragging` — a click either lands on a pin or on a
    /// graphic handle, never both.
    pub dragging_handle: Option<(usize, GraphicHandle)>,
    /// True while the user holds right- or middle-button to pan.
    pub panning: bool,
    /// Last cursor screen position during a pan, used to compute
    /// per-frame deltas.
    pub last_pan_pos: Option<iced::Point>,
}

/// Builder for the per-render [`SymbolCanvas`] — all the inputs the
/// canvas needs from the surrounding state. The canvas itself is
/// constructed fresh on every iced view tick (see
/// `library/editor/standalone.rs::view_symbol_canvas`).
pub struct SymbolCanvas<'a> {
    pub symbol: &'a Symbol,
    pub selected: Option<SymbolSelection>,
    pub tool: SymbolTool,
    /// Active sub-part the canvas is filtering pins for. Pins with
    /// `part_number == 0` (Part Zero) render on every part; pins
    /// with `part_number == active_part` render on the active part
    /// only. Defaults to `1` (single-part components).
    pub active_part: u8,
    /// Pan/zoom state owned by the editor tab — see
    /// [`crate::app::SymbolEditorState::camera`].
    pub camera: &'a crate::canvas::Camera,
    /// Visible grid spacing in mm — sourced from
    /// `panel_ctx.grid_size_mm` so the schematic + library editors
    /// share the global grid setting.
    pub grid_size_mm: f64,
    /// Whether the grid is rendered. Sourced from
    /// `panel_ctx.grid_visible` (View ▸ Toggle Grid / status-bar
    /// click).
    pub grid_visible: bool,
    pub bg_color: Color,
    pub grid_color: Color,
    pub body_color: Color,
    pub pin_color: Color,
    pub selected_color: Color,
    pub text_color: Color,
}

impl<'a> SymbolCanvas<'a> {
    /// Construct the per-frame canvas with the inputs from
    /// `SymbolEditorState` + the active theme + global grid/unit
    /// settings. See module-level docs for the parity rationale.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        symbol: &'a Symbol,
        selected: Option<SymbolSelection>,
        tool: SymbolTool,
        active_part: u8,
        camera: &'a crate::canvas::Camera,
        grid_size_mm: f64,
        grid_visible: bool,
        sheet_color: Color,
        accent_color: Color,
        body_color: Color,
        text_color: Color,
        grid_color: Color,
    ) -> Self {
        Self {
            symbol,
            selected,
            tool,
            active_part,
            camera,
            grid_size_mm,
            grid_visible,
            bg_color: sheet_color,
            grid_color,
            body_color,
            pin_color: text_color,
            selected_color: accent_color,
            text_color,
        }
    }

    /// True when `pin` should render on the currently-active part.
    /// Part Zero (`part_number == 0`) appears on every part; other
    /// pins only render when they match `active_part`.
    fn pin_visible_on_active_part(&self, pin: &SymbolPin) -> bool {
        pin.part_number == 0 || pin.part_number == self.active_part
    }

    /// Body rectangle: derived from the first `SymbolGraphicKind::Rectangle`
    /// in `symbol.graphics`, or a sensible default.
    fn body_rect(&self) -> (f64, f64, f64, f64) {
        for g in &self.symbol.graphics {
            if let SymbolGraphicKind::Rectangle { from, to } = &g.kind {
                return (from[0], from[1], to[0], to[1]);
            }
        }
        (-5.08, -2.54, 5.08, 2.54)
    }

    /// Bounding box around the body + every pin + every graphic,
    /// with a generous pad. Used by `Fit` (Home key) to centre the
    /// camera on the symbol's content.
    pub(crate) fn bbox(&self) -> (f64, f64, f64, f64) {
        let (bx0, by0, bx1, by1) = self.body_rect();
        let mut min_x = bx0.min(bx1) - 5.08;
        let mut min_y = by0.min(by1) - 5.08;
        let mut max_x = bx0.max(bx1) + 5.08;
        let mut max_y = by0.max(by1) + 5.08;
        for pin in &self.symbol.pins {
            min_x = min_x.min(pin.position[0] - 1.27);
            min_y = min_y.min(pin.position[1] - 1.27);
            max_x = max_x.max(pin.position[0] + pin.length + 1.27);
            max_y = max_y.max(pin.position[1] + 1.27);
        }
        // Include every graphic's extent so Fit doesn't leave shapes
        // off-screen.
        for g in &self.symbol.graphics {
            match &g.kind {
                SymbolGraphicKind::Rectangle { from, to }
                | SymbolGraphicKind::Line { from, to } => {
                    min_x = min_x.min(from[0]).min(to[0]);
                    min_y = min_y.min(from[1]).min(to[1]);
                    max_x = max_x.max(from[0]).max(to[0]);
                    max_y = max_y.max(from[1]).max(to[1]);
                }
                SymbolGraphicKind::Circle { center, radius }
                | SymbolGraphicKind::Arc { center, radius, .. } => {
                    min_x = min_x.min(center[0] - radius);
                    min_y = min_y.min(center[1] - radius);
                    max_x = max_x.max(center[0] + radius);
                    max_y = max_y.max(center[1] + radius);
                }
                SymbolGraphicKind::Text { position, size, .. } => {
                    min_x = min_x.min(position[0] - size);
                    min_y = min_y.min(position[1] - size);
                    max_x = max_x.max(position[0] + size);
                    max_y = max_y.max(position[1] + size);
                }
            }
        }
        (min_x, min_y, max_x, max_y)
    }
}

/// World-space mm grid the symbol canvas snaps cursor positions
/// to when the user is placing/moving things. Independent of the
/// visible grid (which follows `panel_ctx.grid_size_mm`) so a 0.635
/// mm visible grid still snaps to 1.27 mm — Altium's "smaller grid
/// for visual precision, larger for commit". Future toolbar work
/// could expose a separate snap-grid picker.
const SNAP_GRID_MM: f64 = 1.27;

/// Convert screen coords → world-mm via the camera, then snap to
/// the symbol-canvas grid. The canvas's Standard y-flip happens at
/// the world↔screen boundary inside `world_to_screen` /
/// `screen_to_world`; we mirror it here so screen-down → world-up.
fn world_for(canvas: &SymbolCanvas<'_>, sx: f32, sy: f32, bounds: Rectangle) -> (f64, f64) {
    // The camera's screen_to_world doesn't know about y-flip — it
    // assumes screen and world share the same y-axis direction.
    // Symbol coords are Standard y-up; mirror by negating after.
    let world = canvas
        .camera
        .screen_to_world(iced::Point::new(sx, sy), bounds);
    let wx = world.x as f64;
    let wy = -world.y as f64;
    (
        (wx / SNAP_GRID_MM).round() * SNAP_GRID_MM,
        (wy / SNAP_GRID_MM).round() * SNAP_GRID_MM,
    )
}

/// Same as `world_for` but without the snap — used by the cursor
/// readout so the status footer shows the unsnapped position the
/// user actually pointed at.
fn world_unsnapped(canvas: &SymbolCanvas<'_>, sx: f32, sy: f32, bounds: Rectangle) -> (f64, f64) {
    let world = canvas
        .camera
        .screen_to_world(iced::Point::new(sx, sy), bounds);
    (world.x as f64, -world.y as f64)
}

impl<'a> canvas::Program<CanvasAction> for SymbolCanvas<'a> {
    type State = CanvasState;

    fn update(
        &self,
        state: &mut CanvasState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<CanvasAction>> {
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let pos = cursor.position_in(bounds)?;
                let (wx, wy) = world_for(self, pos.x, pos.y, bounds);
                match self.tool {
                    SymbolTool::Select => {
                        // Resize handles win over pin hits — corners /
                        // endpoints / radius are usually inside or right
                        // next to the body where pins might also live.
                        if let Some((idx, handle)) =
                            state::hit_test_graphic_handle(self.symbol, wx, wy)
                        {
                            state.dragging_handle = Some((idx, handle));
                            // Capture without publishing — the actual
                            // geometry mutation rides on CursorMoved.
                            return Some(canvas::Action::capture());
                        }
                        if let Some(sel) = state::hit_test(self.symbol, wx, wy) {
                            state.dragging = true;
                            Some(canvas::Action::publish(CanvasAction::Select(sel)).and_capture())
                        } else {
                            Some(canvas::Action::publish(CanvasAction::Deselect).and_capture())
                        }
                    }
                    SymbolTool::AddPin => Some(
                        canvas::Action::publish(CanvasAction::AddPin { x: wx, y: wy })
                            .and_capture(),
                    ),
                    SymbolTool::PlaceRectangle => Some(
                        canvas::Action::publish(CanvasAction::AddRectangle { x: wx, y: wy })
                            .and_capture(),
                    ),
                    SymbolTool::PlaceLine => Some(
                        canvas::Action::publish(CanvasAction::AddLine { x: wx, y: wy })
                            .and_capture(),
                    ),
                    SymbolTool::PlaceCircle => Some(
                        canvas::Action::publish(CanvasAction::AddCircle { x: wx, y: wy })
                            .and_capture(),
                    ),
                    SymbolTool::PlaceArc => Some(
                        canvas::Action::publish(CanvasAction::AddArc { x: wx, y: wy })
                            .and_capture(),
                    ),
                    SymbolTool::PlaceText => Some(
                        canvas::Action::publish(CanvasAction::AddText { x: wx, y: wy })
                            .and_capture(),
                    ),
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right))
            | Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)) => {
                // Right- or middle-button starts a pan. Schematic
                // canvas uses the same gesture (`canvas/mod.rs`).
                let pos = cursor.position_in(bounds)?;
                state.panning = true;
                state.last_pan_pos = Some(pos);
                Some(canvas::Action::capture())
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right))
            | Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)) => {
                state.panning = false;
                state.last_pan_pos = None;
                None
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let pos = cursor.position_in(bounds)?;
                // Pan first so panning while a handle is grabbed
                // doesn't accidentally reshape geometry.
                if state.panning {
                    let last = state.last_pan_pos.unwrap_or(pos);
                    let dx = pos.x - last.x;
                    let dy = pos.y - last.y;
                    state.last_pan_pos = Some(pos);
                    if dx != 0.0 || dy != 0.0 {
                        return Some(canvas::Action::publish(CanvasAction::Pan { dx, dy }));
                    }
                    return None;
                }
                let (wx, wy) = world_for(self, pos.x, pos.y, bounds);
                if let Some((idx, handle)) = state.dragging_handle {
                    return Some(canvas::Action::publish(CanvasAction::MoveGraphicHandle {
                        idx,
                        handle,
                        x: wx,
                        y: wy,
                    }));
                }
                if state.dragging {
                    return Some(canvas::Action::publish(CanvasAction::Move { x: wx, y: wy }));
                }
                // Idle cursor — publish the unsnapped world position
                // for the status footer X/Y readout.
                let (ux, uy) = world_unsnapped(self, pos.x, pos.y, bounds);
                Some(canvas::Action::publish(CanvasAction::CursorAt {
                    x_mm: Some(ux),
                    y_mm: Some(uy),
                }))
            }
            Event::Mouse(mouse::Event::CursorLeft) => {
                state.panning = false;
                state.last_pan_pos = None;
                Some(canvas::Action::publish(CanvasAction::CursorAt {
                    x_mm: None,
                    y_mm: None,
                }))
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let pos = cursor.position_in(bounds)?;
                let dy = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / 30.0,
                };
                if dy.abs() < f32::EPSILON {
                    return None;
                }
                Some(
                    canvas::Action::publish(CanvasAction::Zoom {
                        sx: pos.x,
                        sy: pos.y,
                        delta: dy,
                    })
                    .and_capture(),
                )
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
                state.dragging_handle = None;
                None
            }
            Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => match key {
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete)
                | iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace) => {
                    Some(canvas::Action::publish(CanvasAction::DeleteSelected))
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Home) => {
                    Some(canvas::Action::publish(CanvasAction::Fit))
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        frame.fill_rectangle(iced::Point::ORIGIN, bounds.size(), self.bg_color);

        // Camera-driven world↔screen. Symbol coords are Standard y-up;
        // the camera doesn't know that, so we negate y on the way
        // out to match screen y-down.
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
            // Visible bounds in world space (camera screen→world,
            // y-flipped). The grid pad lets dots peek past the
            // bbox so panning shows continuity.
            let pad = 6.0 * g;
            let (vx0, vy0) = world_unsnapped(self, 0.0, bounds.height, bounds);
            let (vx1, vy1) = world_unsnapped(self, bounds.width, 0.0, bounds);
            let world_x0 = (min_x - pad).min(vx0);
            let world_x1 = (max_x + pad).max(vx1);
            let world_y0 = (min_y - pad).min(vy0);
            let world_y1 = (max_y + pad).max(vy1);
            // Cap the iteration count so a zoomed-out view doesn't
            // try to plot millions of dots.
            let cols = ((world_x1 - world_x0) / g).abs() as i64 + 1;
            let rows = ((world_y1 - world_y0) / g).abs() as i64 + 1;
            // Skip render entirely when dots would be < 4 px apart
            // (they'd just smear into noise).
            let dot_screen_spacing = (g as f32) * scale;
            if cols * rows < 60_000 && dot_screen_spacing >= 4.0 {
                let dot_radius = (scale * 0.3).clamp(0.5, 2.0);
                let mut gx = (world_x0 / g).floor() * g;
                while gx <= world_x1 {
                    let mut gy = (world_y0 / g).floor() * g;
                    while gy <= world_y1 {
                        let p = w2s(gx, gy);
                        if p.x >= -dot_radius
                            && p.x <= bounds.width + dot_radius
                            && p.y >= -dot_radius
                            && p.y <= bounds.height + dot_radius
                        {
                            frame.fill(&canvas::Path::circle(p, dot_radius), self.grid_color);
                        }
                        gy += g;
                    }
                    gx += g;
                }
            }
        }

        // ── Body + every other graphic ──
        // Render the first Rectangle as the filled "body" (translucent
        // fill + thick stroke); all other graphics (additional rects,
        // lines, circles, arcs) render as outlines only. Selection
        // halo: the currently-selected graphic gets the accent stroke
        // colour with extra width so it stands out against the body.
        let selected_graphic_idx = match self.selected {
            Some(SymbolSelection::Graphic(i)) => Some(i),
            _ => None,
        };
        let mut body_drawn = false;
        for (i, g) in self.symbol.graphics.iter().enumerate() {
            let is_selected = selected_graphic_idx == Some(i);
            let stroke_color = if is_selected {
                self.selected_color
            } else {
                self.body_color
            };
            let stroke_w = if is_selected { 2.5 } else { 1.5 };
            // Rectangle defaults to a thicker stroke than other
            // outline graphics so the "body" reads cleanly; selection
            // overrides both with the accent stroke width.
            let rect_w = if is_selected { 2.5 } else { 2.0 };
            // Text colour follows the same selection rule; body uses
            // the regular text colour, selected uses the accent.
            let text_c = if is_selected {
                self.selected_color
            } else {
                self.text_color
            };
            match &g.kind {
                SymbolGraphicKind::Rectangle { from, to } => {
                    let p1 = w2s(from[0], from[1]);
                    let p2 = w2s(to[0], to[1]);
                    let top_left = iced::Point::new(p1.x.min(p2.x), p1.y.min(p2.y));
                    let size = Size::new((p2.x - p1.x).abs(), (p2.y - p1.y).abs());
                    let path = canvas::Path::rectangle(top_left, size);
                    if !body_drawn {
                        frame.fill(
                            &path,
                            Color {
                                a: 0.16,
                                ..self.body_color
                            },
                        );
                        body_drawn = true;
                    }
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_color(stroke_color)
                            .with_width(rect_w),
                    );
                }
                SymbolGraphicKind::Line { from, to } => {
                    let mut builder = canvas::path::Builder::new();
                    builder.move_to(w2s(from[0], from[1]));
                    builder.line_to(w2s(to[0], to[1]));
                    let path = builder.build();
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_color(stroke_color)
                            .with_width(stroke_w),
                    );
                }
                SymbolGraphicKind::Circle { center, radius } => {
                    let p = w2s(center[0], center[1]);
                    let path = canvas::Path::circle(p, (*radius as f32) * scale);
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_color(stroke_color)
                            .with_width(stroke_w),
                    );
                }
                SymbolGraphicKind::Arc {
                    center,
                    radius,
                    start_deg,
                    end_deg,
                } => {
                    let p = w2s(center[0], center[1]);
                    let r = (*radius as f32) * scale;
                    let s = (*start_deg as f32).to_radians();
                    let e = (*end_deg as f32).to_radians();
                    let mut builder = canvas::path::Builder::new();
                    builder.arc(canvas::path::Arc {
                        center: p,
                        radius: r,
                        start_angle: iced::Radians(s),
                        end_angle: iced::Radians(e),
                    });
                    let path = builder.build();
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_color(stroke_color)
                            .with_width(stroke_w),
                    );
                }
                SymbolGraphicKind::Text {
                    position,
                    content,
                    size: text_size,
                } => {
                    frame.fill_text(canvas::Text {
                        content: content.clone(),
                        position: w2s(position[0], position[1]),
                        size: ((*text_size as f32) * scale * 0.5).into(),
                        color: text_c,
                        ..canvas::Text::default()
                    });
                }
            }
        }

        // No graphics → fall back to a default body rectangle so the
        // user sees the symbol bounds while the body geometry is still
        // empty.
        if !body_drawn {
            let (bx0, by0, bx1, by1) = self.body_rect();
            let p1 = w2s(bx0, by0);
            let p2 = w2s(bx1, by1);
            let top_left = iced::Point::new(p1.x.min(p2.x), p1.y.min(p2.y));
            let size = Size::new((p2.x - p1.x).abs(), (p2.y - p1.y).abs());
            let path = canvas::Path::rectangle(top_left, size);
            frame.fill(
                &path,
                Color {
                    a: 0.10,
                    ..self.body_color
                },
            );
            frame.stroke(
                &path,
                canvas::Stroke::default()
                    .with_color(Color {
                        a: 0.4,
                        ..self.body_color
                    })
                    .with_width(1.0),
            );
        }

        // Pins — filtered by active_part. Pins with part_number == 0
        // (Part Zero) render on every part; other pins render only
        // when their part matches editor.active_part.
        for (i, pin) in self.symbol.pins.iter().enumerate() {
            if !self.pin_visible_on_active_part(pin) {
                continue;
            }
            self.draw_pin(&mut frame, &w2s, scale, pin, i);
        }

        // Resize handles for every placed graphic — visible when the
        // Select tool is active so the user can grab any corner /
        // endpoint / radius / arc-angle / text-anchor at any time.
        if self.tool == SymbolTool::Select {
            for idx in 0..self.symbol.graphics.len() {
                for (_handle, pos) in state::graphic_handles(self.symbol, idx) {
                    let p = w2s(pos[0], pos[1]);
                    let half = 3.0_f32;
                    let top_left = iced::Point::new(p.x - half, p.y - half);
                    let size = Size::new(half * 2.0, half * 2.0);
                    let path = canvas::Path::rectangle(top_left, size);
                    frame.fill(&path, self.bg_color);
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_color(self.selected_color)
                            .with_width(1.0),
                    );
                }
            }
        }

        // Tool hint.
        let tool_label = match self.tool {
            SymbolTool::Select => "Tool: Select  (Del to remove)",
            SymbolTool::AddPin => "Tool: Add Pin  (click to place)",
            SymbolTool::PlaceRectangle => "Tool: Place Rectangle  (click)",
            SymbolTool::PlaceLine => "Tool: Place Line  (click)",
            SymbolTool::PlaceCircle => "Tool: Place Ellipse  (click)",
            SymbolTool::PlaceArc => "Tool: Place Arc  (click)",
            SymbolTool::PlaceText => "Tool: Place Text  (click)",
        };
        frame.fill_text(canvas::Text {
            content: tool_label.to_string(),
            position: iced::Point::new(8.0, 8.0),
            size: 11.0.into(),
            color: Color {
                a: 0.55,
                ..self.text_color
            },
            ..canvas::Text::default()
        });

        vec![frame.into_geometry()]
    }
}

impl<'a> SymbolCanvas<'a> {
    fn draw_pin<F>(
        &self,
        frame: &mut canvas::Frame,
        w2s: &F,
        _scale: f32,
        pin: &SymbolPin,
        idx: usize,
    ) where
        F: Fn(f64, f64) -> iced::Point,
    {
        use signex_library::PinOrientation;
        let (dx, dy) = match pin.orientation {
            PinOrientation::Right => (pin.length, 0.0),
            PinOrientation::Up => (0.0, pin.length),
            PinOrientation::Left => (-pin.length, 0.0),
            PinOrientation::Down => (0.0, -pin.length),
            // `PinOrientation` is `non_exhaustive` — fall back to a
            // sensible default if signex-library adds new variants.
            _ => (-pin.length, 0.0),
        };
        let tip = w2s(pin.position[0], pin.position[1]);
        let body_end = w2s(pin.position[0] + dx, pin.position[1] + dy);
        let selected = matches!(self.selected, Some(SymbolSelection::Pin(i)) if i == idx);
        let stroke_color = if selected {
            self.selected_color
        } else {
            self.pin_color
        };

        frame.stroke(
            &canvas::Path::line(tip, body_end),
            canvas::Stroke::default()
                .with_color(stroke_color)
                .with_width(if selected { 2.5 } else { 1.5 }),
        );
        // Selection halo.
        if selected {
            frame.stroke(
                &canvas::Path::circle(tip, 5.0),
                canvas::Stroke::default()
                    .with_color(self.selected_color)
                    .with_width(1.0),
            );
        }
        // Marker dot at the electrical end.
        frame.fill(&canvas::Path::circle(tip, 2.5), stroke_color);

        // Pin number — between body_end and tip.
        let num_pos =
            iced::Point::new((tip.x + body_end.x) * 0.5, (tip.y + body_end.y) * 0.5 - 8.0);
        frame.fill_text(canvas::Text {
            content: pin.number.clone(),
            position: num_pos,
            size: 10.0.into(),
            color: self.text_color,
            ..canvas::Text::default()
        });

        // Pin name — past the body_end so the body looks tidy.
        let name_pos = iced::Point::new(body_end.x + 4.0, body_end.y - 6.0);
        frame.fill_text(canvas::Text {
            content: pin.name.clone(),
            position: name_pos,
            size: 10.0.into(),
            color: Color {
                a: 0.85,
                ..self.text_color
            },
            ..canvas::Text::default()
        });
    }
}
