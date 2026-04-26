//! Procedural 3D preview pane.
//!
//! WS-F: stub-quality CPU isometric render of the footprint's pads
//! (extruded as boxes), the courtyard outline, and the Body3D box
//! straight from `Footprint::body_3d`. The render is intentionally
//! cheap — a single `iced::widget::Canvas` with no GPU pipeline.
//!
//! TODO(v0.9-phase-3 / v2.x): real 3D pipeline per
//! `docs/internal/docs/PCB_3D_RENDER_PLAN.md` (wgpu Shader widget
//! + STEP geometry triangulation + lighting model).

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Theme, mouse};
use signex_library::{BodyShape, Footprint};

use crate::library::messages::LibraryMessage;

/// Render the procedural 3D preview as an iced canvas widget.
pub fn view<'a>(fp: &'a Footprint) -> Element<'a, LibraryMessage> {
    let program = Preview3D { fp };
    iced::widget::Canvas::new(program)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

struct Preview3D<'a> {
    fp: &'a Footprint,
}

impl<'a> canvas::Program<LibraryMessage> for Preview3D<'a> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        // Background — tone darker than the 2D editor so the eye can
        // tell the panes apart.
        frame.fill_rectangle(
            Point::ORIGIN,
            bounds.size(),
            Color::from_rgb(0.06, 0.07, 0.08),
        );

        // Compute world-space bbox covering pads + body + courtyard.
        let bbox = compute_bbox(self.fp);
        let (w_min_x, w_min_y, w_max_x, w_max_y) = bbox;
        let world_w = (w_max_x - w_min_x).max(1.0);
        let world_h = (w_max_y - w_min_y).max(1.0);

        // Iso projection: scaled fit in the canvas. We paint a
        // 30-degree iso (cos30 / sin30 ~= 0.866 / 0.5) so the body box
        // pops as a parallelepiped without a perspective camera.
        let pad = 18.0_f32;
        let avail_w = (bounds.width - pad * 2.0).max(40.0);
        let avail_h = (bounds.height - pad * 2.0).max(40.0);
        let scale = (avail_w / world_w as f32 * 0.85)
            .min(avail_h / world_h as f32 * 0.55)
            .max(2.0);

        // Origin (pad-plane). Y axis flipped so positive-y goes "up".
        let cx = bounds.width * 0.5;
        let cy = bounds.height * 0.65;
        let project = |x: f64, y: f64, z: f64| -> Point {
            // x, y in mm; z in mm above PCB.
            // Iso projection: 30° rotation about origin.
            let cos30 = 0.8660254_f32;
            let sin30 = 0.5_f32;
            let xs = x as f32 * scale;
            let ys = y as f32 * scale;
            let zs = z as f32 * scale;
            let sx = (xs - ys) * cos30;
            let sy = -(xs + ys) * sin30 - zs;
            Point::new(cx + sx, cy + sy)
        };

        // Draw board base (a thin slab).
        let pcb_color = Color::from_rgb(0.10, 0.16, 0.10);
        let board_pts = [
            project(w_min_x - 0.5, w_min_y - 0.5, 0.0),
            project(w_max_x + 0.5, w_min_y - 0.5, 0.0),
            project(w_max_x + 0.5, w_max_y + 0.5, 0.0),
            project(w_min_x - 0.5, w_max_y + 0.5, 0.0),
        ];
        fill_quad(&mut frame, &board_pts, pcb_color);
        stroke_quad(&mut frame, &board_pts, Color::from_rgba(0.30, 0.55, 0.30, 1.0));

        // Pads: each pad is a thin extruded rect.
        let pad_height = 0.05_f64; // 50 µm — matches solder paste thickness.
        for pad in &self.fp.pads {
            let cx_w = pad.position[0];
            let cy_w = pad.position[1];
            let hw = pad.size[0] * 0.5;
            let hh = pad.size[1] * 0.5;
            let layer_color = pad_color(pad);
            let pts_top = [
                project(cx_w - hw, cy_w - hh, pad_height),
                project(cx_w + hw, cy_w - hh, pad_height),
                project(cx_w + hw, cy_w + hh, pad_height),
                project(cx_w - hw, cy_w + hh, pad_height),
            ];
            fill_quad(&mut frame, &pts_top, layer_color);
            stroke_quad(
                &mut frame,
                &pts_top,
                Color {
                    a: 1.0,
                    ..layer_color
                },
            );
        }

        // Courtyard outline — drawn as a thin yellow stroke at z=0.
        if !self.fp.courtyard.points.is_empty() {
            let courtyard_pts: Vec<Point> = self
                .fp
                .courtyard
                .points
                .iter()
                .map(|[x, y]| project(*x, *y, 0.0))
                .collect();
            let path = Path::new(|b| {
                if let Some((first, rest)) = courtyard_pts.split_first() {
                    b.move_to(*first);
                    for p in rest {
                        b.line_to(*p);
                    }
                    b.close();
                }
            });
            frame.stroke(
                &path,
                Stroke::default()
                    .with_color(Color::from_rgba(0.95, 0.85, 0.20, 0.85))
                    .with_width(1.5),
            );
        }

        // Body3D box.
        let body = &self.fp.body_3d;
        let body_bbox = body_bbox(self.fp);
        let bz = body.offset_z_mm as f64;
        let tz = bz + body.height_mm as f64;
        let (bx0, by0, bx1, by1) = body_bbox;
        let bottom = [
            project(bx0, by0, bz),
            project(bx1, by0, bz),
            project(bx1, by1, bz),
            project(bx0, by1, bz),
        ];
        let top = [
            project(bx0, by0, tz),
            project(bx1, by0, tz),
            project(bx1, by1, tz),
            project(bx0, by1, tz),
        ];
        let side_c = Color::from_rgba(
            body.side_color[0],
            body.side_color[1],
            body.side_color[2],
            body.side_color[3].max(0.85),
        );
        let top_c = Color::from_rgba(
            body.top_color[0],
            body.top_color[1],
            body.top_color[2],
            body.top_color[3].max(0.85),
        );

        // Side faces — front-right + front-left visible from the iso angle.
        match body.shape {
            BodyShape::Extrude | BodyShape::Custom => {
                fill_quad(&mut frame, &[bottom[0], bottom[1], top[1], top[0]], side_c);
                fill_quad(&mut frame, &[bottom[1], bottom[2], top[2], top[1]], side_c);
                fill_quad(&mut frame, &top, top_c);
                stroke_quad(&mut frame, &top, Color::from_rgba(0.0, 0.0, 0.0, 0.35));
            }
            BodyShape::Dome => {
                // Approximate dome as ellipse arc on top + extruded side.
                fill_quad(&mut frame, &[bottom[0], bottom[1], top[1], top[0]], side_c);
                fill_quad(&mut frame, &[bottom[1], bottom[2], top[2], top[1]], side_c);
                let cx_p = (top[0].x + top[2].x) * 0.5;
                let cy_p = (top[0].y + top[2].y) * 0.5;
                let rx = (top[1].x - top[0].x).abs() * 0.5;
                let ry = (top[1].y - top[0].y).abs() * 0.5 + body.height_mm * scale * 0.5;
                let dome = Path::new(|b| {
                    b.move_to(Point::new(cx_p - rx, cy_p));
                    // Half-ellipse (top half).
                    let steps = 24;
                    for i in 0..=steps {
                        let t = std::f32::consts::PI * (i as f32) / (steps as f32);
                        let x = cx_p - rx * t.cos();
                        let y = cy_p - ry * t.sin();
                        b.line_to(Point::new(x, y));
                    }
                    b.close();
                });
                frame.fill(&dome, top_c);
                frame.stroke(
                    &dome,
                    Stroke::default()
                        .with_color(Color::from_rgba(0.0, 0.0, 0.0, 0.35))
                        .with_width(1.0),
                );
            }
            BodyShape::Cylinder => {
                // Short cylinder approximation.
                fill_quad(&mut frame, &[bottom[0], bottom[1], top[1], top[0]], side_c);
                fill_quad(&mut frame, &[bottom[1], bottom[2], top[2], top[1]], side_c);
                fill_quad(&mut frame, &top, top_c);
                stroke_quad(&mut frame, &top, Color::from_rgba(0.0, 0.0, 0.0, 0.35));
            }
            _ => {
                fill_quad(&mut frame, &top, top_c);
                stroke_quad(&mut frame, &top, Color::from_rgba(0.0, 0.0, 0.0, 0.35));
            }
        }

        // Hint label.
        frame.fill_text(canvas::Text {
            content: "3D preview (iso) — TODO(v2.x): wgpu pipeline".to_string(),
            position: Point::new(8.0, 8.0),
            size: 10.0.into(),
            color: Color::from_rgba(0.85, 0.88, 0.92, 0.55),
            ..canvas::Text::default()
        });

        vec![frame.into_geometry()]
    }
}

