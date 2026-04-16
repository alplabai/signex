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
use super::text::display_text_content;

pub fn draw_label(
    frame: &mut canvas::Frame,
    label: &Label,
    transform: &ScreenTransform,
    color: Color,
) {
    // All schematic canvas text renders at 10 pt (1.8 mm, cap-height basis)
    // regardless of what the source file declares. Overriding here instead of
    // mutating the stored value keeps save round-trips stable.
    let font_size_mm = crate::SCHEMATIC_TEXT_MM;
    let _stored = label.font_size;
    let screen_font = transform.world_len(font_size_mm).abs();
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
    let offset = schematic_text_offset_net(label, crate::SCHEMATIC_TEXT_MM);
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
    // Altium port palette — pale cream-yellow fill (#FFFACD / lemon chiffon)
    // with outline + text painted in the label's configured color (dark red).
    let fill_color = Color::from_rgb8(0xFF, 0xFA, 0xCD);

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
    let text_w = label.text.len() as f64 * fs * 0.6;
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
    // Tight text width — slightly under half the font size per glyph matches
    // the actual rendered widths for most monospace-ish labels.
    // Iced's `fill_text(size = S)` reserves roughly S of vertical space for
    // glyphs including descenders; visible glyphs sit slightly inside that
    // metric (left bearing + small descender space). Use measurements tuned
    // against a real canvas font so the bbox wraps the *visible* text.
    let tw = label.text.chars().count() as f64 * fs * 0.58;
    let baseline_off = pen_width_mm();
    let cap = baseline_off + fs * 1.05;
    // No inset — the visible first glyph actually aligns with the anchor,
    // so shifting the bbox inward leaves a visible gap on the leading side.
    let inset = 0.0_f64;
    let (x0, y0, x1, y1) = match label_spin_style(label) {
        // Text to the right of the anchor, above the anchor line.
        SpinStyle::Right => (
            label.position.x + inset,
            label.position.y - cap,
            label.position.x + inset + tw,
            label.position.y,
        ),
        // Text to the left of the anchor, above the anchor line.
        SpinStyle::Left => (
            label.position.x - inset - tw,
            label.position.y - cap,
            label.position.x - inset,
            label.position.y,
        ),
        // Rotated +90° — text extends upward from the anchor.
        SpinStyle::Up => (
            label.position.x,
            label.position.y - inset - tw,
            label.position.x + cap,
            label.position.y - inset,
        ),
        // Rotated -90° — text extends downward from the anchor.
        SpinStyle::Bottom => (
            label.position.x - cap,
            label.position.y + inset,
            label.position.x,
            label.position.y + inset + tw,
        ),
    };
    signex_types::schematic::Aabb::new(x0, y0, x1, y1)
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

fn schematic_text_offset_net(label: &Label, _font_size_mm: f64) -> (f64, f64) {
    // Altium places the net-label baseline right on the wire — just the
    // pen width's worth of clearance above the anchor.
    let dist = pen_width_mm();
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
                content: display_text_content(&label.text),
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
                    content: display_text_content(&label.text),
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
