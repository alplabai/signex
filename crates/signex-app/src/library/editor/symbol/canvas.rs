//! Symbol-tab interactive canvas.
//!
//! The canvas reads the typed [`signex_library::Symbol`] primitive
//! directly. The body rectangle is derived from `Symbol.graphics`
//! (first `Rectangle` graphic), or defaults to a
//! `[-5.08, -2.54] .. [5.08, 2.54]` rectangle when the primitive
//! carries no body geometry yet.
//!
//! Pan/zoom is intentionally fixed — the canvas auto-fits the body so
//! the parent view can render side-by-side with the properties pane
//! without contending for cursor input.
//!
//! World-space convention mirrors the schematic editor: 1.27 mm grid,
//! Standard y-axis (positive going up; on screen y goes down so we flip).

use iced::Color;
use iced::Rectangle;
use iced::Renderer;
use iced::Size;
use iced::Theme;
use iced::event::Event;
use iced::mouse;
use iced::widget::canvas;
use signex_library::{Symbol, SymbolGraphicKind, SymbolPin};

use super::state::{self, SymbolSelection};

/// The actions a [`SymbolCanvas`] can emit upward.
#[derive(Debug, Clone, Copy)]
pub enum CanvasAction {
    AddPin { x: f64, y: f64 },
    /// Stamp a default-sized rectangle (10 × 5 mm) centred on
    /// `(x, y)`. Drag-to-resize lands in a follow-up — for the
    /// first cut the rectangle is committed in one click and
    /// the user can later edit the corners via the Properties
    /// panel (or move/delete via the Select tool).
    AddRectangle { x: f64, y: f64 },
    /// Stamp a 5 mm horizontal line starting at `(x, y)` going
    /// right.
    AddLine { x: f64, y: f64 },
    /// Stamp a circle of radius 2 mm centred on `(x, y)`.
    AddCircle { x: f64, y: f64 },
    Select(SymbolSelection),
    Deselect,
    Move { x: f64, y: f64 },
    DeleteSelected,
}

/// Canvas tools — Altium-style `Tool` enum scoped to this surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolTool {
    Select,
    AddPin,
    PlaceRectangle,
    PlaceLine,
    PlaceCircle,
}

impl SymbolTool {
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            SymbolTool::Select => "Select",
            SymbolTool::AddPin => "Add Pin",
            SymbolTool::PlaceRectangle => "Rectangle",
            SymbolTool::PlaceLine => "Line",
            SymbolTool::PlaceCircle => "Circle",
        }
    }
}

/// Canvas-program ephemeral state — drag tracking only.
#[derive(Debug, Default)]
pub struct CanvasState {
    /// True when the user is mid-drag (cursor pressed and moved).
    pub dragging: bool,
}

/// The canvas program.
pub struct SymbolCanvas<'a> {
    pub symbol: &'a Symbol,
    pub selected: Option<SymbolSelection>,
    pub tool: SymbolTool,
    pub bg_color: Color,
    pub grid_color: Color,
    pub body_color: Color,
    pub pin_color: Color,
    pub selected_color: Color,
    pub text_color: Color,
}

