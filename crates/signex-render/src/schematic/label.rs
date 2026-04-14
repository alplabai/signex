//! Label rendering — net labels, global labels, hierarchical labels.
//!
//! Reference: signex Tauri app schematicDrawHelpers.ts::drawLabels()
//! and KiCad sch_painter.cpp SCH_LABEL render.
//!
//! Net label:   Plain text at anchor. No shape. Bottom-aligned.
//! Global:      Arrow/pentagon shape. Shape type from label.shape field.
//! Hier:        Pentagon (flag) shape.
//! Power:       Rendered via LibSymbol in symbol pass — skip here.

use iced::Color;
use iced::widget::canvas::{self, path};

use signex_types::schematic::{HAlign, Label, LabelType};

use super::ScreenTransform;

pub fn draw_label(
    frame: &mut canvas::Frame,
    label: &Label,
    transform: &ScreenTransform,
    color: Color,
) {
    let font_size_mm = if label.font_size > 0.0 {
        label.font_size
    } else {
        1.27
    };
    let screen_font = (transform.world_len(font_size_mm) * crate::canvas_font_size_scale()).abs();
    if screen_font < 1.0 {
        return;
    }

    match label.label_type {
        LabelType::Net => draw_net_label(frame, label, transform, color, screen_font),
        LabelType::Global => {
            draw_global_label(frame, label, transform, color, screen_font, font_size_mm)
        }
        LabelType::Hierarchical => {
            draw_hier_label(frame, label, transform, color, screen_font, font_size_mm)
        }
        LabelType::Power => {}
    }
}

// Net label: plain text, no slash/anchor line.
// Anchor = connection point = bottom of text (textBaseline bottom).
fn draw_net_label(
    frame: &mut canvas::Frame,
    label: &Label,
    transform: &ScreenTransform,
    color: Color,
    screen_font: f32,
) {
    let offset = schematic_text_offset_net(label, label.font_size.max(1.27));
    draw_spin_text(frame, label, transform, color, screen_font, offset, false);
}

fn draw_global_label(
    frame: &mut canvas::Frame,
    label: &Label,
    transform: &ScreenTransform,
    color: Color,
    screen_font: f32,
    font_size_mm: f64,
) {
    let spin = label_spin_style(label);
    let shape = label.shape.as_str();
    let fs = font_size_mm;
    let h = fs * 1.4;
    let arrow_w = h * 0.5;
    let pad = fs * 0.3;
    let text_w = label.text.len() as f64 * fs * 0.6;
    let lx = label.position.x;
    let ly = label.position.y;
    let sw = (transform.scale * 0.15).clamp(0.5, 2.0);

    if matches!(spin, SpinStyle::Left | SpinStyle::Right) {
        let conn_right = matches!(spin, SpinStyle::Right);
        let dir: f64 = if conn_right { 1.0 } else { -1.0 };
        let body_w = text_w + pad * 2.0;
        let bsx = lx + dir * arrow_w;
        let bex = lx + dir * (arrow_w + body_w);
        let tip = lx + dir * (arrow_w + body_w + arrow_w);

        let pts: Vec<(f64, f64)> = match shape {
            "input" => vec![
                (lx, ly),
                (bsx, ly - h / 2.0),
                (bex, ly - h / 2.0),
                (bex, ly + h / 2.0),
                (bsx, ly + h / 2.0),
            ],
            "output" => vec![
                (lx, ly),
                (bsx, ly - h / 2.0),
                (bex, ly - h / 2.0),
                (tip, ly),
                (bex, ly + h / 2.0),
                (bsx, ly + h / 2.0),
            ],
            "bidirectional" | "tri_state" => vec![
                (lx, ly),
                (bsx, ly - h / 2.0),
                (bex, ly - h / 2.0),
                (tip, ly),
                (bex, ly + h / 2.0),
                (bsx, ly + h / 2.0),
            ],
            _ => {
                let x1 = lx.min(bex);
                let x2 = lx.max(bex);
                vec![
                    (x1, ly - h / 2.0),
                    (x2, ly - h / 2.0),
                    (x2, ly + h / 2.0),
                    (x1, ly + h / 2.0),
                ]
            }
        };
        draw_shape_closed(frame, &pts, transform, color, sw);

    } else {
        let (sx, sy) = transform.world_to_screen(lx, ly);
        let ra = if matches!(spin, SpinStyle::Up) {
            -std::f32::consts::FRAC_PI_2
        } else {
            std::f32::consts::FRAC_PI_2
        };
        let s_h = transform.world_len(h);
        let s_arr = transform.world_len(arrow_w);
        let s_body = transform.world_len(text_w + pad * 2.0);
        let s_tip = s_arr + s_body + s_arr;
        frame.with_save(|f| {
            f.translate(iced::Vector::new(sx, sy));
            f.rotate(ra);
            let pts_s: Vec<iced::Point> = match shape {
                "input" => vec![
                    iced::Point::new(0.0, 0.0),
                    iced::Point::new(s_arr, -s_h / 2.0),
                    iced::Point::new(s_arr + s_body, -s_h / 2.0),
                    iced::Point::new(s_arr + s_body, s_h / 2.0),
                    iced::Point::new(s_arr, s_h / 2.0),
                ],
                "output" => vec![
                    iced::Point::new(0.0, 0.0),
                    iced::Point::new(s_arr, -s_h / 2.0),
                    iced::Point::new(s_arr + s_body, -s_h / 2.0),
                    iced::Point::new(s_tip, 0.0),
                    iced::Point::new(s_arr + s_body, s_h / 2.0),
                    iced::Point::new(s_arr, s_h / 2.0),
                ],
                "bidirectional" | "tri_state" => vec![
                    iced::Point::new(0.0, 0.0),
                    iced::Point::new(s_arr, -s_h / 2.0),
                    iced::Point::new(s_arr + s_body, -s_h / 2.0),
                    iced::Point::new(s_tip, 0.0),
                    iced::Point::new(s_arr + s_body, s_h / 2.0),
                    iced::Point::new(s_arr, s_h / 2.0),
                ],
                _ => vec![
                    iced::Point::new(0.0, -s_h / 2.0),
                    iced::Point::new(s_arr + s_body, -s_h / 2.0),
                    iced::Point::new(s_arr + s_body, s_h / 2.0),
                    iced::Point::new(0.0, s_h / 2.0),
                ],
            };
            let pth = canvas::Path::new(|b: &mut path::Builder| {
                b.move_to(pts_s[0]);
                for &p in &pts_s[1..] {
                    b.line_to(p);
                }
                b.close();
            });
            f.stroke(
                &pth,
                canvas::Stroke::default()
                    .with_color(color)
                    .with_width((transform.scale * 0.15).clamp(0.5, 2.0)),
            );
        });
    }

    let text_offset = schematic_text_offset_global(label, spin, font_size_mm);
    draw_spin_text(frame, label, transform, color, screen_font, text_offset, true);
}

