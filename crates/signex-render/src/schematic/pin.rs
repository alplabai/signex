//! Pin rendering -- draws pin lines and optional name/number labels.
//!
//! A pin is defined in library-local coordinates. We apply the same
//! instance transform as for symbol graphics, then draw:
//! 1. A line from the pin position extending outward by `pin.length`.
//! 2. The pin name (outside the body) if `show_pin_names` is true.
//! 3. The pin number (inside the body) if `show_pin_numbers` is true.

use iced::Color;
use iced::widget::canvas::{self, path, LineCap, LineJoin};

use signex_types::schematic::{LibSymbol, Pin, PinShape, Point, Symbol};

use super::ScreenTransform;

// ---------------------------------------------------------------------------
// Instance transform (duplicated for self-containment -- could be shared)
// ---------------------------------------------------------------------------

/// Delegate to the shared instance_transform in mod.rs.
fn instance_transform(sym: &Symbol, local: &Point) -> (f64, f64) {
    super::instance_transform(sym, local)
}

/// Apply instance rotation + mirror to a direction vector (no translation).
fn instance_rotate_dir(sym: &Symbol, dx: f64, dy: f64) -> (f64, f64) {
    let x = dx;
    let y = -dy; // lib Y-up → schematic Y-down

    let rad = -sym.rotation.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    let rx = x * cos - y * sin;
    let ry = x * sin + y * cos;

    let rx = if sym.mirror_y { -rx } else { rx };
    let ry = if sym.mirror_x { -ry } else { ry };

    (rx, ry)
}

// ---------------------------------------------------------------------------
// Pin direction from rotation
// ---------------------------------------------------------------------------

/// Returns the unit direction vector a pin extends from its position
/// based on the pin's local rotation (0=right, 90=up, 180=left, 270=down).
/// Returns the unit direction vector a pin extends from its connection-point
/// toward the symbol body, in lib-local Y-UP coordinates.
/// 0=right, 90=up-in-lib, 180=left, 270=down-in-lib.
/// Results are passed through instance_transform which applies Y-flip + rotation.
fn pin_direction(pin: &Pin) -> (f64, f64) {
    let deg = ((pin.rotation % 360.0) + 360.0) % 360.0;
    match deg as i32 {
        0 => (1.0, 0.0),
        90 => (0.0, 1.0), // up in lib Y-up space
        180 => (-1.0, 0.0),
        270 => (0.0, -1.0), // down in lib Y-up space
        _ => {
            let rad = deg.to_radians();
            (rad.cos(), rad.sin()) // lib Y-up: positive sin = upward
        }
    }
}

// ---------------------------------------------------------------------------
// Draw all pins for a symbol
// ---------------------------------------------------------------------------

/// Draw all pins of a library symbol at the instance's position,
/// filtering to only the matching unit and normal body style.
pub fn draw_symbol_pins(
    frame: &mut canvas::Frame,
    sym: &Symbol,
    lib: &LibSymbol,
    transform: &ScreenTransform,
    pin_color: Color,
) {
    for lp in &lib.pins {
        // unit 0 = common; otherwise must match sym.unit
        if lp.unit != 0 && lp.unit != sym.unit {
            continue;
        }
        // Skip De Morgan body style (body_style 2)
        if lp.body_style != 0 && lp.body_style != 1 {
            continue;
        }
        draw_pin(frame, sym, lib, &lp.pin, transform, pin_color);
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
    let body_end = Point::new(pin.position.x + dir_x * len, pin.position.y + dir_y * len);

    // Transform to world coordinates
    let (wx1, wy1) = instance_transform(sym, &pin.position);
    let (wx2, wy2) = instance_transform(sym, &body_end);

    let p1 = transform.to_screen_point(wx1, wy1);
    let p2 = transform.to_screen_point(wx2, wy2);

    let stroke_width = transform.world_len(0.15).max(0.5);
    let stroke = canvas::Stroke {
        line_cap: LineCap::Square,
        line_join: LineJoin::Miter,
        ..canvas::Stroke::default().with_color(pin_color).with_width(stroke_width)
    };

    // INVERTED / INVERTED_CLOCK draw their own (shortened) line inside draw_pin_shape.
    // All other shapes get the full pin line drawn here.
    let shape_draws_own_line = matches!(pin.shape, PinShape::Inverted | PinShape::InvertedClock);
    if !shape_draws_own_line {
        let path = canvas::Path::new(|b: &mut path::Builder| {
            b.move_to(p1);
            b.line_to(p2);
        });
        frame.stroke(&path, stroke);
    }

    // Draw shape decorator at the connection end (p1).
    draw_pin_shape(frame, p1, p2, pin.shape, stroke_width, transform, pin_color);

    // Small circle at the connection point (endpoint) — removed, KiCad doesn't draw this

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
            font: crate::IOSEVKA,
            align_x: h_align.into(),
            align_y: iced::alignment::Vertical::Center,
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
        let (wp_dx, wp_dy) =
            instance_rotate_dir(sym, perp_x * perp_offset_mm, perp_y * perp_offset_mm);
        let np = transform.to_screen_point(mwx + wp_dx, mwy + wp_dy);

        let small_font = (screen_font * 0.8).max(5.0);
        let text = canvas::Text {
            content: pin.number.clone(),
            position: np,
            color: pin_color,
            size: iced::Pixels(small_font),
            font: crate::IOSEVKA,
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Center,
            ..canvas::Text::default()
        };
        frame.fill_text(text);
    }
}

