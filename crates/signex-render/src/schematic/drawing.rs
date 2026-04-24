//! Schematic drawing primitives -- Line, Rect, Circle, Arc, Polyline,
//! and ChildSheet rendering.

use iced::Color;
use iced::widget::canvas::{self, path};

use signex_types::schematic::{ChildSheet, FillType, SchDrawing};

use super::ScreenTransform;
use super::text::draw_rich_text;

/// Resolve the per-drawing stroke colour: falls back to the theme
/// `color` when the drawing has no `stroke_color` override.
fn resolve_stroke_color(
    theme: Color,
    override_rgba: Option<signex_types::schematic::StrokeColor>,
) -> Color {
    match override_rgba {
        Some(c) => Color::from_rgba(
            c.r as f32 / 255.0,
            c.g as f32 / 255.0,
            c.b as f32 / 255.0,
            c.a as f32 / 255.0,
        ),
        None => theme,
    }
}

/// Draw a schematic drawing primitive.
pub fn draw_sch_drawing(
    frame: &mut canvas::Frame,
    drawing: &SchDrawing,
    transform: &ScreenTransform,
    color: Color,
) {
    match drawing {
        SchDrawing::Line {
            start,
            end,
            width,
            stroke_color,
            ..
        } => {
            let p1 = transform.to_screen_point(start.x, start.y);
            let p2 = transform.to_screen_point(end.x, end.y);
            let line = canvas::Path::line(p1, p2);
            let w = stroke_width(transform, *width);
            let c = resolve_stroke_color(color, *stroke_color);
            let stroke = canvas::Stroke::default().with_color(c).with_width(w);
            frame.stroke(&line, stroke);
        }

        SchDrawing::Rect {
            start,
            end,
            width,
            fill,
            stroke_color,
            ..
        } => {
            let p1 = transform.to_screen_point(start.x, start.y);
            let p2 = transform.to_screen_point(end.x, end.y);

            let min_x = p1.x.min(p2.x);
            let min_y = p1.y.min(p2.y);
            let w = (p1.x - p2.x).abs();
            let h = (p1.y - p2.y).abs();

            let rect =
                canvas::Path::rectangle(iced::Point::new(min_x, min_y), iced::Size::new(w, h));

            let stroke_c = resolve_stroke_color(color, *stroke_color);
            if *fill != FillType::None {
                let fill_color = Color {
                    a: stroke_c.a * 0.15,
                    ..stroke_c
                };
                frame.fill(&rect, fill_color);
            }

            let sw = stroke_width(transform, *width);
            let stroke = canvas::Stroke::default()
                .with_color(stroke_c)
                .with_width(sw);
            frame.stroke(&rect, stroke);
        }

        SchDrawing::Circle {
            center,
            radius,
            width,
            fill,
            stroke_color,
            ..
        } => {
            let c = transform.to_screen_point(center.x, center.y);
            let r = transform.world_len(*radius).max(1.0);
            let circle = canvas::Path::circle(c, r);

            let stroke_c = resolve_stroke_color(color, *stroke_color);
            if *fill != FillType::None {
                let fill_color = Color {
                    a: stroke_c.a * 0.15,
                    ..stroke_c
                };
                frame.fill(&circle, fill_color);
            }

            let sw = stroke_width(transform, *width);
            let stroke = canvas::Stroke::default()
                .with_color(stroke_c)
                .with_width(sw);
            frame.stroke(&circle, stroke);
        }

        SchDrawing::Arc {
            start,
            mid,
            end,
            width,
            fill,
            stroke_color,
            ..
        } => {
            // Approximate arc with line segments via the three-point method
            let sx = start.x;
            let sy = start.y;
            let mx = mid.x;
            let my = mid.y;
            let ex = end.x;
            let ey = end.y;

            let stroke_c = resolve_stroke_color(color, *stroke_color);
            if let Some((cx, cy, r)) = circle_from_three_points(sx, sy, mx, my, ex, ey) {
                let start_angle = (sy - cy).atan2(sx - cx);
                let mid_angle = (my - cy).atan2(mx - cx);
                let end_angle = (ey - cy).atan2(ex - cx);

                let sweep_ccw = is_angle_between_ccw(start_angle, mid_angle, end_angle);
                let n = 24;
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
                    let px = cx + r * start_angle.cos();
                    let py = cy + r * start_angle.sin();
                    b.move_to(transform.to_screen_point(px, py));
                    for i in 1..=n {
                        let t = i as f64 / n as f64;
                        let a = start_angle + total_sweep * t;
                        let px = cx + r * a.cos();
                        let py = cy + r * a.sin();
                        b.line_to(transform.to_screen_point(px, py));
                    }
                    if *fill != FillType::None {
                        b.close();
                    }
                });

                if *fill != FillType::None {
                    let fill_color = Color {
                        a: stroke_c.a * 0.15,
                        ..stroke_c
                    };
                    frame.fill(&path, fill_color);
                }

                let sw = stroke_width(transform, *width);
                let stroke = canvas::Stroke::default()
                    .with_color(stroke_c)
                    .with_width(sw);
                frame.stroke(&path, stroke);
            } else {
                // Degenerate -- just draw a line
                let p1 = transform.to_screen_point(sx, sy);
                let p2 = transform.to_screen_point(ex, ey);
                let line = canvas::Path::line(p1, p2);
                let sw = stroke_width(transform, *width);
                let stroke = canvas::Stroke::default()
                    .with_color(stroke_c)
                    .with_width(sw);
                frame.stroke(&line, stroke);
            }
        }

        SchDrawing::Polyline {
            points,
            width,
            fill,
            stroke_color,
            ..
        } => {
            if points.len() < 2 {
                return;
            }

            let path = canvas::Path::new(|b: &mut path::Builder| {
                let p0 = transform.to_screen_point(points[0].x, points[0].y);
                b.move_to(p0);
                for pt in &points[1..] {
                    b.line_to(transform.to_screen_point(pt.x, pt.y));
                }
                if *fill != FillType::None && points.len() > 2 {
                    b.close();
                }
            });

            let stroke_c = resolve_stroke_color(color, *stroke_color);
            if *fill != FillType::None {
                let fill_color = Color {
                    a: stroke_c.a * 0.15,
                    ..stroke_c
                };
                frame.fill(&path, fill_color);
            }

            let sw = stroke_width(transform, *width);
            let stroke = canvas::Stroke::default()
                .with_color(stroke_c)
                .with_width(sw);
            frame.stroke(&path, stroke);
        }
    }
}

