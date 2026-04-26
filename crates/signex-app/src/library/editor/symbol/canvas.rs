//! Symbol-tab interactive canvas.
//!
//! A small `iced::widget::canvas::Program` that renders the editable
//! [`SymbolDoc`](super::state::SymbolDoc) (body rectangle + pins +
//! Designator/Value fields) and emits user actions as
//! [`CanvasAction`] messages. Pan/zoom is intentionally fixed —
//! Phase-1 keeps focus on the placement primitives so the parent view
//! can render the canvas side-by-side with a properties pane without
//! contending for cursor input.
//!
//! World-space convention mirrors the schematic editor: 1.27 mm
//! schematic grid, Standard y-axis (positive going down on screen).
//! World origin is centred in the canvas; the active scale is computed
//! from the canvas size at draw time so the body always fits.

use iced::Color;
use iced::Rectangle;
use iced::Renderer;
use iced::Size;
use iced::Theme;
use iced::event::Event;
use iced::mouse;
use iced::widget::canvas;

use super::state::{FieldKey, SymbolDoc, SymbolPin, SymbolSelection};

/// The actions a [`SymbolCanvas`] can emit upward.
#[derive(Debug, Clone, Copy)]
pub enum CanvasAction {
    /// Add-pin tool: click the empty canvas → place a pin at the
    /// snapped world coordinate.
    AddPin { x: f64, y: f64 },
    /// Selection click on a pin or field.
    Select(SymbolSelection),
    /// Drop selection (background click, no item under cursor).
    Deselect,
    /// In-flight move drag — pin or field follows the cursor.
    Move { x: f64, y: f64 },
    /// User pressed Delete / Backspace.
    DeleteSelected,
}

/// Canvas tools — Altium-style `Tool` enum scoped to this surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolTool {
    Select,
    AddPin,
}

impl SymbolTool {
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            SymbolTool::Select => "Select",
            SymbolTool::AddPin => "Add Pin",
        }
    }
}

/// Canvas-program ephemeral state — drag tracking only. Persists
/// across frames via iced's per-program state, but is volatile across
/// view rebuilds.
#[derive(Debug, Default)]
pub struct CanvasState {
    /// True when the user is mid-drag (cursor moved past threshold).
    pub dragging: bool,
}

/// The canvas program.
pub struct SymbolCanvas<'a> {
    pub doc: &'a SymbolDoc,
    pub tool: SymbolTool,
    pub bg_color: Color,
    pub grid_color: Color,
    pub body_color: Color,
    pub pin_color: Color,
    pub selected_color: Color,
    pub text_color: Color,
}

impl<'a> SymbolCanvas<'a> {
    pub fn new(doc: &'a SymbolDoc, tool: SymbolTool) -> Self {
        Self {
            doc,
            tool,
            bg_color: Color::from_rgb(0.10, 0.11, 0.13),
            grid_color: Color::from_rgba(1.0, 1.0, 1.0, 0.06),
            body_color: Color::from_rgb(0.95, 0.78, 0.30),
            pin_color: Color::from_rgb(0.85, 0.88, 0.92),
            selected_color: Color::from_rgb(0.30, 0.85, 0.95),
            text_color: Color::from_rgb(0.85, 0.88, 0.92),
        }
    }

    /// Build the active world↔screen transform. Returns `(scale,
    /// world_origin_x_in_pixels, world_origin_y_in_pixels)`.
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

    /// Bounding box around all drawable elements, with a generous pad
    /// so the user can drag pins beyond the body rectangle.
    fn bbox(&self) -> (f64, f64, f64, f64) {
        let mut min_x = self.doc.body.x0.min(self.doc.body.x1) - 5.08;
        let mut min_y = self.doc.body.y0.min(self.doc.body.y1) - 5.08;
        let mut max_x = self.doc.body.x0.max(self.doc.body.x1) + 5.08;
        let mut max_y = self.doc.body.y0.max(self.doc.body.y1) + 5.08;
        for pin in &self.doc.pins {
            min_x = min_x.min(pin.x - 1.27);
            min_y = min_y.min(pin.y - 1.27);
            max_x = max_x.max(pin.x + pin.length + 1.27);
            max_y = max_y.max(pin.y + 1.27);
        }
        for f in [&self.doc.designator, &self.doc.value_field] {
            min_x = min_x.min(f.x);
            min_y = min_y.min(f.y);
            max_x = max_x.max(f.x);
            max_y = max_y.max(f.y);
        }
        (min_x, min_y, max_x, max_y)
    }

}

