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

use signex_types::schematic::{HAlign, Label, LabelType, VAlign};

use super::ScreenTransform;
use super::text::draw_rich_text;
use crate::LabelStyle;

pub fn draw_label(
    frame: &mut canvas::Frame,
    label: &Label,
    transform: &ScreenTransform,
    color: Color,
    body_fill: Color,
) {
    // All schematic canvas text renders at 10 pt (1.8 mm, cap-height basis)
    // regardless of what the source file declares. Overriding here instead of
    // mutating the stored value keeps save round-trips stable.
    let font_size_mm = crate::SCHEMATIC_TEXT_MM;
    let _stored = label.font_size;
    let screen_font = transform.world_len(font_size_mm).abs() * crate::STROKE_FONT_SCALE;
    if screen_font < 1.0 {
        return;
    }

    match label.label_type {
        LabelType::Net => draw_net_label(frame, label, transform, color, screen_font),
        LabelType::Global => {
            if matches!(crate::label_style(), LabelStyle::Altium) {
                draw_port_label_altium(
                    frame,
                    label,
                    transform,
                    color,
                    body_fill,
                    screen_font,
                    font_size_mm,
                );
            } else {
                draw_global_label(
                    frame,
                    label,
                    transform,
                    color,
                    body_fill,
                    screen_font,
                    font_size_mm,
                );
            }
        }
        LabelType::Hierarchical => {
            if matches!(crate::label_style(), LabelStyle::Altium) {
                draw_port_label_altium(
                    frame,
                    label,
                    transform,
                    color,
                    body_fill,
                    screen_font,
                    font_size_mm,
                );
            } else {
                draw_hier_label(frame, label, transform, color, screen_font, font_size_mm);
            }
        }
        LabelType::Power => {}
    }
}