fn draw_hier_label(
    frame: &mut canvas::Frame,
    label: &Label,
    transform: &ScreenTransform,
    color: Color,
    screen_font: f32,
    font_size_mm: f64,
) {
    let spin = label_spin_style(label);
    let fs = font_size_mm;
    let h = fs * 1.4;
    let arrow_w = h * 0.5;
    let pad = fs * 0.3;
    let text_w = label.text.len() as f64 * fs * 0.6;
    let lx = label.position.x;
    let ly = label.position.y;
    let sw = (transform.scale * 0.15).clamp(0.5, 2.0);

    if matches!(spin, SpinStyle::Left | SpinStyle::Right) {
        let conn_right = matches!(spin, SpinStyle::Right);
        let pts: Vec<(f64, f64)> = if conn_right {
            vec![
                (lx, ly),
                (lx + arrow_w, ly - h / 2.0),
                (lx + arrow_w + text_w + pad * 2.0, ly - h / 2.0),
                (lx + arrow_w + text_w + pad * 2.0, ly + h / 2.0),
                (lx + arrow_w, ly + h / 2.0),
            ]
        } else {
            vec![
                (lx, ly),
                (lx - arrow_w, ly - h / 2.0),
                (lx - arrow_w - text_w - pad * 2.0, ly - h / 2.0),
                (lx - arrow_w - text_w - pad * 2.0, ly + h / 2.0),
                (lx - arrow_w, ly + h / 2.0),
            ]
        };
        draw_shape_closed(frame, &pts, transform, color, sw);
    } else {
        let (sx, sy) = transform.world_to_screen(lx, ly);
        let ra = if matches!(spin, SpinStyle::Up) {
            -std::f32::consts::FRAC_PI_2
        } else {
            std::f32::consts::FRAC_PI_2
        };
        let s_h = transform.world_len(h);
        let s_arr = transform.world_len(arrow_w);
        let s_body = transform.world_len(text_w + pad * 2.0);
        frame.with_save(|f| {
            f.translate(iced::Vector::new(sx, sy));
            f.rotate(ra);
            let pts_s = [
                iced::Point::new(0.0, 0.0),
                iced::Point::new(s_arr, -s_h / 2.0),
                iced::Point::new(s_arr + s_body, -s_h / 2.0),
                iced::Point::new(s_arr + s_body, s_h / 2.0),
                iced::Point::new(s_arr, s_h / 2.0),
            ];
            let pth = canvas::Path::new(|b: &mut path::Builder| {
                b.move_to(pts_s[0]);
                for &p in &pts_s[1..] {
                    b.line_to(p);
                }
                b.close();
            });
            f.stroke(
                &pth,
                canvas::Stroke::default()
                    .with_color(color)
                    .with_width((transform.scale * 0.15).clamp(0.5, 2.0)),
            );
        });
    }

    let text_offset = schematic_text_offset_hier(label, spin, font_size_mm);
    draw_spin_text(frame, label, transform, color, screen_font, text_offset, true);
}

#[derive(Clone, Copy)]
enum SpinStyle {
    Left,
    Up,
    Right,
    Bottom,
}