fn fill_quad(frame: &mut canvas::Frame, pts: &[Point; 4], color: Color) {
    let path = Path::new(|b| {
        b.move_to(pts[0]);
        b.line_to(pts[1]);
        b.line_to(pts[2]);
        b.line_to(pts[3]);
        b.close();
    });
    frame.fill(&path, color);
}

fn stroke_quad(frame: &mut canvas::Frame, pts: &[Point; 4], color: Color) {
    let path = Path::new(|b| {
        b.move_to(pts[0]);
        b.line_to(pts[1]);
        b.line_to(pts[2]);
        b.line_to(pts[3]);
        b.close();
    });
    frame.stroke(&path, Stroke::default().with_color(color).with_width(1.0));
}

fn compute_bbox(fp: &Footprint) -> (f64, f64, f64, f64) {
    let mut bbox: Option<(f64, f64, f64, f64)> = None;
    let mut grow = |x0: f64, y0: f64, x1: f64, y1: f64| {
        bbox = Some(match bbox {
            Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
            None => (x0, y0, x1, y1),
        });
    };
    for p in &fp.pads {
        let hw = p.size[0] * 0.5;
        let hh = p.size[1] * 0.5;
        grow(
            p.position[0] - hw,
            p.position[1] - hh,
            p.position[0] + hw,
            p.position[1] + hh,
        );
    }
    for v in &fp.courtyard.points {
        grow(v[0], v[1], v[0], v[1]);
    }
    bbox.unwrap_or((-3.0, -2.0, 3.0, 2.0))
}