impl<'a> SymbolCanvas<'a> {
    pub fn new(symbol: &'a Symbol, selected: Option<SymbolSelection>, tool: SymbolTool) -> Self {
        Self {
            symbol,
            selected,
            tool,
            bg_color: Color::from_rgb(0.10, 0.11, 0.13),
            grid_color: Color::from_rgba(1.0, 1.0, 1.0, 0.06),
            body_color: Color::from_rgb(0.95, 0.78, 0.30),
            pin_color: Color::from_rgb(0.85, 0.88, 0.92),
            selected_color: Color::from_rgb(0.30, 0.85, 0.95),
            text_color: Color::from_rgb(0.85, 0.88, 0.92),
        }
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

    /// Build the active world↔screen transform.
    fn transform(&self, bounds: Rectangle) -> (f32, f32, f32) {
        let (min_x, min_y, max_x, max_y) = self.bbox();
        let w = (max_x - min_x).max(0.1) as f32;
        let h = (max_y - min_y).max(0.1) as f32;
        let pad = 30.0_f32;
        let view_w = (bounds.width - 2.0 * pad).max(40.0);
        let view_h = (bounds.height - 2.0 * pad).max(40.0);
        let scale = (view_w / w).min(view_h / h).max(0.1);
        let cx = ((min_x + max_x) * 0.5) as f32;
        let cy = ((min_y + max_y) * 0.5) as f32;
        let ox = bounds.width * 0.5 - cx * scale;
        let oy = bounds.height * 0.5 + cy * scale;
        (scale, ox, oy)
    }

    /// Bounding box around the body + every pin, with a generous pad.
    fn bbox(&self) -> (f64, f64, f64, f64) {
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
        (min_x, min_y, max_x, max_y)
    }
}

/// Convert screen coords → schematic-mm, snapped to the 1.27 mm grid.
fn world_for(canvas: &SymbolCanvas<'_>, sx: f32, sy: f32, bounds: Rectangle) -> (f64, f64) {
    let (scale, ox, oy) = canvas.transform(bounds);
    // Standard y is positive-up in the data model; screen y is positive-down.
    let wx = ((sx - ox) / scale) as f64;
    let wy = -((sy - oy) / scale) as f64;
    let snap = 1.27_f64;
    let sx = (wx / snap).round() * snap;
    let sy = (wy / snap).round() * snap;
    (sx, sy)
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
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if !state.dragging {
                    return None;
                }
                let pos = cursor.position_in(bounds)?;
                let (wx, wy) = world_for(self, pos.x, pos.y, bounds);
                Some(canvas::Action::publish(CanvasAction::Move { x: wx, y: wy }))
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
                None
            }
            Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => match key {
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete)
                | iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace) => {
                    Some(canvas::Action::publish(CanvasAction::DeleteSelected))
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

        let (scale, ox, oy) = self.transform(bounds);
        let w2s = |x: f64, y: f64| -> iced::Point {
            iced::Point::new(ox + (x as f32) * scale, oy - (y as f32) * scale)
        };

        // Grid — major dot every 2.54 mm.
        let (min_x, min_y, max_x, max_y) = self.bbox();
        let mut gx = (min_x / 2.54).floor() * 2.54;
        while gx <= max_x {
            let mut gy = (min_y / 2.54).floor() * 2.54;
            while gy <= max_y {
                let p = w2s(gx, gy);
                frame.fill(&canvas::Path::circle(p, 0.8), self.grid_color);
                gy += 2.54;
            }
            gx += 2.54;
        }

        // ── Body + every other graphic ──
        // Render the first Rectangle as the filled "body" (translucent
        // fill + thick stroke); all other graphics (additional rects,
        // lines, circles, arcs) render as outlines only.
        let mut body_drawn = false;
        for g in &self.symbol.graphics {
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
                            .with_color(self.body_color)
                            .with_width(2.0),
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
                            .with_color(self.body_color)
                            .with_width(1.5),
                    );
                }
                SymbolGraphicKind::Circle { center, radius } => {
                    let p = w2s(center[0], center[1]);
                    let path = canvas::Path::circle(p, (*radius as f32) * scale);
                    frame.stroke(
                        &path,
                        canvas::Stroke::default()
                            .with_color(self.body_color)
                            .with_width(1.5),
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
                            .with_color(self.body_color)
                            .with_width(1.5),
                    );
                }
                SymbolGraphicKind::Text { position, content, size: text_size } => {
                    frame.fill_text(canvas::Text {
                        content: content.clone(),
                        position: w2s(position[0], position[1]),
                        size: ((*text_size as f32) * scale * 0.5).into(),
                        color: self.text_color,
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

        // Pins.
        for (i, pin) in self.symbol.pins.iter().enumerate() {
            self.draw_pin(&mut frame, &w2s, scale, pin, i);
        }

        // Tool hint.
        let tool_label = match self.tool {
            SymbolTool::Select => "Tool: Select  (Del to remove)",
            SymbolTool::AddPin => "Tool: Add Pin  (click to place)",
            SymbolTool::PlaceRectangle => "Tool: Place Rectangle  (click)",
            SymbolTool::PlaceLine => "Tool: Place Line  (click)",
            SymbolTool::PlaceCircle => "Tool: Place Circle  (click)",
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