fn draw_port_label_altium(
    frame: &mut canvas::Frame,
    label: &Label,
    transform: &ScreenTransform,
    color: Color,
    body_fill: Color,
    screen_font: f32,
    font_size_mm: f64,
) {
    // Altium-like unified port rendering for Global + Hier labels.
    // Net labels stay untouched in draw_net_label().
    // Long names need extra body width and right-tip breathing room so
    // text does not visually collide with the closing arrow notch.
    let spin = label_spin_style(label);
    let fs = font_size_mm;
    let h = fs * 1.4;
    let arrow_w = h * 0.5;
    let base_pad = fs * 0.3;
    let text_w = super::text::visible_char_count(&label.text) as f64 * fs * 0.6;
    let extra_body = if text_w > fs * 4.0 { fs * 0.8 } else { fs * 0.0 };
    let right_breathing = fs * 0.45;
    let lx = label.position.x;
    let ly = label.position.y;
    let sw = (transform.scale * 0.15).clamp(0.5, 2.0);

    if matches!(spin, SpinStyle::Left | SpinStyle::Right) {
        let conn_right = matches!(spin, SpinStyle::Right);
        let dir: f64 = if conn_right { 1.0 } else { -1.0 };
        let body_w = text_w + base_pad * 2.0 + extra_body + right_breathing;
        let bsx = lx + dir * arrow_w;
        let bex = lx + dir * (arrow_w + body_w);
        let tip = lx + dir * (arrow_w + body_w + arrow_w);

        let pts = vec![
            (lx, ly),
            (bsx, ly - h / 2.0),
            (bex, ly - h / 2.0),
            (tip, ly),
            (bex, ly + h / 2.0),
            (bsx, ly + h / 2.0),
        ];
        draw_shape_closed_filled(frame, &pts, transform, color, sw, Some(body_fill));
    } else {
        let (sx, sy) = transform.world_to_screen(lx, ly);
        let ra = if matches!(spin, SpinStyle::Up) {
            -std::f32::consts::FRAC_PI_2
        } else {
            std::f32::consts::FRAC_PI_2
        };
        let s_h = transform.world_len(h);
        let s_arr = transform.world_len(arrow_w);
        let s_body = transform.world_len(text_w + base_pad * 2.0 + extra_body + right_breathing);
        let s_tip = s_arr + s_body + s_arr;
        frame.with_save(|f| {
            f.translate(iced::Vector::new(sx, sy));
            f.rotate(ra);
            let pts_s = [
                iced::Point::new(0.0, 0.0),
                iced::Point::new(s_arr, -s_h / 2.0),
                iced::Point::new(s_arr + s_body, -s_h / 2.0),
                iced::Point::new(s_tip, 0.0),
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
            f.fill(&pth, body_fill);
            f.stroke(
                &pth,
                canvas::Stroke::default()
                    .with_color(color)
                    .with_width((transform.scale * 0.15).clamp(0.5, 2.0)),
            );
        });
    }

    let text_offset = schematic_text_offset_global(label, spin, font_size_mm);
    draw_spin_text(
        frame,
        label,
        transform,
        color,
        screen_font,
        text_offset,
    );
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
    let offset = schematic_text_offset_net(label, crate::SCHEMATIC_TEXT_MM);
    draw_spin_text(frame, label, transform, color, screen_font, offset);
}

fn draw_global_label(
    frame: &mut canvas::Frame,
    label: &Label,
    transform: &ScreenTransform,
    color: Color,
    body_fill: Color,
    screen_font: f32,
    font_size_mm: f64,
) {
    let spin = label_spin_style(label);
    let shape = label.shape.as_str();
    let fs = font_size_mm;
    let h = fs * 1.4;
    let arrow_w = h * 0.5;
    let pad = fs * 0.3;
    let text_w = super::text::visible_char_count(&label.text) as f64 * fs * 0.6;
    let lx = label.position.x;
    let ly = label.position.y;
    let sw = (transform.scale * 0.15).clamp(0.5, 2.0);
    // Port interior uses the same fill as component bodies in the active theme.
    let fill_color = body_fill;

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
        draw_shape_closed_filled(frame, &pts, transform, color, sw, Some(fill_color));
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
    draw_spin_text(
        frame,
        label,
        transform,
        color,
        screen_font,
        text_offset,
    );
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
    let text_w = super::text::visible_char_count(&label.text) as f64 * fs * 0.6;
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
    draw_spin_text(
        frame,
        label,
        transform,
        color,
        screen_font,
        text_offset,
    );
}

#[derive(Clone, Copy)]
pub enum SpinStyle {
    Left,
    Up,
    Right,
    Bottom,
}

/// Approximate on-screen AABB for a net/global/hierarchical label, in world
/// space, accounting for the label's orientation so hit-testing and selection
/// overlays match what is drawn.
/// AABB for a port (Global/Hierarchical) that wraps the whole pentagon shape,
/// anchored on the connection point and extending forward into the body.
fn port_shape_aabb(label: &Label) -> signex_types::schematic::Aabb {
    let fs = crate::SCHEMATIC_TEXT_MM;
    let h = fs * 1.4;
    let arrow_w = h * 0.5;
    let pad = fs * 0.3;
    let text_w = super::text::visible_char_count(&label.text) as f64 * fs * 0.6;
    let body_w = text_w + pad * 2.0;
    let total_fw = arrow_w + body_w + arrow_w; // including tip
    let half_h = h * 0.5;
    let (x0, y0, x1, y1) = match label_spin_style(label) {
        SpinStyle::Right => (
            label.position.x,
            label.position.y - half_h,
            label.position.x + total_fw,
            label.position.y + half_h,
        ),
        SpinStyle::Left => (
            label.position.x - total_fw,
            label.position.y - half_h,
            label.position.x,
            label.position.y + half_h,
        ),
        SpinStyle::Up => (
            label.position.x - half_h,
            label.position.y - total_fw,
            label.position.x + half_h,
            label.position.y,
        ),
        SpinStyle::Bottom => (
            label.position.x - half_h,
            label.position.y,
            label.position.x + half_h,
            label.position.y + total_fw,
        ),
    };
    signex_types::schematic::Aabb::new(x0, y0, x1, y1)
}

pub fn label_text_aabb(label: &Label) -> signex_types::schematic::Aabb {
    // Global/Hier labels are pentagons centered on the anchor; their bbox must
    // wrap the whole arrow+body shape, not just the text column. Delegate.
    if matches!(
        label.label_type,
        LabelType::Global | LabelType::Hierarchical
    ) {
        return port_shape_aabb(label);
    }
    let fs = crate::SCHEMATIC_TEXT_MM;
    let (off_x, off_y) = schematic_text_offset_net(label, fs);
    let anchor_x = label.position.x + off_x;
    let anchor_y = label.position.y + off_y;
    // Tight text width — slightly under half the font size per glyph matches
    // the actual rendered widths for most monospace-ish labels.
    // Iced's `fill_text(size = S)` reserves roughly S of vertical space for
    // glyphs including descenders; visible glyphs sit slightly inside that
    // metric (left bearing + small descender space). Use measurements tuned
    // against a real canvas font so the bbox wraps the *visible* text.
    let tw = super::text::visible_char_count(&label.text) as f64 * fs * 0.55;
    let th = pen_width_mm() + fs * 1.05;
    let spin = label_spin_style(label);

    let (x_lo, x_hi) = spin_text_x_bounds(spin, tw);
    let (y_lo, y_hi) = match label.justify_v {
        VAlign::Top => (0.0, th),
        VAlign::Center => (-th * 0.5, th * 0.5),
        VAlign::Bottom => (-th, 0.0),
    };

    let rot = match spin {
        SpinStyle::Left | SpinStyle::Right => 0.0_f64,
        SpinStyle::Up => -(std::f64::consts::FRAC_PI_2),
        SpinStyle::Bottom => std::f64::consts::FRAC_PI_2,
    };
    let (sin_r, cos_r) = rot.sin_cos();
    let corners = [(x_lo, y_lo), (x_hi, y_lo), (x_hi, y_hi), (x_lo, y_hi)];

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for (lx, ly) in corners {
        let rx = lx * cos_r - ly * sin_r;
        let ry = lx * sin_r + ly * cos_r;
        let wx = anchor_x + rx;
        let wy = anchor_y + ry;
        min_x = min_x.min(wx);
        min_y = min_y.min(wy);
        max_x = max_x.max(wx);
        max_y = max_y.max(wy);
    }

    signex_types::schematic::Aabb::new(min_x, min_y, max_x, max_y)
}

fn label_spin_style(label: &Label) -> SpinStyle {
    let rot = normalize_rotation(label.rotation);
    // KiCad net-label connection orientation is determined by rotation for
    // vertical labels; horizontal justify primarily affects horizontal labels.
    match rot {
        90 => SpinStyle::Up,
        270 => SpinStyle::Bottom,
        180 => {
            if matches!(label.justify, HAlign::Right) {
                SpinStyle::Right
            } else {
                SpinStyle::Left
            }
        }
        _ => {
            if matches!(label.justify, HAlign::Right) {
                SpinStyle::Left
            } else {
                SpinStyle::Right
            }
        }
    }
}

fn text_offset_mm(font_size_mm: f64) -> f64 {
    font_size_mm * 0.4
}

fn pen_width_mm() -> f64 {
    0.15
}

fn approx_text_width_mm(text: &str, font_size_mm: f64) -> f64 {
    super::text::visible_char_count(text) as f64 * font_size_mm * 0.6
}

fn schematic_text_offset_net(label: &Label, font_size_mm: f64) -> (f64, f64) {
    // KiCad keeps a small visual gap between the '+' connection point and the
    // first visible glyph. This spacing is implicit renderer behavior (not an
    // S-expression token), so we model it as a compact font-relative offset.
    let gap = (font_size_mm * 0.12).max(0.15);
    match label_spin_style(label) {
        SpinStyle::Right => (gap, 0.0),
        SpinStyle::Left => (-gap, 0.0),
        // Vertical labels still need a lateral wire gap (X axis), not a
        // reading-direction gap (Y axis), so the connection point stays at
        // the text end nearest the wire.
        SpinStyle::Up => (-gap, 0.0),
        SpinStyle::Bottom => (-gap, 0.0),
    }
}

fn spin_text_h_align(spin: SpinStyle) -> iced::alignment::Horizontal {
    match spin {
        SpinStyle::Left | SpinStyle::Up | SpinStyle::Bottom => iced::alignment::Horizontal::Right,
        SpinStyle::Right => iced::alignment::Horizontal::Left,
    }
}

fn spin_text_x_bounds(spin: SpinStyle, text_width: f64) -> (f64, f64) {
    match spin {
        SpinStyle::Left | SpinStyle::Up | SpinStyle::Bottom => (-text_width, 0.0),
        SpinStyle::Right => (0.0, text_width),
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
) {
    let spin = label_spin_style(label);
    let wx = label.position.x + offset_mm.0;
    let wy = label.position.y + offset_mm.1;
    let sp = transform.to_screen_point(wx, wy);

    let h_align = spin_text_h_align(spin);
    let v_align = match label.justify_v {
        VAlign::Top => iced::alignment::Vertical::Top,
        VAlign::Center => iced::alignment::Vertical::Center,
        VAlign::Bottom => iced::alignment::Vertical::Bottom,
    };

    match spin {
        SpinStyle::Left | SpinStyle::Right => {
            draw_rich_text(
                frame,
                &label.text,
                sp,
                color,
                screen_font,
                h_align,
                v_align,
                0.0,
            );
        }
        SpinStyle::Up | SpinStyle::Bottom => {
            let rad = if matches!(spin, SpinStyle::Up) {
                -std::f32::consts::FRAC_PI_2
            } else {
                std::f32::consts::FRAC_PI_2
            };
            draw_rich_text(frame, &label.text, sp, color, screen_font, h_align, v_align, rad);
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
    draw_shape_closed_filled(frame, pts, transform, color, stroke_width, None);
}

/// Like `draw_shape_closed` but with an optional fill color — used by global
/// port shapes which render with an Altium-style pale-yellow body under the
/// colored stroke.
fn draw_shape_closed_filled(
    frame: &mut canvas::Frame,
    pts: &[(f64, f64)],
    transform: &ScreenTransform,
    stroke_color: Color,
    stroke_width: f32,
    fill: Option<Color>,
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
    if let Some(fc) = fill {
        frame.fill(&pth, fc);
    }
    frame.stroke(
        &pth,
        canvas::Stroke::default()
            .with_color(stroke_color)
            .with_width(stroke_width),
    );
}

fn normalize_rotation(deg: f64) -> i32 {
    let r = (deg.round() as i32) % 360;
    if r < 0 { r + 360 } else { r }
}

#[cfg(test)]
mod tests {
    use super::{
        SpinStyle, label_spin_style, schematic_text_offset_net, spin_text_h_align,
        spin_text_x_bounds,
    };
    use signex_types::schematic::{HAlign, Label, LabelType, Point, VAlign};
    use uuid::Uuid;

    fn mk_label(rotation: f64, justify: HAlign) -> Label {
        Label {
            uuid: Uuid::nil(),
            text: "DIVIDED-S_2".to_string(),
            position: Point { x: 0.0, y: 0.0 },
            rotation,
            label_type: LabelType::Net,
            shape: String::new(),
            font_size: 1.27,
            justify,
            justify_v: VAlign::Bottom,
        }
    }

    #[test]
    fn label_spin_style_distinguishes_90_and_270_for_left_justify() {
        assert!(matches!(
            label_spin_style(&mk_label(90.0, HAlign::Left)),
            SpinStyle::Up
        ));
        assert!(matches!(
            label_spin_style(&mk_label(270.0, HAlign::Left)),
            SpinStyle::Bottom
        ));
    }

    #[test]
    fn label_spin_style_distinguishes_90_and_270_for_right_justify() {
        assert!(matches!(
            label_spin_style(&mk_label(90.0, HAlign::Right)),
            SpinStyle::Up
        ));
        assert!(matches!(
            label_spin_style(&mk_label(270.0, HAlign::Right)),
            SpinStyle::Bottom
        ));
    }

    #[test]
    fn label_spin_style_center_uses_rotation_direction() {
        assert!(matches!(
            label_spin_style(&mk_label(0.0, HAlign::Center)),
            SpinStyle::Right
        ));
        assert!(matches!(
            label_spin_style(&mk_label(90.0, HAlign::Center)),
            SpinStyle::Up
        ));
        assert!(matches!(
            label_spin_style(&mk_label(180.0, HAlign::Center)),
            SpinStyle::Left
        ));
        assert!(matches!(
            label_spin_style(&mk_label(270.0, HAlign::Center)),
            SpinStyle::Bottom
        ));
    }

    #[test]
    fn vertical_spins_use_same_side_anchor_bounds() {
        let (u0, u1) = spin_text_x_bounds(SpinStyle::Up, 10.0);
        let (d0, d1) = spin_text_x_bounds(SpinStyle::Bottom, 10.0);
        assert_eq!((u0, u1), (d0, d1));
        assert_eq!(
            spin_text_h_align(SpinStyle::Up),
            spin_text_h_align(SpinStyle::Bottom)
        );
    }

    #[test]
    fn vertical_spins_use_same_lateral_gap() {
        let up = mk_label(90.0, HAlign::Left);
        let down = mk_label(270.0, HAlign::Left);
        let (ux, uy) = schematic_text_offset_net(&up, 1.27);
        let (dx, dy) = schematic_text_offset_net(&down, 1.27);
        assert!((ux - dx).abs() < 1e-9);
        assert!((uy - dy).abs() < 1e-9);
    }
}