fn body_bbox(fp: &Footprint) -> (f64, f64, f64, f64) {
    if let Some(outline) = fp.body_3d.outline.as_ref() {
        let mut bbox: Option<(f64, f64, f64, f64)> = None;
        for v in &outline.points {
            bbox = Some(match bbox {
                Some((a, b, c, d)) => (a.min(v[0]), b.min(v[1]), c.max(v[0]), d.max(v[1])),
                None => (v[0], v[1], v[0], v[1]),
            });
        }
        if let Some(bb) = bbox {
            return bb;
        }
    }
    // Default: shrink the pad/courtyard bbox by 0.5mm so the body
    // doesn't crash through the courtyard.
    let (x0, y0, x1, y1) = compute_bbox(fp);
    let mx = 0.5_f64;
    (x0 + mx, y0 + mx, x1 - mx, y1 - mx)
}

fn pad_color(pad: &signex_library::Pad) -> Color {
    // Pick a colour from the pad's primary layer name. Falls back to
    // a generic copper colour for unknown layers.
    let name = pad
        .layers
        .first()
        .map(|l| l.as_str())
        .unwrap_or("F.Cu");
    match name {
        "F.Cu" => Color::from_rgba(0.85, 0.55, 0.20, 1.0),
        "B.Cu" => Color::from_rgba(0.30, 0.45, 0.90, 1.0),
        _ => Color::from_rgba(0.75, 0.50, 0.20, 1.0),
    }
}
