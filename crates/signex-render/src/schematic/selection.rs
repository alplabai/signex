//! Selection overlay rendering -- draws highlight around selected elements.

use iced::Color;
use iced::widget::canvas;

use signex_types::schematic::*;

use super::{SchematicRenderSnapshot, ScreenTransform, field_display_pos};

/// Selection outline color (Altium-style — thin outline, no fill).
const SEL_COLOR: Color = Color {
    r: 0.2,
    g: 0.85,
    b: 0.2,
    a: 0.9,
};

/// Draw selection highlights for all selected items.
pub fn draw_selection_overlay(
    frame: &mut canvas::Frame,
    sheet: &SchematicRenderSnapshot,
    selected: &[SelectedItem],
    transform: &ScreenTransform,
) {
    for item in selected {
        match item.kind {
            SelectedKind::Symbol => {
                if let Some(sym) = sheet.symbols.iter().find(|s| s.uuid == item.uuid) {
                    // Power ports render via the built-in power glyph even when
                    // a lib_sym exists, so their selection rectangle must come
                    // from the built-in geometry too — not the (possibly stale)
                    // library graphics.
                    if sym.is_power {
                        draw_power_port_selection(frame, sym, transform);
                    } else if let Some(lib_sym) = sheet.lib_symbols.get(&sym.lib_id) {
                        draw_symbol_selection(frame, sym, lib_sym, transform);
                    }
                }
            }
            SelectedKind::Wire => {
                if let Some(w) = sheet.wires.iter().find(|w| w.uuid == item.uuid) {
                    draw_wire_selection(frame, w, transform);
                }
            }
            SelectedKind::Bus => {
                if let Some(b) = sheet.buses.iter().find(|b| b.uuid == item.uuid) {
                    draw_bus_selection(frame, b, transform);
                }
            }
            SelectedKind::Label => {
                if let Some(l) = sheet.labels.iter().find(|l| l.uuid == item.uuid) {
                    draw_label_selection(frame, l, transform);
                }
            }
            SelectedKind::Junction => {
                if let Some(j) = sheet.junctions.iter().find(|j| j.uuid == item.uuid) {
                    draw_point_selection(frame, j.position.x, j.position.y, transform);
                }
            }
            SelectedKind::NoConnect => {
                if let Some(nc) = sheet.no_connects.iter().find(|n| n.uuid == item.uuid) {
                    draw_point_selection(frame, nc.position.x, nc.position.y, transform);
                }
            }
            SelectedKind::TextNote => {
                if let Some(tn) = sheet.text_notes.iter().find(|t| t.uuid == item.uuid) {
                    draw_text_selection(frame, tn, transform);
                }
            }
            SelectedKind::ChildSheet => {
                if let Some(cs) = sheet.child_sheets.iter().find(|c| c.uuid == item.uuid) {
                    draw_sheet_selection(frame, cs, transform);
                }
            }
            SelectedKind::BusEntry => {
                if let Some(be) = sheet.bus_entries.iter().find(|b| b.uuid == item.uuid) {
                    let ex = be.position.x + be.size.0;
                    let ey = be.position.y + be.size.1;
                    let aabb = Aabb::new(be.position.x, be.position.y, ex, ey).expand(1.0);
                    draw_rect_highlight(frame, &aabb, transform);
                }
            }
            SelectedKind::Drawing => {
                if let Some(d) = sheet.drawings.iter().find(|d| {
                    let u = match d {
                        SchDrawing::Line { uuid, .. }
                        | SchDrawing::Rect { uuid, .. }
                        | SchDrawing::Circle { uuid, .. }
                        | SchDrawing::Arc { uuid, .. }
                        | SchDrawing::Polyline { uuid, .. } => *uuid,
                    };
                    u == item.uuid
                }) {
                    draw_drawing_selection(frame, d, transform);
                }
            }
            SelectedKind::SymbolRefField => {
                if let Some(sym) = sheet.symbols.iter().find(|s| s.uuid == item.uuid)
                    && let Some(ref rt) = sym.ref_text
                {
                    draw_text_prop_selection(frame, &sym.reference, rt, sym, transform);
                }
            }
            SelectedKind::SymbolValField => {
                if let Some(sym) = sheet.symbols.iter().find(|s| s.uuid == item.uuid)
                    && let Some(ref vt) = sym.val_text
                {
                    draw_text_prop_selection(frame, &sym.value, vt, sym, transform);
                }
            }
        }
    }
}

