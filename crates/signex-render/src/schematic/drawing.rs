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

    // Sheet name + filename are placed OUTSIDE the box so they don't
    // overlap pins or fill colour:
    //   - Altium style: stacked above the top-left corner.
    //   - KiCad style:  stacked below the bottom-left corner.
    let font_size = transform.world_len(1.5).abs();
    if font_size < 1.0 {
        return;
    }
    let small_font = (font_size * 0.75).abs();
    let label_gap = 2.0_f32;

    let style = crate::multisheet_style();
    let (name_anchor, file_anchor, v_align) = match style {
        crate::MultisheetStyle::Altium => {
            // Above the box: filename closest to the border, name on top.
            let file_y = tl.y - label_gap;
            let name_y = file_y - small_font - label_gap;
            (
                iced::Point::new(tl.x, name_y),
                iced::Point::new(tl.x, file_y),
                iced::alignment::Vertical::Bottom,
            )
        }
        crate::MultisheetStyle::KiCad => {
            // Below the box: name closest to the border, filename underneath.
            let name_y = br.y + label_gap;
            let file_y = name_y + font_size + label_gap;
            (
                iced::Point::new(tl.x, name_y),
                iced::Point::new(tl.x, file_y),
                iced::alignment::Vertical::Top,
            )
        }
    };

    draw_rich_text(
        frame,
        &child.name,
        name_anchor,
        body_color,
        font_size,
        iced::alignment::Horizontal::Left,
        v_align,
        0.0,
    );

    if small_font < 1.0 {
        return;
    }
    draw_rich_text(
        frame,
        &child.filename,
        file_anchor,
        Color {
            a: body_color.a * 0.7,
            ..body_color
        },
        small_font,
        iced::alignment::Horizontal::Left,
        v_align,
        0.0,
    );

    // Draw sheet pins — Altium hierarchical port style. The pin's position is
    // the connection point on the sheet edge; the pentagon tip sits exactly
    // there with the body extending INWARD into the sheet so external wires
    // dock cleanly without any protruding stub.
    //
    // KiCad sheet-pin `rotation` is the OUTWARD direction (the way the pin
    // points away from the sheet body). Inward is therefore the opposite.
    //   rotation 0°   → outward +X (pin on right edge)  → inward -X
    //   rotation 180° → outward -X (pin on left  edge)  → inward +X
    //   rotation 90°  → outward -Y, screen up (pin on top    edge) → inward +Y down
    //   rotation 270° → outward +Y, screen down(pin on bottom edge) → inward -Y up
    let pin_h_mm = 1.4_f64;
    let arrow_len_mm = 0.7_f64;
    let body_len_mm = 2.4_f64;
    let text_pad_mm = 0.4_f64;
    let total_in_mm = arrow_len_mm + body_len_mm;

    for pin in &child.pins {
        let rot = pin.rotation.rem_euclid(360.0).round() as i32;

        // Inward unit vector (into the sheet) and label placement that puts
        // the text inside the sheet, hugging the flat back of the pentagon.
        // For top / bottom pins the label is rotated 90° so it reads
        // vertically along the inward direction — otherwise long names
        // overlap each other on closely-spaced pins (Altium convention).
        let (
            ix,
            iy,
            h_align,
            v_align,
            text_dx,
            text_dy,
            label_rotation_rad,
        ): (
            f64,
            f64,
            iced::alignment::Horizontal,
            iced::alignment::Vertical,
            f64,
            f64,
            f32,
        ) = match rot {
            0 => (
                // pin on RIGHT edge, body extends LEFT into sheet
                -1.0,
                0.0,
                iced::alignment::Horizontal::Right,
                iced::alignment::Vertical::Center,
                -(total_in_mm + text_pad_mm),
                0.0,
                0.0,
            ),
            90 => (
                // pin on TOP edge, body extends DOWN into sheet — vertical
                // label reads top-to-bottom (edge → into sheet).
                0.0,
                1.0,
                iced::alignment::Horizontal::Left,
                iced::alignment::Vertical::Center,
                0.0,
                total_in_mm + text_pad_mm,
                std::f32::consts::FRAC_PI_2,
            ),
            270 => (
                // pin on BOTTOM edge, body extends UP into sheet — vertical
                // label reads bottom-to-top (edge → into sheet).
                0.0,
                -1.0,
                iced::alignment::Horizontal::Left,
                iced::alignment::Vertical::Center,
                0.0,
                -(total_in_mm + text_pad_mm),
                -std::f32::consts::FRAC_PI_2,
            ),
            _ => (
                // 180° and fallback: pin on LEFT edge, body extends RIGHT into sheet
                1.0,
                0.0,
                iced::alignment::Horizontal::Left,
                iced::alignment::Vertical::Center,
                total_in_mm + text_pad_mm,
                0.0,
                0.0,
            ),
        };

        // Perpendicular vector (rotate inward 90° CCW) for the body half-height.
        let perpx = -iy;
        let perpy = ix;
        let half_h = pin_h_mm / 2.0;

        let lx = pin.position.x;
        let ly = pin.position.y;
        let arr_x = lx + ix * arrow_len_mm;
        let arr_y = ly + iy * arrow_len_mm;
        let back_x = lx + ix * total_in_mm;
        let back_y = ly + iy * total_in_mm;

        // Pentagon: tip on edge → arrow shoulders → flat back inside.
        let pts_world = [
            (lx, ly),
            (arr_x + perpx * half_h, arr_y + perpy * half_h),
            (back_x + perpx * half_h, back_y + perpy * half_h),
            (back_x - perpx * half_h, back_y - perpy * half_h),
            (arr_x - perpx * half_h, arr_y - perpy * half_h),
        ];

        let path = canvas::Path::new(|b: &mut path::Builder| {
            let p0 = transform.to_screen_point(pts_world[0].0, pts_world[0].1);
            b.move_to(p0);
            for &(x, y) in &pts_world[1..] {
                b.line_to(transform.to_screen_point(x, y));
            }
            b.close();
        });

        frame.fill(&path, body_fill_color);
        let sw = (transform.scale * 0.16).clamp(1.0, 2.0);
        frame.stroke(
            &path,
            canvas::Stroke::default().with_color(body_color).with_width(sw),
        );

        let text_anchor = transform.to_screen_point(lx + text_dx, ly + text_dy);
        draw_rich_text(
            frame,
            &pin.name,
            text_anchor,
            body_color,
            small_font,
            h_align,
            v_align,
            label_rotation_rad,
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
