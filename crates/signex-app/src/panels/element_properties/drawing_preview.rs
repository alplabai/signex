//! Live shape-preview canvas widget (`DrawingPreview`) shown above the
//! Drawing properties rows, plus its bounding-box / circumcircle / arc-
//! sweep geometry helpers. Moved verbatim from the former single-file
//! `element_properties` module.

use super::super::*;

// ─── Drawing preview widget ─────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DrawingPreview {
    pub drawing: signex_types::schematic::SchDrawing,
    pub stroke: Color,
    pub fill: Color,
    pub muted: Color,
    pub accent: Color,
}

impl<Message> canvas::Program<Message> for DrawingPreview {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        use signex_types::schematic::SchDrawing;
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let pad = 14.0_f32;
        let view_w = (bounds.width - 2.0 * pad).max(20.0);
        let view_h = (bounds.height - 2.0 * pad).max(20.0);
        let cx_px = bounds.width / 2.0;
        let cy_px = bounds.height / 2.0;

        let (min_x, min_y, max_x, max_y) = shape_preview_bbox(&self.drawing);
        let span_w = (max_x - min_x).abs().max(0.1);
        let span_h = (max_y - min_y).abs().max(0.1);
        let scale = (view_w as f64 / span_w).min(view_h as f64 / span_h) as f32;
        let wcx = (min_x + max_x) * 0.5;
        let wcy = (min_y + max_y) * 0.5;
        let w2s = |wx: f64, wy: f64| -> Point {
            Point::new(
                cx_px + ((wx - wcx) as f32) * scale,
                cy_px + ((wy - wcy) as f32) * scale,
            )
        };

        let stroke = canvas::Stroke::default()
            .with_color(self.stroke)
            .with_width(1.8);
        let dashed = canvas::Stroke::default()
            .with_color(Color {
                a: 0.4,
                ..self.muted
            })
            .with_width(1.0);
        let annotation = canvas::Stroke::default()
            .with_color(self.accent)
            .with_width(1.4);

        match &self.drawing {
            SchDrawing::Line { start, end, .. } => {
                frame.stroke(
                    &canvas::Path::line(w2s(start.x, start.y), w2s(end.x, end.y)),
                    stroke,
                );
                let dot = |f: &mut canvas::Frame, p: Point| {
                    f.fill(&canvas::Path::circle(p, 3.0), self.accent);
                };
                dot(&mut frame, w2s(start.x, start.y));
                dot(&mut frame, w2s(end.x, end.y));
            }
            SchDrawing::Rect {
                start, end, fill, ..
            } => {
                let x0 = start.x.min(end.x);
                let x1 = start.x.max(end.x);
                let y0 = start.y.min(end.y);
                let y1 = start.y.max(end.y);
                let a = w2s(x0, y0);
                let b = w2s(x1, y1);
                let rect_pos = Point::new(a.x.min(b.x), a.y.min(b.y));
                let rect_size =
                    iced::Size::new((b.x - a.x).abs().max(1.0), (b.y - a.y).abs().max(1.0));
                let path = canvas::Path::rectangle(rect_pos, rect_size);
                if !matches!(fill, signex_types::schematic::FillType::None) {
                    frame.fill(
                        &path,
                        Color {
                            a: 0.25,
                            ..self.fill
                        },
                    );
                }
                frame.stroke(&path, stroke);
            }
            SchDrawing::Circle {
                center,
                radius,
                fill,
                ..
            } => {
                let cp = w2s(center.x, center.y);
                let rs = (*radius as f32) * scale;
                let path = canvas::Path::circle(cp, rs.max(1.0));
                if !matches!(fill, signex_types::schematic::FillType::None) {
                    frame.fill(
                        &path,
                        Color {
                            a: 0.22,
                            ..self.fill
                        },
                    );
                }
                frame.stroke(&path, stroke);
                let spoke = canvas::Path::line(cp, Point::new(cp.x + rs, cp.y));
                frame.stroke(&spoke, annotation);
            }
            SchDrawing::Arc {
                start, mid, end, ..
            } => {
                if let Some((cxw, cyw, rw)) =
                    signex_types::schematic::circumcircle(*start, *mid, *end)
                {
                    let cp = w2s(cxw, cyw);
                    let rs = (rw as f32) * scale;
                    frame.stroke(&canvas::Path::circle(cp, rs.max(1.0)), dashed);
                    let sa = (start.y - cyw).atan2(start.x - cxw);
                    let ea = (end.y - cyw).atan2(end.x - cxw);
                    let ma = (mid.y - cyw).atan2(mid.x - cxw);
                    let (from, to) = arc_sweep_local(sa, ma, ea);
                    let steps = 64_usize;
                    let mut prev = w2s(start.x, start.y);
                    for i in 1..=steps {
                        let t = i as f64 / steps as f64;
                        let a = from + (to - from) * t;
                        let wx = cxw + rw * a.cos();
                        let wy = cyw + rw * a.sin();
                        let next = w2s(wx, wy);
                        frame.stroke(&canvas::Path::line(prev, next), stroke);
                        prev = next;
                    }
                    frame.stroke(&canvas::Path::line(cp, w2s(start.x, start.y)), annotation);
                    frame.stroke(&canvas::Path::line(cp, w2s(end.x, end.y)), annotation);
                } else {
                    frame.stroke(
                        &canvas::Path::line(w2s(start.x, start.y), w2s(mid.x, mid.y)),
                        stroke,
                    );
                    frame.stroke(
                        &canvas::Path::line(w2s(mid.x, mid.y), w2s(end.x, end.y)),
                        stroke,
                    );
                }
            }
            SchDrawing::Polyline { points, fill, .. } => {
                if points.len() >= 2 {
                    let close = !matches!(fill, signex_types::schematic::FillType::None)
                        && points.len() >= 3;
                    let path = canvas::Path::new(|b| {
                        let first = w2s(points[0].x, points[0].y);
                        b.move_to(first);
                        for p in &points[1..] {
                            b.line_to(w2s(p.x, p.y));
                        }
                        if close {
                            b.close();
                        }
                    });
                    if close {
                        frame.fill(
                            &path,
                            Color {
                                a: 0.22,
                                ..self.fill
                            },
                        );
                    }
                    frame.stroke(&path, stroke);
                    for p in points {
                        let sp = w2s(p.x, p.y);
                        frame.fill(&canvas::Path::circle(sp, 2.5), self.accent);
                    }
                }
            }
        }