/// Draw a tight selection rectangle around a symbol's reference/value text
/// property. Uses the same glyph metrics as the renderer so the box matches
/// the visible text bounds.
fn draw_text_prop_selection(
    frame: &mut canvas::Frame,
    content: &str,
    prop: &TextProp,
    sym: &Symbol,
    transform: &ScreenTransform,
) {
    use signex_types::schematic::{HAlign, VAlign};
    let fs = crate::SCHEMATIC_TEXT_MM;
    // `visible_char_count` drops KiCad `{slash}`-style escapes; Iosevka's
    // advance width is ≈0.55 em, so use the same coefficient the text
    // renderer uses below so bbox and glyphs line up.
    let tw = super::text::visible_char_count(content) as f64 * fs * 0.55;
    let th = fs;
    let (dx, dy) = field_display_pos(&prop.position, sym);
    // Mirror the effective alignment AND rotation the renderer computed so
    // the outline wraps the glyphs instead of a theoretical center-aligned
    // rect. For 90°-rotated text the reading and perpendicular axes swap:
    // `eff_h` now positions along the vertical axis, `eff_v` along the
    // horizontal axis.
    let (dr, eff_h, eff_v) = super::field_effective_style(prop, sym);
    let rotated = (dr - 90.0).abs() < 0.1;
    let (x0, x1, y0, y1) = if rotated {
        // Rotation -90° CCW on Iced canvas: reading direction maps from +X
        // (original) to −Y (screen). Left-justify → anchor at reading start
        // → bbox extends upward (smaller Y) by `tw`. Perpendicular extent is
        // `th` across the anchor; `eff_v::Top` maps to the left side of the
        // rotated glyph box, `Bottom` to the right.
        let (y0, y1) = match eff_h {
            HAlign::Left => (dy - tw, dy),
            HAlign::Right => (dy, dy + tw),
            HAlign::Center => (dy - tw * 0.5, dy + tw * 0.5),
        };
        let (x0, x1) = match eff_v {
            VAlign::Top => (dx, dx + th),
            VAlign::Bottom => (dx - th, dx),
            VAlign::Center => (dx - th * 0.5, dx + th * 0.5),
        };
        (x0, x1, y0, y1)
    } else {
        let (x0, x1) = match eff_h {
            HAlign::Left => (dx, dx + tw),
            HAlign::Right => (dx - tw, dx),
            HAlign::Center => (dx - tw * 0.5, dx + tw * 0.5),
        };
        let (y0, y1) = match eff_v {
            VAlign::Top => (dy, dy + th),
            VAlign::Bottom => (dy - th, dy),
            VAlign::Center => (dy - th * 0.5, dy + th * 0.5),
        };
        (x0, x1, y0, y1)
    };
    let aabb = Aabb::new(x0, y0, x1, y1).expand(0.15);
    draw_rect_highlight(frame, &aabb, transform);
}

