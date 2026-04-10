//! Label rendering — net labels, global labels, hierarchical labels, power labels.
//!
//! KiCad label types:
//! - Net label: plain text with a small angled line at the anchor point
//! - Global label: text inside a rectangle with pointed ends (overbar shape)
//! - Hierarchical label: text inside a flag/pentagon shape
//! - Power label: rendered as symbol graphics (GND bar, VCC arrow, etc.)

use iced::widget::canvas::{self, path};
use iced::Color;

use signex_types::schematic::{Label, LabelType};

use super::ScreenTransform;

/// Draw a label. Net labels are plain text. Global/Hier have shapes.
/// Power labels are handled separately via symbol rendering.
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
        LabelType::Net => {
            draw_net_label(frame, label, transform, color, screen_font);
        }
        LabelType::Global => {
            draw_global_label(frame, label, transform, color, screen_font, font_size_mm);
        }
        LabelType::Hierarchical => {
            draw_hier_label(frame, label, transform, color, screen_font, font_size_mm);
        }
        LabelType::Power => {
            // Power labels are rendered via their LibSymbol graphics in the symbol pass.
            // Just draw the text as fallback if the symbol isn't found.
            draw_net_label(frame, label, transform, color, screen_font);
        }
    }
}

/// Net label: plain text with a small angled line at the connection point.
/// This matches KiCad's rendering — no enclosed shape, just text + anchor line.
fn draw_net_label(
    frame: &mut canvas::Frame,
    label: &Label,
    transform: &ScreenTransform,
    color: Color,
    screen_font: f32,
) {
    let (sx, sy) = transform.world_to_screen(label.position.x, label.position.y);
    let line_len = transform.world_len(1.0); // 1mm anchor line

    // Draw small line at anchor point
    let stroke = canvas::Stroke::default()
        .with_color(color)
        .with_width((transform.scale * 0.15).max(0.5).min(2.0));

    let rot = normalize_rotation(label.rotation);
    let (line_end, text_pos, h_align) = match rot {
        0 => {
            // Text to the right, line goes down-left
            let end = iced::Point::new(sx - line_len * 0.5, sy + line_len * 0.5);
            let tp = iced::Point::new(sx + line_len * 0.3, sy - screen_font * 0.4);
            (end, tp, iced::alignment::Horizontal::Left)
        }
        90 => {
            // Text upward
            let end = iced::Point::new(sx - line_len * 0.5, sy + line_len * 0.5);
            let tp = iced::Point::new(sx - screen_font * 0.4, sy - line_len * 0.3);
            (end, tp, iced::alignment::Horizontal::Right)
        }
        180 => {
            // Text to the left
            let end = iced::Point::new(sx + line_len * 0.5, sy + line_len * 0.5);
            let tp = iced::Point::new(sx - line_len * 0.3, sy - screen_font * 0.4);
            (end, tp, iced::alignment::Horizontal::Right)
        }
        270 => {
            // Text downward
            let end = iced::Point::new(sx + line_len * 0.5, sy - line_len * 0.5);
            let tp = iced::Point::new(sx + screen_font * 0.4, sy + line_len * 0.3);
            (end, tp, iced::alignment::Horizontal::Left)
        }
        _ => {
            let end = iced::Point::new(sx - line_len * 0.5, sy + line_len * 0.5);
            let tp = iced::Point::new(sx + line_len * 0.3, sy - screen_font * 0.4);
            (end, tp, iced::alignment::Horizontal::Left)
        }
    };

    // Anchor line
    let line = canvas::Path::line(iced::Point::new(sx, sy), line_end);
    frame.stroke(&line, stroke);

    // Text
    let text = canvas::Text {
        content: label.text.clone(),
        position: text_pos,
        color,
        size: iced::Pixels(screen_font),
        align_x: h_align.into(),
        ..canvas::Text::default()
    };
    frame.fill_text(text);
}

