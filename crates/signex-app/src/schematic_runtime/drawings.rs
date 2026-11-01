//! Lowers sheet drawings to the scene's ordered bucket (#1579).
//!
//! Every drawing becomes one or more [`OrderedItem`]s pushed in file order, so
//! the order on the sheet is the paint order on both the CPU replay and the
//! GPU. A circle is a fill disc (only when filled) then a ring: separate fill
//! and stroke colours, and the ring's SDF reads the stroke at the centre even
//! when it is wider than the radius.

use super::snapshot::arc_geometry;
use super::*;
use signex_gfx::primitive::arc::Arc;
use signex_gfx::primitive::circle::Circle;
use signex_gfx::primitive::line::LineSegment;
use signex_gfx::primitive::ordered::OrderedItem;
use signex_gfx::primitive::polygon::GpuPolygon;

const CLEAR: [f32; 4] = [0.0, 0.0, 0.0, 0.0];

/// How a drawing is stroked and filled, resolved from the document.
struct Look {
    stroke: [f32; 4],
    fill: [f32; 4],
    width_mm: f32,
}

fn solid_line(p0: Point, p1: Point, look: &Look) -> OrderedItem {
    OrderedItem::Line(LineSegment {
        p0: [p0.x as f32, p0.y as f32],
        p1: [p1.x as f32, p1.y as f32],
        width: look.width_mm,
        color: look.stroke,
        style: 0,
        _pad: 0,
    })
}

fn polygon(vertices: Vec<[f32; 2]>, look: &Look) -> OrderedItem {
    OrderedItem::Polygon(GpuPolygon {
        holes: Vec::new(),
        vertices,
        fill_color: look.fill,
        stroke_color: Some(look.stroke),
        stroke_width: look.width_mm,
    })
}

/// Append `drawing`'s items to `out`. `scale` is the screen scale, for the
/// radius floors.
pub(super) fn push_drawing(
    drawing: &SchDrawing,
    colors: &CanvasColors,
    focus_set: Option<&HashSet<uuid::Uuid>>,
    scale: f32,
    out: &mut Vec<OrderedItem>,
) {
    let (uuid, width, fill, stroke_color) = match drawing {
        SchDrawing::Line {
            uuid,
            width,
            stroke_color,
            ..
        }
        | SchDrawing::Arc {
            uuid,
            width,
            stroke_color,
            ..
        } => (*uuid, *width, FillType::None, stroke_color),
        SchDrawing::Rect {
            uuid,
            width,
            fill,
            stroke_color,
            ..
        }
        | SchDrawing::Circle {
            uuid,
            width,
            fill,
            stroke_color,
            ..
        }
        | SchDrawing::Polyline {
            uuid,
            width,
            fill,
            stroke_color,
            ..
        } => (*uuid, *width, *fill, stroke_color),
    };
    let base_color = focus_color(to_iced(&colors.body), focus_set, uuid);
    let look = Look {
        stroke: to_rgba(resolve_stroke_color(stroke_color, base_color)),
        fill: fill_color_for(fill, stroke_color, colors)
            .map(to_rgba)
            .unwrap_or(CLEAR),
        width_mm: signex_types::schematic::effective_stroke_mm(width) as f32,
    };

    match drawing {
        SchDrawing::Line { start, end, .. } => out.push(solid_line(*start, *end, &look)),
        SchDrawing::Rect { start, end, .. } => out.push(polygon(
            vec![
                [start.x as f32, start.y as f32],
                [end.x as f32, start.y as f32],
                [end.x as f32, end.y as f32],
                [start.x as f32, end.y as f32],
            ],
            &look,
        )),
        SchDrawing::Circle { center, radius, .. } => {
            push_circle(*center, *radius, &look, fill, scale, out);
        }
        SchDrawing::Arc {
            start, mid, end, ..
        } => push_arc_items([*start, *mid, *end], &look, scale, out),
        SchDrawing::Polyline { points, .. } => push_polyline(points, &look, fill, out),
    }
}

fn push_circle(
    center: Point,
    radius: f64,
    look: &Look,
    fill: FillType,
    scale: f32,
    out: &mut Vec<OrderedItem>,
) {
    let radius = radius.max(screen_px_to_world_mm(
        signex_types::schematic::SCHEMATIC_RENDER_DRAWING_MIN_CIRCLE_RADIUS_PX,
        scale,
    )) as f32;
    let center = [center.x as f32, center.y as f32];
    if !matches!(fill, FillType::None) {
        out.push(OrderedItem::Circle(Circle {
            center,
            radius,
            stroke_width: 0.0,
            color: look.fill,
        }));
    }
    out.push(OrderedItem::Circle(Circle {
        center,
        radius,
        stroke_width: look.width_mm,
        color: look.stroke,
    }));
}

/// An arc, or the two straight segments a collinear triple degenerates to.
fn push_arc_items(points: [Point; 3], look: &Look, scale: f32, out: &mut Vec<OrderedItem>) {
    let [start, mid, end] = points;
    let Some(geometry) = arc_geometry(points, scale) else {
        out.push(solid_line(start, mid, look));
        out.push(solid_line(mid, end, look));
        return;
    };
    out.push(OrderedItem::Arc(Arc {
        center: geometry.center,
        radius: geometry.radius_mm,
        start_angle: geometry.start_angle_rad,
        end_angle: geometry.end_angle_rad,
        width: look.width_mm,
        color: look.stroke,
        _pad: [0.0; 3],
    }));
}

/// An unfilled polyline is its segments; a filled one is a polygon.
fn push_polyline(points: &[Point], look: &Look, fill: FillType, out: &mut Vec<OrderedItem>) {
    if points.len() < 2 {
        return;
    }
    if matches!(fill, FillType::None) {
        out.extend(
            points
                .windows(2)
                .map(|pair| solid_line(pair[0], pair[1], look)),
        );
    } else {
        out.push(polygon(
            points.iter().map(|p| [p.x as f32, p.y as f32]).collect(),
            look,
        ));
    }
}