/// Selection rectangle for built-in power ports (lib_sym may be absent).
/// Wraps the pin stub, body glyph, and value label together.
fn draw_power_port_selection(frame: &mut canvas::Frame, sym: &Symbol, transform: &ScreenTransform) {
    let id = sym.lib_id.to_lowercase();
    let is_gnd_like = id.contains("gnd");
    let dir: f64 = if is_gnd_like { -1.0 } else { 1.0 };
    let pin_len = 1.27_f64;
    let body_extent = if id.contains("gnd") && !id.contains("earth") && !id.contains("gndref") {
        1.2
    } else if id.contains("gndref") {
        1.27
    } else if id.contains("earth") {
        0.9
    } else if id.contains("arrow") {
        1.4
    } else if id.contains("circle") {
        1.2
    } else {
        0.0
    };
    let label_extent = crate::SCHEMATIC_TEXT_MM * 1.2; // cap + descender
    let half_w =
        1.8_f64.max(sym.value.chars().count() as f64 * crate::SCHEMATIC_TEXT_MM * 0.55 * 0.5);

    // In lib-local Y (before Y-flip in instance_transform): the pin stub
    // starts at the anchor (y = 0) and extends toward the body. Keep the
    // bbox anchored at 0 so the stub is covered, not just the body+label.
    let far = (pin_len + body_extent + label_extent + 0.4) * dir;
    let y_min = 0.0_f64.min(far);
    let y_max = 0.0_f64.max(far);
    let aabb = Aabb::new(-half_w, y_min, half_w, y_max).expand(0.25);

    // Transform the 4 lib-local corners through instance_transform and draw.
    let corners_lib = [
        (aabb.min_x, aabb.min_y),
        (aabb.max_x, aabb.min_y),
        (aabb.max_x, aabb.max_y),
        (aabb.min_x, aabb.max_y),
    ];
    let corners_screen: Vec<iced::Point> = corners_lib
        .iter()
        .map(|&(lx, ly)| {
            let (wx, wy) = lib_to_world(sym, lx, ly);
            transform.to_screen_point(wx, wy)
        })
        .collect();
    let path = canvas::Path::new(|b| {
        b.move_to(corners_screen[0]);
        for c in &corners_screen[1..] {
            b.line_to(*c);
        }
        b.close();
    });
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_color(SEL_COLOR)
            .with_width(1.0),
    );
    let dot_sz = 2.0;
    for c in &corners_screen {
        let grip = canvas::Path::rectangle(
            iced::Point::new(c.x - dot_sz, c.y - dot_sz),
            iced::Size::new(dot_sz * 2.0, dot_sz * 2.0),
        );
        frame.fill(&grip, SEL_COLOR);
    }
}