/// Global label: text inside a rectangle with pointed ends on both sides.
fn draw_global_label(
    frame: &mut canvas::Frame,
    label: &Label,
    transform: &ScreenTransform,
    color: Color,
    screen_font: f32,
    font_size_mm: f64,
) {
    let x = label.position.x;
    let y = label.position.y;
    let pad = 0.5;
    let notch = 1.0;
    let text_w = label.text.len() as f64 * font_size_mm * 0.55;
    let half_h = font_size_mm * 0.5 + pad;
    let rot = normalize_rotation(label.rotation);

    // Build shape in world coords — hexagonal (pointed both ends)
    let pts: Vec<(f64, f64)> = match rot {
        0 => vec![
            (x, y),                              // anchor (right point)
            (x - notch, y - half_h),
            (x - notch - text_w, y - half_h),
            (x - notch - text_w - notch, y),     // left point
            (x - notch - text_w, y + half_h),
            (x - notch, y + half_h),
        ],
        180 => vec![
            (x, y),                              // anchor (left point)
            (x + notch, y - half_h),
            (x + notch + text_w, y - half_h),
            (x + notch + text_w + notch, y),     // right point
            (x + notch + text_w, y + half_h),
            (x + notch, y + half_h),
        ],
        90 => vec![
            (x, y),                              // anchor (top point)
            (x + half_h, y + notch),
            (x + half_h, y + notch + text_w),
            (x, y + notch + text_w + notch),
            (x - half_h, y + notch + text_w),
            (x - half_h, y + notch),
        ],
        270 => vec![
            (x, y),                              // anchor (bottom point)
            (x - half_h, y - notch),
            (x - half_h, y - notch - text_w),
            (x, y - notch - text_w - notch),
            (x + half_h, y - notch - text_w),
            (x + half_h, y - notch),
        ],
        _ => vec![
            (x, y),
            (x - notch, y - half_h),
            (x - notch - text_w, y - half_h),
            (x - notch - text_w - notch, y),
            (x - notch - text_w, y + half_h),
            (x - notch, y + half_h),
        ],
    };

    draw_shape(frame, &pts, transform, color);

    // Text centered in the shape
    let (cx, cy) = match rot {
        0 => (x - notch - text_w * 0.5, y),
        180 => (x + notch + text_w * 0.5, y),
        90 => (x, y + notch + text_w * 0.5),
        270 => (x, y - notch - text_w * 0.5),
        _ => (x - notch - text_w * 0.5, y),
    };
    let sp = transform.to_screen_point(cx, cy);
    let text = canvas::Text {
        content: label.text.clone(),
        position: sp,
        color,
        size: iced::Pixels(screen_font),
        align_x: iced::alignment::Horizontal::Center.into(),
        align_y: iced::alignment::Vertical::Center.into(),
        ..canvas::Text::default()
    };
    frame.fill_text(text);
}

/// Hierarchical label: text inside a flag shape (pointed on one side).
fn draw_hier_label(
    frame: &mut canvas::Frame,
    label: &Label,
    transform: &ScreenTransform,
    color: Color,
    screen_font: f32,
    font_size_mm: f64,
) {
    let x = label.position.x;
    let y = label.position.y;
    let pad = 0.5;
    let notch = 1.0;
    let text_w = label.text.len() as f64 * font_size_mm * 0.55;
    let half_h = font_size_mm * 0.5 + pad;
    let rot = normalize_rotation(label.rotation);

    // Pentagon: pointed on the anchor side, flat on the other
    let pts: Vec<(f64, f64)> = match rot {
        0 => vec![
            (x, y),                              // point (connection side)
            (x - notch, y - half_h),
            (x - notch - text_w - pad, y - half_h),
            (x - notch - text_w - pad, y + half_h),
            (x - notch, y + half_h),
        ],
        180 => vec![
            (x, y),
            (x + notch, y - half_h),
            (x + notch + text_w + pad, y - half_h),
            (x + notch + text_w + pad, y + half_h),
            (x + notch, y + half_h),
        ],
        90 => vec![
            (x, y),
            (x + half_h, y + notch),
            (x + half_h, y + notch + text_w + pad),
            (x - half_h, y + notch + text_w + pad),
            (x - half_h, y + notch),
        ],
        270 => vec![
            (x, y),
            (x - half_h, y - notch),
            (x - half_h, y - notch - text_w - pad),
            (x + half_h, y - notch - text_w - pad),
            (x + half_h, y - notch),
        ],
        _ => vec![
            (x, y),
            (x - notch, y - half_h),
            (x - notch - text_w - pad, y - half_h),
            (x - notch - text_w - pad, y + half_h),
            (x - notch, y + half_h),
        ],
    };

    draw_shape(frame, &pts, transform, color);

    // Text centered in the shape
    let (cx, cy) = match rot {
        0 => (x - notch - (text_w + pad) * 0.5, y),
        180 => (x + notch + (text_w + pad) * 0.5, y),
        90 => (x, y + notch + (text_w + pad) * 0.5),
        270 => (x, y - notch - (text_w + pad) * 0.5),
        _ => (x - notch - (text_w + pad) * 0.5, y),
    };
    let sp = transform.to_screen_point(cx, cy);
    let text = canvas::Text {
        content: label.text.clone(),
        position: sp,
        color,
        size: iced::Pixels(screen_font),
        align_x: iced::alignment::Horizontal::Center.into(),
        align_y: iced::alignment::Vertical::Center.into(),
        ..canvas::Text::default()
    };
    frame.fill_text(text);
}

// ─── Helpers ──────────────────────────────────────────────────

fn draw_shape(
    frame: &mut canvas::Frame,
    pts: &[(f64, f64)],
    transform: &ScreenTransform,
    color: Color,
) {
    if pts.is_empty() {
        return;
    }
    let path = canvas::Path::new(|b: &mut path::Builder| {
        let first = transform.to_screen_point(pts[0].0, pts[0].1);
        b.move_to(first);
        for &(px, py) in &pts[1..] {
            b.line_to(transform.to_screen_point(px, py));
        }
        b.close();
    });

    let width = (transform.scale * 0.15).max(0.5).min(2.0);
    frame.stroke(
        &path,
        canvas::Stroke::default().with_color(color).with_width(width),
    );
}

fn normalize_rotation(deg: f64) -> i32 {
    let r = (deg as i32) % 360;
    if r < 0 { r + 360 } else { r }
}
