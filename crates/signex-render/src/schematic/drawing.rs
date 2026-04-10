//! Schematic drawing primitives -- Line, Rect, Circle, Arc, Polyline,
//! and ChildSheet rendering.

use iced::widget::canvas::{self, path};
use iced::Color;

use signex_types::schematic::{ChildSheet, FillType, SchDrawing};

use super::ScreenTransform;

/// Draw a schematic drawing primitive.
pub fn draw_sch_drawing(
    frame: &mut canvas::Frame,
    drawing: &SchDrawing,
    transform: &ScreenTransform,
    color: Color,
) {
    match drawing {
        SchDrawing::Line {
            start, end, width, ..
        } => {
            let p1 = transform.to_screen_point(start.x, start.y);
            let p2 = transform.to_screen_point(end.x, end.y);
            let line = canvas::Path::line(p1, p2);
            let w = stroke_width(transform, *width);
            let stroke = canvas::Stroke::default()
                .with_color(color)
                .with_width(w);
            frame.stroke(&line, stroke);
        }

        SchDrawing::Rect {
            start,
            end,
            width,
            fill,
            ..
        } => {
            let p1 = transform.to_screen_point(start.x, start.y);
            let p2 = transform.to_screen_point(end.x, end.y);

            let min_x = p1.x.min(p2.x);
            let min_y = p1.y.min(p2.y);
            let w = (p1.x - p2.x).abs();
            let h = (p1.y - p2.y).abs();

            let rect = canvas::Path::rectangle(
                iced::Point::new(min_x, min_y),
                iced::Size::new(w, h),
            );

            if *fill != FillType::None {
                let fill_color = Color { a: color.a * 0.15, ..color };
                frame.fill(&rect, fill_color);
            }

            let sw = stroke_width(transform, *width);
            let stroke = canvas::Stroke::default()
                .with_color(color)
                .with_width(sw);
            frame.stroke(&rect, stroke);
        }

        SchDrawing::Circle {
            center,
            radius,
            width,
            fill,
            ..
        } => {
            let c = transform.to_screen_point(center.x, center.y);
            let r = transform.world_len(*radius).max(1.0);
            let circle = canvas::Path::circle(c, r);

            if *fill != FillType::None {
                let fill_color = Color { a: color.a * 0.15, ..color };
                frame.fill(&circle, fill_color);
            }

            let sw = stroke_width(transform, *width);
            let stroke = canvas::Stroke::default()
                .with_color(color)
                .with_width(sw);
            frame.stroke(&circle, stroke);
        }

        SchDrawing::Arc {
            start,
            mid,
            end,
            width,
            ..
        } => {
            // Approximate arc with line segments via the three-point method
            let sx = start.x;
            let sy = start.y;
            let mx = mid.x;
            let my = mid.y;
            let ex = end.x;
            let ey = end.y;

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
                });

                let sw = stroke_width(transform, *width);
                let stroke = canvas::Stroke::default()
                    .with_color(color)
                    .with_width(sw);
                frame.stroke(&path, stroke);
            } else {
                // Degenerate -- just draw a line
                let p1 = transform.to_screen_point(sx, sy);
                let p2 = transform.to_screen_point(ex, ey);
                let line = canvas::Path::line(p1, p2);
                let sw = stroke_width(transform, *width);
                let stroke = canvas::Stroke::default()
                    .with_color(color)
                    .with_width(sw);
                frame.stroke(&line, stroke);
            }
        }

        SchDrawing::Polyline {
            points,
            width,
            fill,
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
                if *fill && points.len() > 2 {
                    b.close();
                }
            });

            if *fill {
                let fill_color = Color { a: color.a * 0.15, ..color };
                frame.fill(&path, fill_color);
            }

            let sw = stroke_width(transform, *width);
            let stroke = canvas::Stroke::default()
                .with_color(color)
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
    let sw = (transform.scale * 0.2).max(1.0).min(3.0);
    let stroke = canvas::Stroke::default()
        .with_color(body_color)
        .with_width(sw);
    frame.stroke(&rect, stroke);

    // Sheet name text
    let font_size = transform.world_len(1.5).max(8.0);
    let text = canvas::Text {
        content: child.name.clone(),
        position: iced::Point::new(tl.x + 4.0, tl.y + font_size + 2.0),
        color: body_color,
        size: iced::Pixels(font_size),
        font: crate::IOSEVKA,
        ..canvas::Text::default()
    };
    frame.fill_text(text);

    // Filename text (smaller, below name)
    let small_font = (font_size * 0.75).max(6.0);
    let file_text = canvas::Text {
        content: child.filename.clone(),
        position: iced::Point::new(tl.x + 4.0, tl.y + font_size + small_font + 6.0),
        color: Color { a: body_color.a * 0.7, ..body_color },
        size: iced::Pixels(small_font),
        font: crate::IOSEVKA,
        ..canvas::Text::default()
    };
    frame.fill_text(file_text);

    // Draw sheet pins
    for pin in &child.pins {
        let pp = transform.to_screen_point(pin.position.x, pin.position.y);
        let dot = canvas::Path::circle(pp, (transform.scale * 0.3).max(2.0));
        frame.fill(&dot, body_color);

        let pin_text = canvas::Text {
            content: pin.name.clone(),
            position: iced::Point::new(pp.x + 4.0, pp.y),
            color: body_color,
            size: iced::Pixels(small_font),
            font: crate::IOSEVKA,
            align_y: iced::alignment::Vertical::Center.into(),
            ..canvas::Text::default()
        };
        frame.fill_text(pin_text);
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
    w.max(0.5).min(4.0)
}

fn circle_from_three_points(
    x1: f64, y1: f64,
    x2: f64, y2: f64,
    x3: f64, y3: f64,
) -> Option<(f64, f64, f64)> {
    let d = 2.0 * (x1 * (y2 - y3) + x2 * (y3 - y1) + x3 * (y1 - y2));
    if d.abs() < 1e-10 {
        return None;
    }
    let ux = ((x1 * x1 + y1 * y1) * (y2 - y3)
        + (x2 * x2 + y2 * y2) * (y3 - y1)
        + (x3 * x3 + y3 * y3) * (y1 - y2))
        / d;
    let uy = ((x1 * x1 + y1 * y1) * (x3 - x2)
        + (x2 * x2 + y2 * y2) * (x1 - x3)
        + (x3 * x3 + y3 * y3) * (x2 - x1))
        / d;
    let r = ((x1 - ux).powi(2) + (y1 - uy).powi(2)).sqrt();
    Some((ux, uy, r))
}

fn is_angle_between_ccw(start: f64, mid: f64, end: f64) -> bool {
    let tau = std::f64::consts::TAU;
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