fn draw_symbol_selection(
    frame: &mut canvas::Frame,
    sym: &Symbol,
    lib_sym: &LibSymbol,
    transform: &ScreenTransform,
) {
    // Compute bounding box in lib space, then transform corners to screen
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    let mut has = false;

    for lg in &lib_sym.graphics {
        if lg.unit != 0 && lg.unit != sym.unit {
            continue;
        }
        if lg.body_style != 0 && lg.body_style != 1 {
            continue;
        }
        match &lg.graphic {
            Graphic::Rectangle { start, end, .. } => {
                ext(
                    &mut min_x, &mut min_y, &mut max_x, &mut max_y, start.x, start.y,
                );
                ext(&mut min_x, &mut min_y, &mut max_x, &mut max_y, end.x, end.y);
                has = true;
            }
            Graphic::Polyline { points, .. } => {
                for p in points {
                    ext(&mut min_x, &mut min_y, &mut max_x, &mut max_y, p.x, p.y);
                    has = true;
                }
            }
            Graphic::Circle { center, radius, .. } => {
                ext(
                    &mut min_x,
                    &mut min_y,
                    &mut max_x,
                    &mut max_y,
                    center.x - radius,
                    center.y - radius,
                );
                ext(
                    &mut min_x,
                    &mut min_y,
                    &mut max_x,
                    &mut max_y,
                    center.x + radius,
                    center.y + radius,
                );
                has = true;
            }
            Graphic::Arc {
                start, mid, end, ..
            } => {
                ext(
                    &mut min_x, &mut min_y, &mut max_x, &mut max_y, start.x, start.y,
                );
                ext(&mut min_x, &mut min_y, &mut max_x, &mut max_y, mid.x, mid.y);
                ext(&mut min_x, &mut min_y, &mut max_x, &mut max_y, end.x, end.y);
                has = true;
            }
            _ => {}
        }
    }
    for lp in &lib_sym.pins {
        if lp.unit != 0 && lp.unit != sym.unit {
            continue;
        }
        let p = &lp.pin;
        if !p.visible {
            continue;
        }
        ext(
            &mut min_x,
            &mut min_y,
            &mut max_x,
            &mut max_y,
            p.position.x,
            p.position.y,
        );
        let a = p.rotation.to_radians();
        ext(
            &mut min_x,
            &mut min_y,
            &mut max_x,
            &mut max_y,
            p.position.x + p.length * a.cos(),
            p.position.y + p.length * a.sin(),
        );
        has = true;
    }

    if !has {
        return;
    }

    // Transform lib corners through symbol transform to get screen-space box
    let margin = 0.5; // Tight box like Altium
    let corners_lib = [
        (min_x - margin, min_y - margin),
        (max_x + margin, min_y - margin),
        (max_x + margin, max_y + margin),
        (min_x - margin, max_y + margin),
    ];

    let mut corners_screen: Vec<iced::Point> = corners_lib
        .iter()
        .map(|&(lx, ly)| {
            let (wx, wy) = lib_to_world(sym, lx, ly);
            transform.to_screen_point(wx, wy)
        })
        .collect();

    // For power ports the net-name label (val_text) is the *subject* of the
    // selection, so it gets included. For regular components the designator
    // ("R13") and value ("10k") texts stay *outside* the selection rectangle
    // — matching Altium's behavior where the body-only bbox is the anchor.
    let include_val = if sym.is_power {
        sym.val_text.as_ref().filter(|t| !t.hidden)
    } else {
        None
    };
    let include_ref: Option<&signex_types::schematic::TextProp> = None;
    let text_boxes = [include_val, include_ref];
    if text_boxes.iter().any(|t| t.is_some()) {
        let mut bx_min_x = corners_screen.iter().map(|p| p.x).fold(f32::MAX, f32::min);
        let mut bx_max_x = corners_screen.iter().map(|p| p.x).fold(f32::MIN, f32::max);
        let mut bx_min_y = corners_screen.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let mut bx_max_y = corners_screen.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        let text_display = |t: &signex_types::schematic::TextProp, content: &str| {
            let fs = t.font_size.max(1.27);
            let tw = content.chars().count() as f64 * fs * 0.6;
            let th = fs * 1.2;
            let anchor = transform.to_screen_point(t.position.x, t.position.y);
            let half_w = transform.world_len(tw) * 0.5;
            let half_h = transform.world_len(th) * 0.5;
            (anchor, half_w, half_h)
        };
        if let Some(vt) = include_val {
            let (p, hw, hh) = text_display(vt, &sym.value);
            bx_min_x = bx_min_x.min(p.x - hw);
            bx_max_x = bx_max_x.max(p.x + hw);
            bx_min_y = bx_min_y.min(p.y - hh);
            bx_max_y = bx_max_y.max(p.y + hh);
        }
        if let Some(rt) = include_ref {
            let (p, hw, hh) = text_display(rt, &sym.reference);
            bx_min_x = bx_min_x.min(p.x - hw);
            bx_max_x = bx_max_x.max(p.x + hw);
            bx_min_y = bx_min_y.min(p.y - hh);
            bx_max_y = bx_max_y.max(p.y + hh);
        }
        corners_screen = vec![
            iced::Point::new(bx_min_x, bx_min_y),
            iced::Point::new(bx_max_x, bx_min_y),
            iced::Point::new(bx_max_x, bx_max_y),
            iced::Point::new(bx_min_x, bx_max_y),
        ];
    }

    // Draw the transformed rectangle
    let path = canvas::Path::new(|b| {
        b.move_to(corners_screen[0]);
        for c in &corners_screen[1..] {
            b.line_to(*c);
        }
        b.close();
    });

    // Altium-style: thin outline only, no fill, no corner grips
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_color(SEL_COLOR)
            .with_width(1.0),
    );

    // Small corner dots (Altium shows small squares at corners)
    let dot_sz = 2.0;
    for c in &corners_screen {
        let grip = canvas::Path::rectangle(
            iced::Point::new(c.x - dot_sz, c.y - dot_sz),
            iced::Size::new(dot_sz * 2.0, dot_sz * 2.0),
        );
        frame.fill(&grip, SEL_COLOR);
    }
}

