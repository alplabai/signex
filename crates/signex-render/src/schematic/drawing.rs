//! Drawing primitives — sheet-level decorations (lines, rectangles,
//! circles, arcs, polylines, beziers).
//!
//! Each variant of [`signex_types::schematic::SchDrawing`] becomes a
//! single iced canvas Path with a stroke (always) plus an optional
//! fill driven by [`signex_types::schematic::FillType`]. A per-shape
//! `stroke_color` overrides the theme's `body` colour when present.
//! See `docs/RENDERING_RULES.md` (general — line/rect/arc/circle/
//! polygon section).

use iced::widget::canvas::{Frame, Path, Stroke};
use signex_types::schematic::{
    Aabb, FillType, Point, SchDrawing, SelectedItem, SelectedKind, StrokeColor,
};

use super::RenderContext;
use super::util::{aabbs_overlap, iced_color, point_finite};

/// Default stroke width when `width == 0.0`. World mm.
pub const DEFAULT_STROKE_MM: f64 = 0.15;

const SELECTION_WEIGHT_FACTOR: f64 = 1.5;

/// Render a single drawing primitive into the content layer's frame.
///
/// All variants are dispatched through one entry; per-variant helpers
/// are private to keep the call surface small.
pub fn draw_drawing(frame: &mut Frame, drawing: &SchDrawing, ctx: &RenderContext<'_>) {
    let bbox = drawing_aabb(drawing);
    if !aabbs_overlap(&bbox, &ctx.visible_world_bounds()) {
        return;
    }

    let uuid = drawing_uuid(drawing);
    let selected = ctx.is_selected(&SelectedItem::new(uuid, SelectedKind::Drawing));

    match drawing {
        SchDrawing::Line {
            start,
            end,
            width,
            stroke_color,
            ..
        } => draw_line(frame, *start, *end, *width, stroke_color, selected, ctx),
        SchDrawing::Rect {
            start,
            end,
            width,
            fill,
            stroke_color,
            ..
        } => draw_rect(
            frame,
            *start,
            *end,
            *width,
            *fill,
            stroke_color,
            selected,
            ctx,
        ),
        SchDrawing::Circle {
            center,
            radius,
            width,
            fill,
            stroke_color,
            ..
        } => draw_circle(
            frame,
            *center,
            *radius,
            *width,
            *fill,
            stroke_color,
            selected,
            ctx,
        ),
        SchDrawing::Arc {
            start,
            mid,
            end,
            width,
            stroke_color,
            ..
        } => draw_arc(
            frame,
            *start,
            *mid,
            *end,
            *width,
            stroke_color,
            selected,
            ctx,
        ),
        SchDrawing::Polyline {
            points,
            width,
            fill,
            stroke_color,
            ..
        } => draw_polyline(frame, points, *width, *fill, stroke_color, selected, ctx),
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_line(
    frame: &mut Frame,
    start: Point,
    end: Point,
    width: f64,
    stroke_color: &Option<StrokeColor>,
    selected: bool,
    ctx: &RenderContext<'_>,
) {
    let s = ctx.viewport.world_to_screen(start);
    let e = ctx.viewport.world_to_screen(end);
    if !point_finite(s) || !point_finite(e) {
        return;
    }
    frame.stroke(
        &Path::line(s, e),
        build_stroke(width, stroke_color, selected, ctx),
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_rect(
    frame: &mut Frame,
    start: Point,
    end: Point,
    width: f64,
    fill: FillType,
    stroke_color: &Option<StrokeColor>,
    selected: bool,
    ctx: &RenderContext<'_>,
) {
    let a = ctx.viewport.world_to_screen(start);
    let b = ctx.viewport.world_to_screen(end);
    if !point_finite(a) || !point_finite(b) {
        return;
    }
    let path = Path::new(|builder| {
        builder.move_to(a);
        builder.line_to(iced::Point::new(b.x, a.y));
        builder.line_to(b);
        builder.line_to(iced::Point::new(a.x, b.y));
        builder.close();
    });
    if let Some(color) = fill_colour(fill, stroke_color, ctx) {
        frame.fill(&path, color);
    }
    frame.stroke(&path, build_stroke(width, stroke_color, selected, ctx));
}

#[allow(clippy::too_many_arguments)]
fn draw_circle(
    frame: &mut Frame,
    center: Point,
    radius: f64,
    width: f64,
    fill: FillType,
    stroke_color: &Option<StrokeColor>,
    selected: bool,
    ctx: &RenderContext<'_>,
) {
    let c = ctx.viewport.world_to_screen(center);
    if !point_finite(c) {
        return;
    }
    let r_px = (radius * ctx.viewport.zoom_px_per_mm()).max(0.5) as f32;
    let path = Path::circle(c, r_px);
    if let Some(color) = fill_colour(fill, stroke_color, ctx) {
        frame.fill(&path, color);
    }
    frame.stroke(&path, build_stroke(width, stroke_color, selected, ctx));
}

#[allow(clippy::too_many_arguments)]
fn draw_arc(
    frame: &mut Frame,
    start: Point,
    mid: Point,
    end: Point,
    width: f64,
    stroke_color: &Option<StrokeColor>,
    selected: bool,
    ctx: &RenderContext<'_>,
) {
    let Some((cx_w, cy_w, r_w)) = circumcircle((start.x, start.y), (mid.x, mid.y), (end.x, end.y))
    else {
        // Degenerate (collinear) input — fall back to a polyline through
        // the three points so nothing is silently dropped.
        let path = Path::new(|builder| {
            builder.move_to(ctx.viewport.world_to_screen(start));
            builder.line_to(ctx.viewport.world_to_screen(mid));
            builder.line_to(ctx.viewport.world_to_screen(end));
        });
        frame.stroke(&path, build_stroke(width, stroke_color, selected, ctx));
        return;
    };

    let centre_screen = ctx.viewport.world_to_screen(Point::new(cx_w, cy_w));
    let r_px = (r_w * ctx.viewport.zoom_px_per_mm()).max(0.5) as f32;
    let a0 = (start.y - cy_w).atan2(start.x - cx_w);
    let am = (mid.y - cy_w).atan2(mid.x - cx_w);
    let a1 = (end.y - cy_w).atan2(end.x - cx_w);

    // Direction (CCW / CW) chosen so the arc sweeps through `mid`.
    let (start_angle, end_angle) = if arc_sweeps_through_mid(a0, am, a1) {
        (a0, a1)
    } else {
        (a1, a0)
    };

    let path = Path::new(|builder| {
        builder.arc(iced::widget::canvas::path::Arc {
            center: centre_screen,
            radius: r_px,
            start_angle: iced::Radians(start_angle as f32),
            end_angle: iced::Radians(end_angle as f32),
        });
    });
    frame.stroke(&path, build_stroke(width, stroke_color, selected, ctx));
}

#[allow(clippy::too_many_arguments)]
fn draw_polyline(
    frame: &mut Frame,
    points: &[Point],
    width: f64,
    fill: FillType,
    stroke_color: &Option<StrokeColor>,
    selected: bool,
    ctx: &RenderContext<'_>,
) {
    if points.len() < 2 {
        return;
    }
    let path = Path::new(|builder| {
        let mut iter = points.iter();
        if let Some(first) = iter.next() {
            builder.move_to(ctx.viewport.world_to_screen(*first));
        }
        for p in iter {
            builder.line_to(ctx.viewport.world_to_screen(*p));
        }
        // Close only when fill is requested AND first/last point differ.
        if fill_colour(fill, stroke_color, ctx).is_some() {
            builder.close();
        }
    });
    if let Some(color) = fill_colour(fill, stroke_color, ctx) {
        frame.fill(&path, color);
    }
    frame.stroke(&path, build_stroke(width, stroke_color, selected, ctx));
}

/// World-space AABB enclosing a drawing primitive, used by frustum
/// culling and (in Wave 4) hit testing.
pub(crate) fn drawing_aabb(drawing: &SchDrawing) -> Aabb {
    match drawing {
        SchDrawing::Line { start, end, .. } => Aabb::new(start.x, start.y, end.x, end.y),
        SchDrawing::Rect { start, end, .. } => Aabb::new(start.x, start.y, end.x, end.y),
        SchDrawing::Circle { center, radius, .. } => {
            let r = radius.abs();
            Aabb::new(center.x - r, center.y - r, center.x + r, center.y + r)
        }
        SchDrawing::Arc {
            start, mid, end, ..
        } => {
            let xs = [start.x, mid.x, end.x];
            let ys = [start.y, mid.y, end.y];
            Aabb::new(
                xs.iter().cloned().fold(f64::INFINITY, f64::min),
                ys.iter().cloned().fold(f64::INFINITY, f64::min),
                xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            )
        }
        SchDrawing::Polyline { points, .. } => {
            if points.is_empty() {
                Aabb::new(0.0, 0.0, 0.0, 0.0)
            } else {
                let mut min_x = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_y = f64::NEG_INFINITY;
                for p in points {
                    min_x = min_x.min(p.x);
                    max_x = max_x.max(p.x);
                    min_y = min_y.min(p.y);
                    max_y = max_y.max(p.y);
                }
                Aabb::new(min_x, min_y, max_x, max_y)
            }
        }
    }
}

#[inline]
fn drawing_uuid(drawing: &SchDrawing) -> uuid::Uuid {
    match drawing {
        SchDrawing::Line { uuid, .. }
        | SchDrawing::Rect { uuid, .. }
        | SchDrawing::Circle { uuid, .. }
        | SchDrawing::Arc { uuid, .. }
        | SchDrawing::Polyline { uuid, .. } => *uuid,
    }
}

fn build_stroke<'a>(
    width: f64,
    stroke_color: &Option<StrokeColor>,
    selected: bool,
    ctx: &RenderContext<'_>,
) -> Stroke<'a> {
    let mm = if width > 0.0 {
        width
    } else {
        DEFAULT_STROKE_MM
    };
    let scaled = mm
        * if selected {
            SELECTION_WEIGHT_FACTOR
        } else {
            1.0
        };
    let px = (scaled * ctx.viewport.zoom_px_per_mm()).max(1.0) as f32;
    let colour = if selected {
        iced_color(&ctx.theme().selection)
    } else if let Some(sc) = stroke_color {
        iced_color(&signex_types::theme::Color::new(sc.r, sc.g, sc.b, sc.a))
    } else {
        iced_color(&ctx.theme().body)
    };
    Stroke::default().with_width(px).with_color(colour)
}

fn fill_colour(
    fill: FillType,
    stroke_color: &Option<StrokeColor>,
    ctx: &RenderContext<'_>,
) -> Option<iced::Color> {
    match fill {
        FillType::None => None,
        FillType::Outline => stroke_color
            .map(|sc| iced_color(&signex_types::theme::Color::new(sc.r, sc.g, sc.b, sc.a)))
            .or_else(|| Some(iced_color(&ctx.theme().body))),
        FillType::Background => Some(iced_color(&ctx.theme().body_fill)),
    }
}

/// Circle through three non-collinear points. Returns (cx, cy, r).
/// Mirrors the helper in `signex-engine::selection`; duplicated here
/// to keep the renderer crate dependency-free of the engine.
pub(crate) fn circumcircle(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> Option<(f64, f64, f64)> {
    let (ax, ay) = a;
    let (bx, by) = b;
    let (cx, cy) = c;
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-12 {
        return None;
    }
    let ux = ((ax * ax + ay * ay) * (by - cy)
        + (bx * bx + by * by) * (cy - ay)
        + (cx * cx + cy * cy) * (ay - by))
        / d;
    let uy = ((ax * ax + ay * ay) * (cx - bx)
        + (bx * bx + by * by) * (ax - cx)
        + (cx * cx + cy * cy) * (bx - ax))
        / d;
    let r = ((ax - ux).powi(2) + (ay - uy).powi(2)).sqrt();
    Some((ux, uy, r))
}

/// Does a CCW arc from angle `a0` to angle `a1` pass through angle
/// `am`? Used to choose CW vs CCW direction so the rendered arc
/// includes the user-specified mid-point.
fn arc_sweeps_through_mid(a0: f64, am: f64, a1: f64) -> bool {
    let two_pi = 2.0 * std::f64::consts::PI;
    let norm = |a: f64| (a - a0).rem_euclid(two_pi);
    norm(am) < norm(a1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn rect(start: Point, end: Point) -> SchDrawing {
        SchDrawing::Rect {
            uuid: Uuid::new_v4(),
            start,
            end,
            width: 0.0,
            fill: FillType::None,
            stroke_color: None,
        }
    }

    #[test]
    fn drawing_aabb_rect_normalises_endpoints() {
        let r = rect(Point::new(5.0, 5.0), Point::new(-1.0, 1.0));
        let bbox = drawing_aabb(&r);
        assert!(bbox.min_x <= bbox.max_x);
        assert!(bbox.min_y <= bbox.max_y);
        assert!(bbox.contains(0.0, 3.0));
    }

    #[test]
    fn circumcircle_three_collinear_points_returns_none() {
        assert!(circumcircle((0.0, 0.0), (1.0, 0.0), (2.0, 0.0)).is_none());
    }

    #[test]
    fn circumcircle_unit_triangle_is_finite_and_centred() {
        let (cx, cy, r) = circumcircle((0.0, 0.0), (1.0, 0.0), (0.0, 1.0)).unwrap();
        assert!(cx.is_finite() && cy.is_finite() && r.is_finite());
        // Centre of right-isoceles triangle is at (0.5, 0.5).
        assert!((cx - 0.5).abs() < 1e-9);
        assert!((cy - 0.5).abs() < 1e-9);
    }

    #[test]
    fn drawing_aabb_polyline_collinear_points_still_has_extent() {
        // Edge case from the Wave 1 stub note: collinear polyline.
        let pl = SchDrawing::Polyline {
            uuid: Uuid::new_v4(),
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(2.0, 0.0),
            ],
            width: 0.0,
            fill: FillType::None,
            stroke_color: None,
        };
        let bbox = drawing_aabb(&pl);
        assert!(bbox.width() >= 2.0);
        // Height collapses to 0 — the renderer still strokes it as a
        // line; bbox accepts that.
        assert!(bbox.height() == 0.0);
    }
}
