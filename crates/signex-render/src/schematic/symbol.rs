//! Symbol body rendering -- draws LibSymbol graphics (polyline, rectangle,
//! circle, arc, text) with the symbol instance's position/rotation/mirror
//! transform applied.

use iced::widget::canvas::{self, path};
use iced::Color;

use signex_types::schematic::{FillType, Graphic, LibSymbol, Point, Symbol};

use super::ScreenTransform;

// ---------------------------------------------------------------------------
// Instance transform: position + rotation + mirror
// ---------------------------------------------------------------------------

/// Transform a local library-space point through the symbol instance's
/// position, rotation, and mirror state, returning a world-space point.
fn instance_transform(sym: &Symbol, local: &Point) -> (f64, f64) {
    let lx = local.x;
    // KiCad library coords are Y-up (math), schematic coords are Y-down (screen).
    // Flip Y at the boundary.
    let ly = -local.y;

    // Mirror (applied before rotation, KiCad convention)
    let lx = if sym.mirror_x { -lx } else { lx };
    let ly = if sym.mirror_y { -ly } else { ly };

    // Rotation — standard CCW rotation. Since we already flipped Y,
    // we're in Y-down space and the standard formula works directly
    // with KiCad's stored rotation angle.
    let rad = sym.rotation.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();

    let rx = lx * cos - ly * sin;
    let ry = lx * sin + ly * cos;

    // Translate to world position
    (rx + sym.position.x, ry + sym.position.y)
}

/// Get the stroke width in screen pixels for a graphic element.
fn graphic_stroke_width(transform: &ScreenTransform, world_width: f64) -> f32 {
    let w = if world_width > 0.0 {
        transform.world_len(world_width)
    } else {
        transform.scale * 0.15
    };
    w.max(0.5).min(4.0)
}

/// Apply fill according to fill type, body_color, body_fill_color.
fn apply_fill(
    frame: &mut canvas::Frame,
    path: &canvas::Path,
    fill_type: FillType,
    body_color: Color,
    body_fill_color: Color,
) {
    match fill_type {
        FillType::None => {}
        FillType::Outline => {
            frame.fill(path, body_color);
        }
        FillType::Background => {
            frame.fill(path, body_fill_color);
        }
    }
}

// ---------------------------------------------------------------------------
// Main symbol drawing
// ---------------------------------------------------------------------------

