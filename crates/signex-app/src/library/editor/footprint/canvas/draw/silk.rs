//! Silk-layer graphics renderer. Used for both `silk_f` (front) and
//! `silk_b` (back) passes; the layer colour follows whichever layer
//! the caller passes.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point};
use signex_library::primitive::footprint::FpGraphicKind;

use super::super::super::layers::FpLayer;
use super::super::FootprintCanvasState;

/// v0.18.16 — render the silk-layer graphics list. Each `FpGraphic`
/// becomes a single Path stroke / fill in the layer's colour.
pub(super) fn draw_silk_graphics(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    graphics: &[signex_library::primitive::footprint::FpGraphic],
    layer: FpLayer,
    selected_idx: Option<usize>,
) {
    let base_colour = layer.color();
    let highlight = Color::from_rgb(1.0, 1.0, 1.0);
    let stroke_default_px: f32 = 1.0;
    for (idx, g) in graphics.iter().enumerate() {
        let is_selected = selected_idx == Some(idx);
        let colour = if is_selected { highlight } else { base_colour };
        let mut stroke_px = if g.stroke_width > 0.0 {
            crate::renderer_scene_canvas::min_stroke_px(g.stroke_width as f32 * cstate.scale)
        } else {
            stroke_default_px
        };
        if is_selected {
            stroke_px = (stroke_px + 1.0).max(2.0);
        }
        match &g.kind {
            FpGraphicKind::Line { from, to } => {
                let p0 = cstate.world_to_screen((from[0], from[1]));
                let p1 = cstate.world_to_screen((to[0], to[1]));
                frame.stroke(
                    &Path::line(p0, p1),
                    Stroke::default().with_width(stroke_px).with_color(colour),
                );
            }
            FpGraphicKind::Rectangle { from, to } => {
                let p0 = cstate.world_to_screen((from[0], from[1]));
                let p1 = cstate.world_to_screen((to[0], to[1]));
                let rect = Path::rectangle(
                    Point::new(p0.x.min(p1.x), p0.y.min(p1.y)),
                    iced::Size::new((p1.x - p0.x).abs(), (p1.y - p0.y).abs()),
                );
                frame.stroke(
                    &rect,
                    Stroke::default().with_width(stroke_px).with_color(colour),
                );
            }
            FpGraphicKind::Circle { center, radius } => {
                let c = cstate.world_to_screen((center[0], center[1]));
                let r_px = (*radius as f32) * cstate.scale;
                frame.stroke(
                    &Path::circle(c, r_px),
                    Stroke::default().with_width(stroke_px).with_color(colour),
                );
            }
            FpGraphicKind::Arc {
                center,
                radius,
                start_deg,
                end_deg,
            } => {
                let c_world = (*center)[0..2].try_into().unwrap_or([0.0, 0.0]);
                let c = cstate.world_to_screen((c_world[0], c_world[1]));
                let r_px = (*radius as f32) * cstate.scale;
                let pts = arc_polyline(c, r_px, *start_deg, *end_deg, 64);
                let path = Path::new(|builder| {
                    builder.move_to(pts[0]);
                    for p in &pts[1..] {
                        builder.line_to(*p);
                    }
                });
                frame.stroke(
                    &path,
                    Stroke::default().with_width(stroke_px).with_color(colour),
                );
            }
            FpGraphicKind::Text {
                position,
                content,
                size,
                frame: text_frame,
            } => {
                let p = cstate.world_to_screen((position[0], position[1]));
                match text_frame {
                    // v0.14 — bounding-box Text Frame (item ③). Top-
                    // left anchored at `position`, same as the
                    // point-text branch below; the only difference is
                    // a guide-rectangle stroke and a size clamp so
                    // the string doesn't overflow the box's height.
                    // No auto-wrap/reflow — a string wider than the
                    // frame simply overruns it horizontally, exactly
                    // like Altium's non-autosize text frames.
                    Some((w, h)) => {
                        let p1 = cstate
                            .world_to_screen((position[0] + *w as f64, position[1] + *h as f64));
                        let rect = Path::rectangle(
                            Point::new(p.x.min(p1.x), p.y.min(p1.y)),
                            iced::Size::new((p1.x - p.x).abs(), (p1.y - p.y).abs()),
                        );
                        frame.stroke(
                            &rect,
                            Stroke::default().with_width(stroke_px).with_color(colour),
                        );
                        let size_px = ((*size as f32) * cstate.scale)
                            .min((p1.y - p.y).abs())
                            .max(4.0);
                        frame.fill_text(canvas::Text {
                            content: content.clone(),
                            position: Point::new(p.x, p.y),
                            size: size_px.into(),
                            color: colour,
                            align_x: iced::alignment::Horizontal::Left.into(),
                            align_y: iced::alignment::Vertical::Top,
                            ..canvas::Text::default()
                        });
                    }
                    None => {
                        let size_px = ((*size as f32) * cstate.scale).max(8.0);
                        frame.fill_text(canvas::Text {
                            content: content.clone(),
                            position: Point::new(p.x, p.y),
                            size: size_px.into(),
                            color: colour,
                            align_x: iced::alignment::Horizontal::Left.into(),
                            align_y: iced::alignment::Vertical::Top,
                            ..canvas::Text::default()
                        });
                    }
                }
            }
            FpGraphicKind::Polygon { vertices } => {
                if vertices.len() < 2 {
                    continue;
                }
                let path = Path::new(|builder| {
                    let first = cstate.world_to_screen((vertices[0][0], vertices[0][1]));
                    builder.move_to(first);
                    for v in vertices.iter().skip(1) {
                        builder.line_to(cstate.world_to_screen((v[0], v[1])));
                    }
                    builder.line_to(first);
                });
                if g.filled {
                    frame.fill(
                        &path,
                        Color {
                            a: 0.55,
                            ..base_colour
                        },
                    );
                }
                frame.stroke(
                    &path,
                    Stroke::default().with_width(stroke_px).with_color(colour),
                );
            }
        }
    }
}