// ---------------------------------------------------------------------------
// Pin shape decorators (mirroring KiCad SCH_PAINTER pin shape logic)
// ---------------------------------------------------------------------------

/// Draw two connected segments A→B and B→C (KiCad `triLine`).
fn tri_line(
    frame: &mut canvas::Frame,
    a: iced::Point,
    b: iced::Point,
    c: iced::Point,
    stroke: canvas::Stroke,
) {
    let path = canvas::Path::new(|p| {
        p.move_to(a);
        p.line_to(b);
        p.move_to(b);
        p.line_to(c);
    });
    frame.stroke(&path, stroke);
}

/// Draw the pin shape decorator.
///
/// Coordinate mapping vs KiCad `SCH_PAINTER`:
/// * `p1` = connection end (wire attaches here) = KiCad `pos = GetPosition()`
/// * `p2` = body end (at symbol body boundary)  = KiCad `p0 = GetPinRoot()`
/// * `bdx/bdy` = direction FROM body TOWARD connection = KiCad `dir`
///
/// All KiCad decorators are anchored at `p0` (body end) — so we anchor at
/// `p2`.  For `Inverted` and `InvertedClock`, this function also draws the
/// shortened pin line; `draw_pin` skips the full line for those two shapes.
fn draw_pin_shape(
    frame: &mut canvas::Frame,
    p1: iced::Point,
    p2: iced::Point,
    shape: PinShape,
    stroke_width: f32,
    transform: &ScreenTransform,
    color: Color,
) {
    if matches!(shape, PinShape::Line) {
        return;
    }

    let dx = p2.x - p1.x;
    let dy = p2.y - p1.y;
    let len_px = (dx * dx + dy * dy).sqrt();
    if len_px < 0.1 {
        return;
    }

    // Unit direction from p1 (connection) toward p2 (body).
    let sdx = dx / len_px;
    let sdy = dy / len_px;

    // KiCad "dir": from body (p2) toward connection (p1).
    let bdx = -sdx;
    let bdy = -sdy;

    // Wing (perpendicular) direction for clock triangles.
    // Horizontal pin: |bdx|≥|bdy| → wing = (0, 1).
    // Vertical pin:   |bdy|>|bdx| → wing = (1, 0).
    let wing_x = bdy.abs(); // = |sdy|
    let wing_y = bdx.abs(); // = |sdx|

    // Forward / low helpers for InputLow, ClockLow, OutputLow.
    // KiCad uses absolute screen-space -Y (up) for horizontal, -X (left) for vertical.
    let is_horiz = bdx.abs() >= bdy.abs();
    let (fwd_x, fwd_y) = if is_horiz {
        (bdx, 0.0_f32)
    } else {
        (0.0_f32, bdy)
    };

    // Decorator sizes in screen pixels.
    let radius = transform.world_len(0.508).max(2.0);
    let diam = radius * 2.0;
    let clock_size = transform.world_len(0.762).max(2.5);

    let stroke = canvas::Stroke::default()
        .with_color(color)
        .with_width(stroke_width);

    let pt = |x: f32, y: f32| iced::Point::new(x, y);

    match shape {
        PinShape::Line => unreachable!(),

        // ── INVERTED ─────────────────────────────────────────────────────────
        // Open circle just outside body (toward connection), then shortened line.
        // KiCad: DrawCircle(p0 + dir*radius, radius); DrawLine(p0+dir*diam, pos)
        PinShape::Inverted => {
            let cx = p2.x + bdx * radius;
            let cy = p2.y + bdy * radius;
            frame.stroke(&canvas::Path::circle(pt(cx, cy), radius), stroke);
            let path = canvas::Path::new(|b| {
                b.move_to(pt(p2.x + bdx * diam, p2.y + bdy * diam));
                b.line_to(p1);
            });
            frame.stroke(&path, stroke);
        }

        // ── CLOCK ────────────────────────────────────────────────────────────
        // Triangle at body end: ±perp wings, apex INTO the body.
        // KiCad: triLine(p0±perp*cs, p0 - dir*cs); full line drawn by caller.
        PinShape::Clock => {
            tri_line(
                frame,
                pt(p2.x + wing_x * clock_size, p2.y + wing_y * clock_size),
                pt(p2.x + sdx * clock_size, p2.y + sdy * clock_size), // INTO body
                pt(p2.x - wing_x * clock_size, p2.y - wing_y * clock_size),
                stroke,
            );
        }

        // ── INVERTED_CLOCK ───────────────────────────────────────────────────
        // Clock triangle at body end + Inverted circle + shortened line.
        PinShape::InvertedClock => {
            tri_line(
                frame,
                pt(p2.x + wing_x * clock_size, p2.y + wing_y * clock_size),
                pt(p2.x + sdx * clock_size, p2.y + sdy * clock_size),
                pt(p2.x - wing_x * clock_size, p2.y - wing_y * clock_size),
                stroke,
            );
            let cx = p2.x + bdx * radius;
            let cy = p2.y + bdy * radius;
            frame.stroke(&canvas::Path::circle(pt(cx, cy), radius), stroke);
            let path = canvas::Path::new(|b| {
                b.move_to(pt(p2.x + bdx * diam, p2.y + bdy * diam));
                b.line_to(p1);
            });
            frame.stroke(&path, stroke);
        }

        // ── INPUT_LOW ────────────────────────────────────────────────────────
        // IEEE active-low input: L-shape anchored at body end.
        // KiCad horiz: triLine(p0+(dir.x,0)*d, p0+(dir.x,-1)*d, p0)
        // KiCad vert:  triLine(p0+(0,dir.y)*d, p0+(-1,dir.y)*d, p0)
        PinShape::InputLow => {
            let ax = p2.x + fwd_x * diam;
            let ay = p2.y + fwd_y * diam;
            let (bx, by) = if is_horiz {
                (ax, ay - diam) // -Y = up on screen
            } else {
                (ax - diam, ay) // -X = left on screen
            };
            tri_line(frame, pt(ax, ay), pt(bx, by), p2, stroke);
        }

        // ── CLOCK_LOW / EDGE_CLOCK_HIGH ──────────────────────────────────────
        // Clock triangle + InputLow L-shape (KiCad treats these identically).
        PinShape::ClockLow | PinShape::EdgeClockHigh => {
            tri_line(
                frame,
                pt(p2.x + wing_x * clock_size, p2.y + wing_y * clock_size),
                pt(p2.x + sdx * clock_size, p2.y + sdy * clock_size),
                pt(p2.x - wing_x * clock_size, p2.y - wing_y * clock_size),
                stroke,
            );
            let ax = p2.x + fwd_x * diam;
            let ay = p2.y + fwd_y * diam;
            let (bx, by) = if is_horiz {
                (ax, ay - diam)
            } else {
                (ax - diam, ay)
            };
            tri_line(frame, pt(ax, ay), pt(bx, by), p2, stroke);
        }

        // ── OUTPUT_LOW ───────────────────────────────────────────────────────
        // IEEE active-low output: diagonal "flag" line.
        // KiCad horiz: line(p0 - (0,diam), p0 + dir.x*diam)
        // KiCad vert:  line(p0 - (diam,0), p0 + dir.y*diam)
        PinShape::OutputLow => {
            let (start, end_pt) = if is_horiz {
                (pt(p2.x, p2.y - diam), pt(p2.x + bdx * diam, p2.y))
            } else {
                (pt(p2.x - diam, p2.y), pt(p2.x, p2.y + bdy * diam))
            };
            let path = canvas::Path::new(|b| {
                b.move_to(start);
                b.line_to(end_pt);
            });
            frame.stroke(&path, stroke);
        }

        // ── NON_LOGIC ────────────────────────────────────────────────────────
        // X cross at body end using two diagonal lines.
        // KiCad: d1=(dir.x+dir.y, dir.y-dir.x), d2=(dir.x-dir.y, dir.x+dir.y)
        PinShape::NonLogic => {
            let d1x = bdx + bdy;
            let d1y = bdy - bdx;
            let d2x = bdx - bdy;
            let d2y = bdx + bdy;
            let path = canvas::Path::new(|b| {
                b.move_to(pt(p2.x - d1x * radius, p2.y - d1y * radius));
                b.line_to(pt(p2.x + d1x * radius, p2.y + d1y * radius));
                b.move_to(pt(p2.x - d2x * radius, p2.y - d2y * radius));
                b.line_to(pt(p2.x + d2x * radius, p2.y + d2y * radius));
            });
            frame.stroke(&path, stroke);
        }
    }
}
