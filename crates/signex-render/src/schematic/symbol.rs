//! Symbol body rendering -- draws LibSymbol graphics (polyline, rectangle,
//! circle, arc, text) with the symbol instance's position/rotation/mirror
//! transform applied.

use iced::Color;
use iced::widget::canvas::{self, path, LineCap, LineJoin};

use signex_types::schematic::{FillType, Graphic, LibSymbol, Point, Symbol};

use super::ScreenTransform;

// ---------------------------------------------------------------------------
// Instance transform: position + rotation + mirror
// ---------------------------------------------------------------------------

/// Delegate to the shared instance_transform in mod.rs.
fn instance_transform(sym: &Symbol, local: &Point) -> (f64, f64) {
    super::instance_transform(sym, local)
}

/// KiCad default symbol body stroke width in mm.
const BODY_DEFAULT_WIDTH_MM: f64 = 0.15;

/// Get the stroke width in screen pixels for a graphic element.
fn graphic_stroke_width(transform: &ScreenTransform, world_width: f64) -> f32 {
    let mm = if world_width > 0.0 { world_width } else { BODY_DEFAULT_WIDTH_MM };
    transform.world_len(mm).max(0.5)
}

/// Build a square-cap miter-join stroke for body outlines.
fn body_stroke(color: Color, width: f32) -> canvas::Stroke<'static> {
    canvas::Stroke {
        line_cap: LineCap::Square,
        line_join: LineJoin::Miter,
        ..canvas::Stroke::default().with_color(color).with_width(width)
    }
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

/// Draw a symbol's library graphics at the symbol instance's position,
/// filtering to only the matching unit and normal body style (body_style == 1).
pub fn draw_symbol(
    frame: &mut canvas::Frame,
    sym: &Symbol,
    lib: &LibSymbol,
    transform: &ScreenTransform,
    body_color: Color,
    body_fill_color: Color,
    _pin_color: Color,
) {
    // KiCad renders fills on a lower Z-layer than strokes.  In our single-pass
    // canvas we replicate this by iterating the graphic list TWICE:
    //   Pass 1 — background fills only      (like KiCad LAYER_DEVICE_BACKGROUND)
    //   Pass 2 — outline fills + all strokes (like KiCad LAYER_DEVICE)
    // This prevents body background fills from painting over stroked shapes
    // that happen to reside in an earlier sub-symbol (e.g. Relay_SPDT_0_0 triangle).
    for pass in 0u8..2 {
        for lg in &lib.graphics {
            // unit 0 = common to all units; otherwise must match symbol's unit
            if lg.unit != 0 && lg.unit != sym.unit {
                continue;
            }
            // body_style 0 = common; 1 = normal (default). Skip De Morgan (body_style 2).
            if lg.body_style != 0 && lg.body_style != 1 {
                continue;
            }
            // Pass 0: only background-fill graphics.
            // Pass 1: everything else (no-fill strokes + outline-fill strokes).
            let is_bg = graphic_has_background_fill(&lg.graphic);
            if pass == 0 && !is_bg {
                continue;
            }
            if pass == 1 && is_bg {
                continue;
            }
            match &lg.graphic {
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
                        frame, sym, text, position, *rotation, *font_size, transform, body_color,
                    );
                }
                Graphic::TextBox { .. } => {
                    // TextBox rendering is a v0.5 item
                }
                Graphic::Bezier {
                    points,
                    width,
                    fill,
                } => {
                    draw_bezier(
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
            }
        } // end inner for lg
    } // end for pass
}

// Returns true if a graphic's fill type is Background (needs pass-0 rendering).
fn graphic_has_background_fill(g: &Graphic) -> bool {
    matches!(
        g,
        Graphic::Rectangle {
            fill: FillType::Background,
            ..
        } | Graphic::Polyline {
            fill: FillType::Background,
            ..
        } | Graphic::Circle {
            fill: FillType::Background,
            ..
        } | Graphic::Arc {
            fill: FillType::Background,
            ..
        }
    )
}

// ---------------------------------------------------------------------------
// Individual graphic renderers
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
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
    frame.stroke(&path, body_stroke(body_color, graphic_stroke_width(transform, width)));
}

#[allow(clippy::too_many_arguments)]
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
    frame.stroke(&path, body_stroke(body_color, graphic_stroke_width(transform, width)));
}

#[allow(clippy::too_many_arguments)]
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
    frame.stroke(&circle, body_stroke(body_color, graphic_stroke_width(transform, width)));
}

#[allow(clippy::too_many_arguments)]
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
            // Close the chord for filled arcs (connects arc ends directly,
            // not back through center — this is the correct KiCad behavior)
            if fill != FillType::None {
                b.close();
            }
        });

        apply_fill(frame, &path, fill, body_color, body_fill_color);
        frame.stroke(&path, body_stroke(body_color, graphic_stroke_width(transform, width)));
    } else {
        // Degenerate: just draw a line from start to end
        let p1 = transform.to_screen_point(sx, sy);
        let p2 = transform.to_screen_point(ex, ey);
        let line = canvas::Path::line(p1, p2);
        frame.stroke(&line, body_stroke(body_color, graphic_stroke_width(transform, width)));
    }
}

#[allow(clippy::too_many_arguments)]
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

    let base_size = if font_size > 0.0 {
        transform.world_len(font_size)
    } else {
        transform.world_len(1.27)
    };
    let size = (base_size * crate::canvas_font_size_scale()).abs();
    if size < 1.0 {
        return;
    }

    let text = canvas::Text {
        content: content.to_string(),
        position: sp,
        color,
        size: iced::Pixels(size),
        font: crate::canvas_font(),
        align_x: iced::alignment::Horizontal::Center.into(),
        align_y: iced::alignment::Vertical::Center,
        ..canvas::Text::default()
    };
    frame.fill_text(text);
}

#[allow(clippy::too_many_arguments)]
fn draw_bezier(
    frame: &mut canvas::Frame,
    sym: &Symbol,
    points: &[Point],
    width: f64,
    fill: FillType,
    transform: &ScreenTransform,
    body_color: Color,
    body_fill_color: Color,
) {
    if points.len() != 4 {
        return;
    }

    let (p0x, p0y) = instance_transform(sym, &points[0]);
    let (c1x, c1y) = instance_transform(sym, &points[1]);
    let (c2x, c2y) = instance_transform(sym, &points[2]);
    let (p3x, p3y) = instance_transform(sym, &points[3]);

    let p0 = transform.to_screen_point(p0x, p0y);
    let c1 = transform.to_screen_point(c1x, c1y);
    let c2 = transform.to_screen_point(c2x, c2y);
    let p3 = transform.to_screen_point(p3x, p3y);

    let path = canvas::Path::new(|b: &mut path::Builder| {
        b.move_to(p0);
        b.bezier_curve_to(c1, c2, p3);
        if fill != FillType::None {
            b.close();
        }
    });

    apply_fill(frame, &path, fill, body_color, body_fill_color);
    frame.stroke(&path, body_stroke(body_color, graphic_stroke_width(transform, width)));
}

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

// Geometry helpers — delegated to shared implementations in mod.rs
use super::{circle_from_three_points, is_angle_between_ccw};