/// Draw a hierarchical child sheet as a labeled rectangle.
pub fn draw_child_sheet(
    frame: &mut canvas::Frame,
    child: &ChildSheet,
    transform: &ScreenTransform,
    body_color: Color,
    body_fill_color: Color,
) {
    let tl = transform.to_screen_point(child.position.x, child.position.y);
    let br = transform.to_screen_point(
        child.position.x + child.size.0,
        child.position.y + child.size.1,
    );

    let w = br.x - tl.x;
    let h = br.y - tl.y;

    if w <= 0.0 || h <= 0.0 {
        return;
    }

    // Fill background
    let rect = canvas::Path::rectangle(tl, iced::Size::new(w, h));
    frame.fill(&rect, body_fill_color);

    // Border
    let sw = (transform.scale * 0.2).clamp(1.0, 3.0);
    let stroke = canvas::Stroke::default()
        .with_color(body_color)
        .with_width(sw);
    frame.stroke(&rect, stroke);

    // Sheet name text
    let font_size = transform.world_len(1.5).abs();
    if font_size < 1.0 {
        return;
    }
    draw_rich_text(
        frame,
        &child.name,
        iced::Point::new(tl.x + 4.0, tl.y + font_size + 2.0),
        body_color,
        font_size,
        iced::alignment::Horizontal::Left,
        iced::alignment::Vertical::Top,
        0.0,
    );

    // Filename text (smaller, below name)
    let small_font = (font_size * 0.75).abs();
    if small_font < 1.0 {
        return;
    }
    draw_rich_text(
        frame,
        &child.filename,
        iced::Point::new(tl.x + 4.0, tl.y + font_size + small_font + 6.0),
        Color {
            a: body_color.a * 0.7,
            ..body_color
        },
        small_font,
        iced::alignment::Horizontal::Left,
        iced::alignment::Vertical::Top,
        0.0,
    );

    // Draw sheet pins — stub direction and label placement driven by rotation:
    //   0°   = left edge  (stub exits left,  label inside-right)
    //   180° = right edge (stub exits right, label inside-left)
    //   270° = top edge   (stub exits up,    label inside-below)
    //   90°  = bottom edge(stub exits down,  label inside-above)
    let pin_stub = 1.5;
    for pin in &child.pins {
        let pp = transform.to_screen_point(pin.position.x, pin.position.y);

        let rot = pin.rotation.rem_euclid(360.0).round() as i32;
        let (stub_wx, stub_wy, text_off_x, text_off_y, h_align, v_align) = match rot {
            180 => (
                pin.position.x + pin_stub,
                pin.position.y,
                -4.0,
                0.0,
                iced::alignment::Horizontal::Right,
                iced::alignment::Vertical::Center,
            ),
            270 => (
                pin.position.x,
                pin.position.y - pin_stub,
                0.0,
                small_font + 4.0,
                iced::alignment::Horizontal::Center,
                iced::alignment::Vertical::Top,
            ),
            90 => (
                pin.position.x,
                pin.position.y + pin_stub,
                0.0,
                -(small_font + 4.0),
                iced::alignment::Horizontal::Center,
                iced::alignment::Vertical::Bottom,
            ),
            _ => (
                // 0° and anything else → left edge
                pin.position.x - pin_stub,
                pin.position.y,
                4.0,
                0.0,
                iced::alignment::Horizontal::Left,
                iced::alignment::Vertical::Center,
            ),
        };

        let stub_end = transform.to_screen_point(stub_wx, stub_wy);
        frame.stroke(
            &canvas::Path::line(pp, stub_end),
            canvas::Stroke::default()
                .with_color(body_color)
                .with_width((transform.scale * 0.16).clamp(1.0, 2.0)),
        );

        let dot = canvas::Path::circle(pp, (transform.scale * 0.3).max(2.0));
        frame.fill(&dot, body_color);

        draw_rich_text(
            frame,
            &pin.name,
            iced::Point::new(pp.x + text_off_x, pp.y + text_off_y),
            body_color,
            small_font,
            h_align,
            v_align,
            0.0,
        );
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn stroke_width(transform: &ScreenTransform, world_width: f64) -> f32 {
    let w = if world_width > 0.0 {
        transform.world_len(world_width)
    } else {
        transform.scale * 0.15
    };
    w.clamp(0.5, 4.0)
}

// Geometry helpers — delegated to shared implementations in mod.rs
use super::{circle_from_three_points, is_angle_between_ccw};
