//! Pin rendering -- draws pin lines and optional name/number labels.
//!
//! A pin is defined in library-local coordinates. We apply the same
//! instance transform as for symbol graphics, then draw:
//! 1. A line from the pin position extending outward by `pin.length`.
//! 2. The pin name (outside the body) if `show_pin_names` is true.
//! 3. The pin number (inside the body) if `show_pin_numbers` is true.

use iced::widget::canvas::{self, path};
use iced::Color;

use signex_types::schematic::{LibSymbol, Pin, Point, Symbol};

use super::ScreenTransform;

// ---------------------------------------------------------------------------
// Instance transform (duplicated for self-containment -- could be shared)
// ---------------------------------------------------------------------------

fn instance_transform(sym: &Symbol, local: &Point) -> (f64, f64) {
    let lx = local.x;
    let ly = -local.y; // Library Y-up → schematic Y-down

    let lx = if sym.mirror_x { -lx } else { lx };
    let ly = if sym.mirror_y { -ly } else { ly };

    let rad = sym.rotation.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();

    let rx = lx * cos - ly * sin;
    let ry = lx * sin + ly * cos;

    (rx + sym.position.x, ry + sym.position.y)
}

/// Apply instance rotation + mirror to a direction vector (no translation).
/// Direction vectors also need Y-flip from library space.
fn instance_rotate_dir(sym: &Symbol, dx: f64, dy: f64) -> (f64, f64) {
    let lx = dx;
    let ly = -dy; // Library Y-up → schematic Y-down

    let lx = if sym.mirror_x { -lx } else { lx };
    let ly = if sym.mirror_y { -ly } else { ly };

    let rad = sym.rotation.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();

    (lx * cos - ly * sin, lx * sin + ly * cos)
}

// ---------------------------------------------------------------------------
// Pin direction from rotation
// ---------------------------------------------------------------------------

/// Returns the unit direction vector a pin extends from its position
/// based on the pin's local rotation (0=right, 90=up, 180=left, 270=down).
fn pin_direction(pin: &Pin) -> (f64, f64) {
    let deg = ((pin.rotation % 360.0) + 360.0) % 360.0;
    match deg as i32 {
        0 => (1.0, 0.0),        // points right (endpoint is to the right)
        90 => (0.0, -1.0),      // points up
        180 => (-1.0, 0.0),     // points left
        270 => (0.0, 1.0),      // points down
        _ => {
            let rad = deg.to_radians();
            (rad.cos(), -rad.sin())
        }
    }
}

// ---------------------------------------------------------------------------
// Draw all pins for a symbol
// ---------------------------------------------------------------------------

/// Draw all pins of a library symbol at the instance's position.
pub fn draw_symbol_pins(
    frame: &mut canvas::Frame,
    sym: &Symbol,
    lib: &LibSymbol,
    transform: &ScreenTransform,
    pin_color: Color,
) {
    for pin in &lib.pins {
        draw_pin(frame, sym, lib, pin, transform, pin_color);
    }
}

/// Draw a single pin: line + optional name + optional number.
fn draw_pin(
    frame: &mut canvas::Frame,
    sym: &Symbol,
    lib: &LibSymbol,
    pin: &Pin,
    transform: &ScreenTransform,
    pin_color: Color,
) {
    // Pin position is the connection point (the end the wire connects to).
    // The pin line extends from the position toward the symbol body.
    let (dir_x, dir_y) = pin_direction(pin);
    let len = pin.length;

    // Endpoint = position (connection end)
    // Body end = position + direction * length (inside the symbol body)
    let body_end = Point::new(
        pin.position.x + dir_x * len,
        pin.position.y + dir_y * len,
    );

    // Transform to world coordinates
    let (wx1, wy1) = instance_transform(sym, &pin.position);
    let (wx2, wy2) = instance_transform(sym, &body_end);

    let p1 = transform.to_screen_point(wx1, wy1);
    let p2 = transform.to_screen_point(wx2, wy2);

    // Draw pin line
    let path = canvas::Path::new(|b: &mut path::Builder| {
        b.move_to(p1);
        b.line_to(p2);
    });

    let stroke_width = (transform.scale * 0.15).max(0.5).min(2.5);
    let stroke = canvas::Stroke::default()
        .with_color(pin_color)
        .with_width(stroke_width);
    frame.stroke(&path, stroke);

    // Small circle at the connection point (endpoint)
    let dot_radius = (transform.scale * 0.2).max(1.0).min(3.0);
    let dot = canvas::Path::circle(p1, dot_radius);
    frame.fill(&dot, pin_color);

    // Pin name (outside the body, beyond the endpoint)
    let font_size_mm = 1.27;
    let screen_font = transform.world_len(font_size_mm).max(6.0);

    if lib.show_pin_names && pin.name_visible && !pin.name.is_empty() && pin.name != "~" {
        let name_offset = lib.pin_name_offset.max(0.5);
        // Name is placed beyond the body end, offset along pin direction
        let name_pos = Point::new(
            body_end.x + dir_x * name_offset,
            body_end.y + dir_y * name_offset,
        );
        let (nwx, nwy) = instance_transform(sym, &name_pos);
        let np = transform.to_screen_point(nwx, nwy);

        // Determine text alignment based on the world-space pin direction
        let (wdx, _wdy) = instance_rotate_dir(sym, dir_x, dir_y);
        let h_align = if wdx > 0.1 {
            iced::alignment::Horizontal::Left
        } else if wdx < -0.1 {
            iced::alignment::Horizontal::Right
        } else {
            iced::alignment::Horizontal::Center
        };

        let text = canvas::Text {
            content: pin.name.clone(),
            position: np,
            color: pin_color,
            size: iced::Pixels(screen_font),
            align_x: h_align.into(),
            align_y: iced::alignment::Vertical::Center.into(),
            ..canvas::Text::default()
        };
        frame.fill_text(text);
    }

    // Pin number (inside the body, along the pin line)
    if lib.show_pin_numbers && pin.number_visible && !pin.number.is_empty() {
        // Number is placed at the midpoint of the pin line, offset slightly
        let mid = Point::new(
            pin.position.x + dir_x * len * 0.5,
            pin.position.y + dir_y * len * 0.5,
        );
        let (mwx, mwy) = instance_transform(sym, &mid);

        // Offset perpendicular to the pin direction so the number sits above the line
        let (perp_x, perp_y) = (-dir_y, dir_x);
        let perp_offset_mm = 0.8;
        let (wp_dx, wp_dy) = instance_rotate_dir(sym, perp_x * perp_offset_mm, perp_y * perp_offset_mm);
        let np = transform.to_screen_point(mwx + wp_dx, mwy + wp_dy);

        let small_font = (screen_font * 0.8).max(5.0);
        let text = canvas::Text {
            content: pin.number.clone(),
            position: np,
            color: pin_color,
            size: iced::Pixels(small_font),
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Center.into(),
            ..canvas::Text::default()
        };
        frame.fill_text(text);
    }
}