        vec![frame.into_geometry()]
    }
}

fn shape_preview_bbox(d: &signex_types::schematic::SchDrawing) -> (f64, f64, f64, f64) {
    use signex_types::schematic::SchDrawing;
    match d {
        SchDrawing::Line { start, end, .. } | SchDrawing::Rect { start, end, .. } => (
            start.x.min(end.x),
            start.y.min(end.y),
            start.x.max(end.x),
            start.y.max(end.y),
        ),
        SchDrawing::Circle { center, radius, .. } => (
            center.x - *radius,
            center.y - *radius,
            center.x + *radius,
            center.y + *radius,
        ),
        SchDrawing::Arc {
            start, mid, end, ..
        } => {
            let xs = [start.x, mid.x, end.x];
            let ys = [start.y, mid.y, end.y];
            let min_x = xs.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_x = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let min_y = ys.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_y = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            (min_x, min_y, max_x, max_y)
        }
        SchDrawing::Polyline { points, .. } => {
            if points.is_empty() {
                return (-1.0, -1.0, 1.0, 1.0);
            }
            let mut min_x = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for p in points {
                min_x = min_x.min(p.x);
                min_y = min_y.min(p.y);
                max_x = max_x.max(p.x);
                max_y = max_y.max(p.y);
            }
            (min_x, min_y, max_x, max_y)
        }
    }
}

fn arc_sweep_local(s: f64, m: f64, e: f64) -> (f64, f64) {
    use std::f64::consts::TAU;
    let norm = |a: f64| -> f64 {
        let mut t = a % TAU;
        if t < 0.0 {
            t += TAU;
        }
        t
    };
    let ccw = |a: f64, b: f64| -> f64 {
        let d = b - a;
        if d < 0.0 { d + TAU } else { d }
    };
    let sn = norm(s);
    let mn = norm(m);
    let en = norm(e);
    let s_to_m = ccw(sn, mn);
    let s_to_e = ccw(sn, en);
    if s_to_m <= s_to_e {
        (s, s + s_to_e)
    } else {
        (s, s - (TAU - s_to_e))
    }
}