fn label_spin_style(label: &Label) -> SpinStyle {
    let rot = normalize_rotation(label.rotation);
    let vertical = rot == 90 || rot == 270;

    if vertical {
        if matches!(label.justify, HAlign::Right) {
            SpinStyle::Bottom
        } else {
            SpinStyle::Up
        }
    } else if matches!(label.justify, HAlign::Right) {
        SpinStyle::Left
    } else {
        SpinStyle::Right
    }
}

fn text_offset_mm(font_size_mm: f64) -> f64 {
    font_size_mm * 0.4
}

fn pen_width_mm() -> f64 {
    0.15
}

fn approx_text_width_mm(text: &str, font_size_mm: f64) -> f64 {
    text.chars().count() as f64 * font_size_mm * 0.6
}

fn schematic_text_offset_net(label: &Label, font_size_mm: f64) -> (f64, f64) {
    let dist = text_offset_mm(font_size_mm) + pen_width_mm();
    match label_spin_style(label) {
        SpinStyle::Up | SpinStyle::Bottom => (-dist, 0.0),
        SpinStyle::Left | SpinStyle::Right => (0.0, -dist),
    }
}

fn schematic_text_offset_hier(label: &Label, spin: SpinStyle, font_size_mm: f64) -> (f64, f64) {
    let dist = text_offset_mm(font_size_mm) + approx_text_width_mm(&label.text, font_size_mm);
    match spin {
        SpinStyle::Left => (-dist, 0.0),
        SpinStyle::Up => (0.0, -dist),
        SpinStyle::Right => (dist, 0.0),
        SpinStyle::Bottom => (0.0, dist),
    }
}

fn schematic_text_offset_global(label: &Label, spin: SpinStyle, font_size_mm: f64) -> (f64, f64) {
    let mut horiz = font_size_mm * 0.5;
    let vert = font_size_mm * 0.0715;

    match label.shape.as_str() {
        "input" | "bidirectional" | "tri_state" => {
            horiz += font_size_mm * 0.75;
        }
        _ => {}
    }

    match spin {
        SpinStyle::Left => (-horiz, vert),
        SpinStyle::Up => (vert, -horiz),
        SpinStyle::Right => (horiz, vert),
        SpinStyle::Bottom => (vert, horiz),
    }
}

fn draw_spin_text(
    frame: &mut canvas::Frame,
    label: &Label,
    transform: &ScreenTransform,
    color: Color,
    screen_font: f32,
    offset_mm: (f64, f64),
    center_vertical: bool,
) {
    let spin = label_spin_style(label);
    let wx = label.position.x + offset_mm.0;
    let wy = label.position.y + offset_mm.1;
    let sp = transform.to_screen_point(wx, wy);

    let h_align = match spin {
        SpinStyle::Left | SpinStyle::Bottom => iced::alignment::Horizontal::Right,
        SpinStyle::Right | SpinStyle::Up => iced::alignment::Horizontal::Left,
    };
    let v_align = if center_vertical {
        iced::alignment::Vertical::Center
    } else {
        iced::alignment::Vertical::Bottom
    };

    match spin {
        SpinStyle::Left | SpinStyle::Right => {
            frame.fill_text(canvas::Text {
                content: label.text.clone(),
                position: sp,
                color,
                size: iced::Pixels(screen_font),
                font: crate::canvas_font(),
                align_x: h_align.into(),
                align_y: v_align,
                ..canvas::Text::default()
            });
        }
        SpinStyle::Up | SpinStyle::Bottom => {
            let rad = if matches!(spin, SpinStyle::Up) {
                -std::f32::consts::FRAC_PI_2
            } else {
                std::f32::consts::FRAC_PI_2
            };

            frame.with_save(|f| {
                f.translate(iced::Vector::new(sp.x, sp.y));
                f.rotate(rad);
                f.fill_text(canvas::Text {
                    content: label.text.clone(),
                    position: iced::Point::ORIGIN,
                    color,
                    size: iced::Pixels(screen_font),
                    font: crate::canvas_font(),
                    align_x: h_align.into(),
                    align_y: v_align,
                    ..canvas::Text::default()
                });
            });
        }
    }
}

fn draw_shape_closed(
    frame: &mut canvas::Frame,
    pts: &[(f64, f64)],
    transform: &ScreenTransform,
    color: Color,
    stroke_width: f32,
) {
    if pts.is_empty() {
        return;
    }
    let pth = canvas::Path::new(|b: &mut path::Builder| {
        let first = transform.to_screen_point(pts[0].0, pts[0].1);
        b.move_to(first);
        for &(px, py) in &pts[1..] {
            b.line_to(transform.to_screen_point(px, py));
        }
        b.close();
    });
    frame.stroke(
        &pth,
        canvas::Stroke::default()
            .with_color(color)
            .with_width(stroke_width),
    );
}

fn normalize_rotation(deg: f64) -> i32 {
    let r = (deg.round() as i32) % 360;
    if r < 0 { r + 360 } else { r }
}