fn draw_wire_selection(frame: &mut canvas::Frame, w: &Wire, transform: &ScreenTransform) {
    let s = transform.to_screen_point(w.start.x, w.start.y);
    let e = transform.to_screen_point(w.end.x, w.end.y);

    let path = canvas::Path::line(s, e);
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_color(SEL_COLOR)
            .with_width(3.0),
    );

    // Endpoint grips
    for p in &[s, e] {
        let grip = canvas::Path::rectangle(
            iced::Point::new(p.x - 3.0, p.y - 3.0),
            iced::Size::new(6.0, 6.0),
        );
        frame.fill(&grip, SEL_COLOR);
    }
}

fn draw_bus_selection(frame: &mut canvas::Frame, b: &Bus, transform: &ScreenTransform) {
    let s = transform.to_screen_point(b.start.x, b.start.y);
    let e = transform.to_screen_point(b.end.x, b.end.y);

    let path = canvas::Path::line(s, e);
    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_color(SEL_COLOR)
            .with_width(4.0),
    );
    for p in &[s, e] {
        let grip = canvas::Path::rectangle(
            iced::Point::new(p.x - 3.0, p.y - 3.0),
            iced::Size::new(6.0, 6.0),
        );
        frame.fill(&grip, SEL_COLOR);
    }
}

fn draw_label_selection(frame: &mut canvas::Frame, l: &Label, transform: &ScreenTransform) {
    let aabb = super::label::label_text_aabb(l);
    draw_rect_highlight(frame, &aabb, transform);
}

fn draw_text_selection(frame: &mut canvas::Frame, tn: &TextNote, transform: &ScreenTransform) {
    let tw = tn.text.chars().count() as f64 * tn.font_size.max(1.27) * 0.7;
    let th = tn.font_size.max(1.27) * 1.5;
    let aabb = Aabb::new(
        tn.position.x,
        tn.position.y - th,
        tn.position.x + tw,
        tn.position.y + th * 0.5,
    )
    .expand(0.5);
    draw_rect_highlight(frame, &aabb, transform);
}

fn draw_sheet_selection(frame: &mut canvas::Frame, cs: &ChildSheet, transform: &ScreenTransform) {
    let aabb = Aabb::new(
        cs.position.x,
        cs.position.y,
        cs.position.x + cs.size.0,
        cs.position.y + cs.size.1,
    )
    .expand(1.0);
    draw_rect_highlight(frame, &aabb, transform);
}

fn draw_point_selection(frame: &mut canvas::Frame, x: f64, y: f64, transform: &ScreenTransform) {
    let p = transform.to_screen_point(x, y);
    let r = 6.0;
    let circle = canvas::Path::circle(p, r);
    frame.stroke(
        &circle,
        canvas::Stroke::default()
            .with_color(SEL_COLOR)
            .with_width(1.5),
    );
}

fn draw_rect_highlight(frame: &mut canvas::Frame, aabb: &Aabb, transform: &ScreenTransform) {
    let tl = transform.to_screen_point(aabb.min_x, aabb.min_y);
    let br = transform.to_screen_point(aabb.max_x, aabb.max_y);
    let w = br.x - tl.x;
    let h = br.y - tl.y;
    if w.abs() < 1.0 || h.abs() < 1.0 {
        return;
    }

    // Altium-style: thin outline only, no fill
    let rect = canvas::Path::rectangle(tl, iced::Size::new(w, h));
    frame.stroke(
        &rect,
        canvas::Stroke::default()
            .with_color(SEL_COLOR)
            .with_width(1.0),
    );

    // Small corner dots
    let dot_sz = 2.0;
    for p in &[
        tl,
        iced::Point::new(br.x, tl.y),
        br,
        iced::Point::new(tl.x, br.y),
    ] {
        let grip = canvas::Path::rectangle(
            iced::Point::new(p.x - dot_sz, p.y - dot_sz),
            iced::Size::new(dot_sz * 2.0, dot_sz * 2.0),
        );
        frame.fill(&grip, SEL_COLOR);
    }
}