/// Sample an arc (convention: [`FpGraphicKind::Arc`] — increasing angle
/// from `start_deg` to `end_deg`, wrapping through 360) as `segments + 1`
/// points around `c` at radius `r_px`.
fn arc_polyline(c: Point, r_px: f32, start_deg: f64, end_deg: f64, segments: usize) -> Vec<Point> {
    let start_rad = start_deg.to_radians() as f32;
    let sweep = FpGraphicKind::arc_sweep_deg(start_deg, end_deg).to_radians() as f32;
    (0..=segments)
        .map(|i| {
            let a = start_rad + sweep * (i as f32 / segments as f32);
            Point::new(c.x + r_px * a.cos(), c.y + r_px * a.sin())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bbox(pts: &[Point]) -> (f32, f32, f32, f32) {
        let f = |g: fn(f32, f32) -> f32, h: fn(&Point) -> f32, init: f32| {
            pts.iter().map(h).fold(init, g)
        };
        (
            f(f32::min, |p| p.x, f32::MAX),
            f(f32::max, |p| p.x, f32::MIN),
            f(f32::min, |p| p.y, f32::MAX),
            f(f32::max, |p| p.y, f32::MIN),
        )
    }

    #[test]
    fn arc_300_to_60_is_the_120_degree_arc_through_zero() {
        let pts = arc_polyline(Point::new(0.0, 0.0), 10.0, 300.0, 60.0, 64);
        let (min_x, max_x, min_y, max_y) = bbox(&pts);
        // Passes through 0 degrees, so reaches x = r; never reaches -x.
        assert!((max_x - 10.0).abs() < 1e-3, "max_x {max_x}");
        assert!(min_x > 4.9, "min_x {min_x}");
        // Endpoints sit at +-sin(60) * r.
        let half = 10.0 * 60f32.to_radians().sin();
        assert!((min_y + half).abs() < 1e-3 && (max_y - half).abs() < 1e-3);
    }

    #[test]
    fn arc_10_to_80_is_unchanged() {
        let pts = arc_polyline(Point::new(0.0, 0.0), 10.0, 10.0, 80.0, 64);
        let (min_x, max_x, min_y, max_y) = bbox(&pts);
        let r10 = 10f32.to_radians();
        let r80 = 80f32.to_radians();
        assert!((max_x - 10.0 * r10.cos()).abs() < 1e-3);
        assert!((min_x - 10.0 * r80.cos()).abs() < 1e-3);
        assert!((min_y - 10.0 * r10.sin()).abs() < 1e-3);
        assert!((max_y - 10.0 * r80.sin()).abs() < 1e-3);
    }
}
