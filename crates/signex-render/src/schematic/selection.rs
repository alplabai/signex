//! Selection overlay rendering -- draws highlight around selected elements.

use iced::Color;
use iced::widget::canvas;

use signex_types::schematic::*;

use super::ScreenTransform;

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
    sheet: &SchematicSheet,
    selected: &[SelectedItem],
    transform: &ScreenTransform,
) {
    for item in selected {
        match item.kind {
            SelectedKind::Symbol => {
                if let Some(sym) = sheet.symbols.iter().find(|s| s.uuid == item.uuid)
                    && let Some(lib_sym) = sheet.lib_symbols.get(&sym.lib_id)
                {
                    draw_symbol_selection(frame, sym, lib_sym, transform);
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
            SelectedKind::Drawing => {}
        }
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

    let corners_screen: Vec<iced::Point> = corners_lib
        .iter()
        .map(|&(lx, ly)| {
            let (wx, wy) = lib_to_world(sym, lx, ly);
            transform.to_screen_point(wx, wy)
        })
        .collect();

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
    let tw = l.text.chars().count() as f64 * l.font_size.max(1.27) * 0.7;
    let th = l.font_size.max(1.27) * 1.5;
    let aabb = Aabb::new(
        l.position.x,
        l.position.y - th,
        l.position.x + tw,
        l.position.y + th * 0.5,
    )
    .expand(0.5);
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
