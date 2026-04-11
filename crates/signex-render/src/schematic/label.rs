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
    let screen_font = transform.world_len(font_size_mm).max(6.0);

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
    let (sx, sy) = transform.world_to_screen(label.position.x, label.position.y);
    let offset = transform.world_len(0.3).max(1.5);

    let rot = normalize_rotation(label.rotation);
    let (actual_rot, flip) = match rot {
        180 => (0, true),
        270 => (90, true),
        r => (r, false),
    };

    let h_align = if matches!(label.justify, HAlign::Right) ^ flip {
        iced::alignment::Horizontal::Right
    } else {
        iced::alignment::Horizontal::Left
    };

    if actual_rot == 90 {
        frame.with_save(|f| {
            f.translate(iced::Vector::new(sx, sy));
            f.rotate(-std::f32::consts::FRAC_PI_2);
            f.fill_text(canvas::Text {
                content: label.text.clone(),
                position: iced::Point::new(offset, 0.0),
                color,
                size: iced::Pixels(screen_font),
                font: crate::IOSEVKA,
                align_x: h_align.into(),
                align_y: iced::alignment::Vertical::Bottom,
                ..canvas::Text::default()
            });
        });
    } else {
        frame.fill_text(canvas::Text {
            content: label.text.clone(),
            position: iced::Point::new(sx, sy - offset),
            color,
            size: iced::Pixels(screen_font),
            font: crate::IOSEVKA,
            align_x: h_align.into(),
            align_y: iced::alignment::Vertical::Bottom,
            ..canvas::Text::default()
        });
    }
}

fn draw_global_label(
    frame: &mut canvas::Frame,
    label: &Label,
    transform: &ScreenTransform,
    color: Color,
    screen_font: f32,
    font_size_mm: f64,
) {
    let rot = normalize_rotation(label.rotation);
    let shape = label.shape.as_str();
    let fs = font_size_mm;
    let h = fs * 1.4;
    let arrow_w = h * 0.5;
    let pad = fs * 0.3;
    let text_w = label.text.len() as f64 * fs * 0.6;
    let lx = label.position.x;
    let ly = label.position.y;
    let sw = (transform.scale * 0.15).clamp(0.5, 2.0);

    if rot == 0 || rot == 180 {
        let conn_right = rot == 0;
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

        let tx = lx + dir * (arrow_w + pad);
        let sp = transform.to_screen_point(tx, ly);
        let ha = if conn_right {
            iced::alignment::Horizontal::Left
        } else {
            iced::alignment::Horizontal::Right
        };
        frame.fill_text(canvas::Text {
            content: label.text.clone(),
            position: sp,
            color,
            size: iced::Pixels(screen_font),
            font: crate::IOSEVKA,
            align_x: ha.into(),
            align_y: iced::alignment::Vertical::Center,
            ..canvas::Text::default()
        });
    } else {
        let (sx, sy) = transform.world_to_screen(lx, ly);
        let ra = if rot == 90 {
            -std::f32::consts::FRAC_PI_2
        } else {
            std::f32::consts::FRAC_PI_2
        };
        let s_h = transform.world_len(h);
        let s_arr = transform.world_len(arrow_w);
        let s_body = transform.world_len(text_w + pad * 2.0);
        let s_pad = transform.world_len(pad);
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
            f.fill_text(canvas::Text {
                content: label.text.clone(),
                position: iced::Point::new(s_arr + s_pad, 0.0),
                color,
                size: iced::Pixels(screen_font),
                font: crate::IOSEVKA,
                align_x: iced::alignment::Horizontal::Left.into(),
                align_y: iced::alignment::Vertical::Center,
                ..canvas::Text::default()
            });
        });
    }
}

fn draw_hier_label(
    frame: &mut canvas::Frame,
    label: &Label,
    transform: &ScreenTransform,
    color: Color,
    screen_font: f32,
    font_size_mm: f64,
) {
    let rot = normalize_rotation(label.rotation);
    let fs = font_size_mm;
    let h = fs * 1.4;
    let arrow_w = h * 0.5;
    let pad = fs * 0.3;
    let text_w = label.text.len() as f64 * fs * 0.6;
    let lx = label.position.x;
    let ly = label.position.y;
    let sw = (transform.scale * 0.15).clamp(0.5, 2.0);

    if rot == 0 || rot == 180 {
        let conn_right = rot == 0;
        let dir: f64 = if conn_right { 1.0 } else { -1.0 };
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
        let tx = lx + dir * (arrow_w + pad);
        let sp = transform.to_screen_point(tx, ly);
        let ha = if conn_right {
            iced::alignment::Horizontal::Left
        } else {
            iced::alignment::Horizontal::Right
        };
        frame.fill_text(canvas::Text {
            content: label.text.clone(),
            position: sp,
            color,
            size: iced::Pixels(screen_font),
            font: crate::IOSEVKA,
            align_x: ha.into(),
            align_y: iced::alignment::Vertical::Center,
            ..canvas::Text::default()
        });
    } else {
        let (sx, sy) = transform.world_to_screen(lx, ly);
        let ra = if rot == 90 {
            -std::f32::consts::FRAC_PI_2
        } else {
            std::f32::consts::FRAC_PI_2
        };
        let s_h = transform.world_len(h);
        let s_arr = transform.world_len(arrow_w);
        let s_body = transform.world_len(text_w + pad * 2.0);
        let s_pad = transform.world_len(pad);
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
            f.fill_text(canvas::Text {
                content: label.text.clone(),
                position: iced::Point::new(s_arr + s_pad, 0.0),
                color,
                size: iced::Pixels(screen_font),
                font: crate::IOSEVKA,
                align_x: iced::alignment::Horizontal::Left.into(),
                align_y: iced::alignment::Vertical::Center,
                ..canvas::Text::default()
            });
        });
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