/// Draw a symbol's library graphics at the symbol instance's position.
pub fn draw_symbol(
    frame: &mut canvas::Frame,
    sym: &Symbol,
    lib: &LibSymbol,
    transform: &ScreenTransform,
    body_color: Color,
    body_fill_color: Color,
    _pin_color: Color,
) {
    for graphic in &lib.graphics {
        match graphic {
            Graphic::Polyline {
                points,
                width,
                fill,
            } => {
                draw_polyline(
                    frame,
                    sym,
                    points,
                    *width,
                    *fill,
                    transform,
                    body_color,
                    body_fill_color,
                );
            }
            Graphic::Rectangle {
                start,
                end,
                width,
                fill,
            } => {
                draw_rectangle(
                    frame,
                    sym,
                    start,
                    end,
                    *width,
                    *fill,
                    transform,
                    body_color,
                    body_fill_color,
                );
            }
            Graphic::Circle {
                center,
                radius,
                width,
                fill,
            } => {
                draw_circle(
                    frame,
                    sym,
                    center,
                    *radius,
                    *width,
                    *fill,
                    transform,
                    body_color,
                    body_fill_color,
                );
            }
            Graphic::Arc {
                start,
                mid,
                end,
                width,
                fill,
            } => {
                draw_arc(
                    frame,
                    sym,
                    start,
                    mid,
                    end,
                    *width,
                    *fill,
                    transform,
                    body_color,
                    body_fill_color,
                );
            }
            Graphic::Text {
                text,
                position,
                rotation,
                font_size,
                ..
            } => {
                draw_graphic_text(
                    frame,
                    sym,
                    text,
                    position,
                    *rotation,
                    *font_size,
                    transform,
                    body_color,
                );
            }
            Graphic::TextBox { .. } => {
                // TextBox rendering is a v0.5 item
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Individual graphic renderers
// ---------------------------------------------------------------------------

fn draw_polyline(
    frame: &mut canvas::Frame,
    sym: &Symbol,
    points: &[Point],
    width: f64,
    fill: FillType,
    transform: &ScreenTransform,
    body_color: Color,
    body_fill_color: Color,
) {
    if points.len() < 2 {
        return;
    }

    let path = canvas::Path::new(|b: &mut path::Builder| {
        let (wx, wy) = instance_transform(sym, &points[0]);
        b.move_to(transform.to_screen_point(wx, wy));
        for pt in &points[1..] {
            let (wx, wy) = instance_transform(sym, pt);
            b.line_to(transform.to_screen_point(wx, wy));
        }
        // Close the path if first == last (common for filled shapes)
        if points.len() > 2 {
            let first = &points[0];
            let last = &points[points.len() - 1];
            if (first.x - last.x).abs() < 0.001 && (first.y - last.y).abs() < 0.001 {
                b.close();
            }
        }
    });

    apply_fill(frame, &path, fill, body_color, body_fill_color);

    let stroke = canvas::Stroke::default()
        .with_color(body_color)
        .with_width(graphic_stroke_width(transform, width));
    frame.stroke(&path, stroke);
}

fn draw_rectangle(
    frame: &mut canvas::Frame,
    sym: &Symbol,
    start: &Point,
    end: &Point,
    width: f64,
    fill: FillType,
    transform: &ScreenTransform,
    body_color: Color,
    body_fill_color: Color,
) {
    // Transform the four corners through the instance transform
    let corners = [
        Point::new(start.x, start.y),
        Point::new(end.x, start.y),
        Point::new(end.x, end.y),
        Point::new(start.x, end.y),
    ];

    let path = canvas::Path::new(|b: &mut path::Builder| {
        let (wx, wy) = instance_transform(sym, &corners[0]);
        b.move_to(transform.to_screen_point(wx, wy));
        for c in &corners[1..] {
            let (wx, wy) = instance_transform(sym, c);
            b.line_to(transform.to_screen_point(wx, wy));
        }
        b.close();
    });

    apply_fill(frame, &path, fill, body_color, body_fill_color);

    let stroke = canvas::Stroke::default()
        .with_color(body_color)
        .with_width(graphic_stroke_width(transform, width));
    frame.stroke(&path, stroke);
}

fn draw_circle(
    frame: &mut canvas::Frame,
    sym: &Symbol,
    center: &Point,
    radius: f64,
    width: f64,
    fill: FillType,
    transform: &ScreenTransform,
    body_color: Color,
    body_fill_color: Color,
) {
    let (cx, cy) = instance_transform(sym, center);
    let screen_center = transform.to_screen_point(cx, cy);
    let screen_radius = transform.world_len(radius).max(1.0);

    let circle = canvas::Path::circle(screen_center, screen_radius);

    apply_fill(frame, &circle, fill, body_color, body_fill_color);

    let stroke = canvas::Stroke::default()
        .with_color(body_color)
        .with_width(graphic_stroke_width(transform, width));
    frame.stroke(&circle, stroke);
}

fn draw_arc(
    frame: &mut canvas::Frame,
    sym: &Symbol,
    start: &Point,
    mid: &Point,
    end: &Point,
    width: f64,
    fill: FillType,
    transform: &ScreenTransform,
    body_color: Color,
    body_fill_color: Color,
) {
    // Compute the arc's center and radius from three points (start, mid, end).
    // Uses the circumscribed circle of the triangle formed by the three points.
    let (sx, sy) = instance_transform(sym, start);
    let (mx, my) = instance_transform(sym, mid);
    let (ex, ey) = instance_transform(sym, end);

    if let Some((cx, cy, r)) = circle_from_three_points(sx, sy, mx, my, ex, ey) {
        // Compute angles
        let start_angle = (sy - cy).atan2(sx - cx);
        let mid_angle = (my - cy).atan2(mx - cx);
        let end_angle = (ey - cy).atan2(ex - cx);

        // Determine sweep direction: if mid is between start and end going
        // counter-clockwise, sweep CCW; otherwise CW.
        let sweep_ccw = is_angle_between_ccw(start_angle, mid_angle, end_angle);

        // Approximate the arc with line segments (16 segments)
        let n_segments = 24;
        let total_sweep = if sweep_ccw {
            let mut s = end_angle - start_angle;
            if s <= 0.0 {
                s += std::f64::consts::TAU;
            }
            s
        } else {
            let mut s = end_angle - start_angle;
            if s >= 0.0 {
                s -= std::f64::consts::TAU;
            }
            s
        };

        let path = canvas::Path::new(|b: &mut path::Builder| {
            let a0 = start_angle;
            let px = cx + r * a0.cos();
            let py = cy + r * a0.sin();
            b.move_to(transform.to_screen_point(px, py));

            for i in 1..=n_segments {
                let t = i as f64 / n_segments as f64;
                let a = a0 + total_sweep * t;
                let px = cx + r * a.cos();
                let py = cy + r * a.sin();
                b.line_to(transform.to_screen_point(px, py));
            }
        });

        apply_fill(frame, &path, fill, body_color, body_fill_color);

        let stroke = canvas::Stroke::default()
            .with_color(body_color)
            .with_width(graphic_stroke_width(transform, width));
        frame.stroke(&path, stroke);
    } else {
        // Degenerate: just draw a line from start to end
        let p1 = transform.to_screen_point(sx, sy);
        let p2 = transform.to_screen_point(ex, ey);
        let line = canvas::Path::line(p1, p2);
        let stroke = canvas::Stroke::default()
            .with_color(body_color)
            .with_width(graphic_stroke_width(transform, width));
        frame.stroke(&line, stroke);
    }
}

fn draw_graphic_text(
    frame: &mut canvas::Frame,
    sym: &Symbol,
    content: &str,
    position: &Point,
    _rotation: f64,
    font_size: f64,
    transform: &ScreenTransform,
    color: Color,
) {
    let (wx, wy) = instance_transform(sym, position);
    let sp = transform.to_screen_point(wx, wy);

    let size = if font_size > 0.0 {
        transform.world_len(font_size).max(6.0)
    } else {
        transform.world_len(1.27).max(6.0)
    };

    let text = canvas::Text {
        content: content.to_string(),
        position: sp,
        color,
        size: iced::Pixels(size),
        align_x: iced::alignment::Horizontal::Center.into(),
        align_y: iced::alignment::Vertical::Center.into(),
        ..canvas::Text::default()
    };
    frame.fill_text(text);
}

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

/// Compute the circumscribed circle center and radius from three points.
/// Returns `None` if the points are collinear.
fn circle_from_three_points(
    x1: f64, y1: f64,
    x2: f64, y2: f64,
    x3: f64, y3: f64,
) -> Option<(f64, f64, f64)> {
    let ax = x1;
    let ay = y1;
    let bx = x2;
    let by = y2;
    let cx = x3;
    let cy = y3;

    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-10 {
        return None;
    }

    let ux = ((ax * ax + ay * ay) * (by - cy)
        + (bx * bx + by * by) * (cy - ay)
        + (cx * cx + cy * cy) * (ay - by))
        / d;
    let uy = ((ax * ax + ay * ay) * (cx - bx)
        + (bx * bx + by * by) * (ax - cx)
        + (cx * cx + cy * cy) * (bx - ax))
        / d;

    let r = ((ax - ux).powi(2) + (ay - uy).powi(2)).sqrt();
    Some((ux, uy, r))
}

/// Check if `mid_angle` lies between `start_angle` and `end_angle` when
/// going counter-clockwise from start to end.
fn is_angle_between_ccw(start: f64, mid: f64, end: f64) -> bool {
    let tau = std::f64::consts::TAU;
    // Normalize angles to [0, TAU)
    let normalize = |a: f64| ((a % tau) + tau) % tau;
    let s = normalize(start);
    let m = normalize(mid);
    let e = normalize(end);

    if s <= e {
        s <= m && m <= e
    } else {
        m >= s || m <= e
    }
}