/// Convert screen coords → schematic-mm, snapped to the 1.27 mm grid.
/// Pulled out as a free function so the canvas program can call it
/// from `update` without tripping borrow rules.
fn world_for(canvas: &SymbolCanvas<'_>, sx: f32, sy: f32, bounds: Rectangle) -> (f64, f64) {
    let (scale, ox, oy) = canvas.transform(bounds);
    // Standard y is positive-down; our screen coordinate is also positive-down.
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
                        if let Some(sel) = self.doc.hit_test(wx, wy) {
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

        // Background.
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

        // Body rectangle.
        let body = &self.doc.body;
        let p1 = w2s(body.x0, body.y0);
        let p2 = w2s(body.x1, body.y1);
        let top_left = iced::Point::new(p1.x.min(p2.x), p1.y.min(p2.y));
        let size = Size::new((p2.x - p1.x).abs(), (p2.y - p1.y).abs());
        let body_path = canvas::Path::rectangle(top_left, size);
        frame.fill(
            &body_path,
            Color {
                a: 0.16,
                ..self.body_color
            },
        );
        frame.stroke(
            &body_path,
            canvas::Stroke::default()
                .with_color(self.body_color)
                .with_width(2.0),
        );

        // Pins.
        for (i, pin) in self.doc.pins.iter().enumerate() {
            self.draw_pin(&mut frame, &w2s, scale, pin, i);
        }

        // Fields.
        self.draw_field(
            &mut frame,
            &w2s,
            FieldKey::Reference,
            &self.doc.designator.value,
            self.doc.designator.x,
            self.doc.designator.y,
            matches!(self.doc.selected, Some(SymbolSelection::Field(FieldKey::Reference))),
        );
        self.draw_field(
            &mut frame,
            &w2s,
            FieldKey::Value,
            &self.doc.value_field.value,
            self.doc.value_field.x,
            self.doc.value_field.y,
            matches!(self.doc.selected, Some(SymbolSelection::Field(FieldKey::Value))),
        );

        // Tool hint.
        let tool_label = match self.tool {
            SymbolTool::Select => "Tool: Select  (Del to remove)",
            SymbolTool::AddPin => "Tool: Add Pin  (click to place)",
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
        let (dx, dy) = match pin.rotation as i32 {
            0 => (pin.length, 0.0),
            90 => (0.0, pin.length),
            180 => (-pin.length, 0.0),
            270 => (0.0, -pin.length),
            _ => (-pin.length, 0.0),
        };
        let tip = w2s(pin.x, pin.y);
        let body_end = w2s(pin.x + dx, pin.y + dy);
        let selected = matches!(self.doc.selected, Some(SymbolSelection::Pin(i)) if i == idx);
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
        let num_pos = iced::Point::new(
            (tip.x + body_end.x) * 0.5,
            (tip.y + body_end.y) * 0.5 - 8.0,
        );
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

    fn draw_field<F>(
        &self,
        frame: &mut canvas::Frame,
        w2s: &F,
        _key: FieldKey,
        value: &str,
        wx: f64,
        wy: f64,
        selected: bool,
    ) where
        F: Fn(f64, f64) -> iced::Point,
    {
        let p = w2s(wx, wy);
        let color = if selected {
            self.selected_color
        } else {
            Color {
                a: 0.95,
                ..self.text_color
            }
        };
        frame.fill_text(canvas::Text {
            content: value.to_string(),
            position: iced::Point::new(p.x, p.y),
            size: 12.0.into(),
            color,
            ..canvas::Text::default()
        });
        if selected {
            // Tiny halo around the anchor so the user sees what's grabbed.
            frame.stroke(
                &canvas::Path::circle(p, 6.0),
                canvas::Stroke::default()
                    .with_color(self.selected_color)
                    .with_width(1.0),
            );
        }
    }
}