/// Transform library-local coords to world coords through symbol rotation/mirror/position.
fn lib_to_world(sym: &Symbol, lx: f64, ly: f64) -> (f64, f64) {
    // Y-flip (lib Y-up to schematic Y-down)
    let fy = -ly;

    // Rotate by -rotation
    let angle = (-sym.rotation).to_radians();
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    let mut rx = lx * cos_a - fy * sin_a;
    let mut ry = lx * sin_a + fy * cos_a;

    // Mirror
    if sym.mirror_x {
        ry = -ry;
    }
    if sym.mirror_y {
        rx = -rx;
    }

    (sym.position.x + rx, sym.position.y + ry)
}

fn ext(min_x: &mut f64, min_y: &mut f64, max_x: &mut f64, max_y: &mut f64, x: f64, y: f64) {
    *min_x = min_x.min(x);
    *min_y = min_y.min(y);
    *max_x = max_x.max(x);
    *max_y = max_y.max(y);
}

/// Highlight a graphic drawing. For each shape kind we either repaint
/// the geometry in the selection colour (line / arc / polyline) or
/// surround it with a thin selection bbox (rect / circle).
fn draw_drawing_selection(
    frame: &mut canvas::Frame,
    drawing: &SchDrawing,
    transform: &ScreenTransform,
) {
    let stroke = canvas::Stroke::default()
        .with_color(SEL_COLOR)
        .with_width(2.0);
    match drawing {
        SchDrawing::Line { start, end, .. } => {
            let (sx, sy) = transform.world_to_screen(start.x, start.y);
            let (ex, ey) = transform.world_to_screen(end.x, end.y);
            frame.stroke(
                &canvas::Path::line(iced::Point::new(sx, sy), iced::Point::new(ex, ey)),
                stroke,
            );
        }
        SchDrawing::Rect { start, end, .. } => {
            let x0 = start.x.min(end.x);
            let x1 = start.x.max(end.x);
            let y0 = start.y.min(end.y);
            let y1 = start.y.max(end.y);
            let aabb = Aabb::new(x0, y0, x1, y1).expand(0.5);
            draw_rect_highlight(frame, &aabb, transform);
        }
        SchDrawing::Circle { center, radius, .. } => {
            let (cx, cy) = transform.world_to_screen(center.x, center.y);
            let rs = transform.world_len(*radius).abs();
            frame.stroke(&canvas::Path::circle(iced::Point::new(cx, cy), rs), stroke);
        }
        SchDrawing::Arc {
            start, mid, end, ..
        } => {
            let (sx, sy) = transform.world_to_screen(start.x, start.y);
            let (mx, my) = transform.world_to_screen(mid.x, mid.y);
            let (ex, ey) = transform.world_to_screen(end.x, end.y);
            frame.stroke(
                &canvas::Path::line(iced::Point::new(sx, sy), iced::Point::new(mx, my)),
                stroke,
            );
            frame.stroke(
                &canvas::Path::line(iced::Point::new(mx, my), iced::Point::new(ex, ey)),
                stroke,
            );
        }
        SchDrawing::Polyline { points, .. } => {
            for pair in points.windows(2) {
                let (sx, sy) = transform.world_to_screen(pair[0].x, pair[0].y);
                let (ex, ey) = transform.world_to_screen(pair[1].x, pair[1].y);
                frame.stroke(
                    &canvas::Path::line(iced::Point::new(sx, sy), iced::Point::new(ex, ey)),
                    stroke,
                );
            }
        }
    }
}
