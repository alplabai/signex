//! The points of board objects object snapping chooses from (#2353,
//! #2374). Fixed points (centres, vertices, origins) and points that depend
//! on the pointer (the nearest point of a track or pad edge, a crossing of
//! two tracks near it).

use pcb_geom::{
    Placement, PointNm, Shape, Side, layer_bit, layer_mask, pad_layers, pad_shape,
    pad_world_center, via_layers,
};
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::CopperLayer;

use super::snap::{SnapObject, SnapQuery};

/// Points per quarter turn of a round pad edge.
const EDGE_STEPS: usize = 8;

/// Every point of the asked-for kinds on the asked-for layers, for a
/// pointer at `at`.
pub(super) fn candidates(board: &PcbBoard, query: &SnapQuery, at: Point) -> Vec<Point> {
    let wants = |kind: SnapObject| query.objects.contains(&kind);
    let on = |mask: u32| mask & query.layers != 0;
    // Graphics and texts sit on no copper layer: only All Layers sees them.
    let every_layer = query.layers == u32::MAX;
    let mut out = Vec::new();
    for footprint in board
        .footprints
        .iter()
        .filter(|fp| !query.skip.contains(&fp.uuid))
    {
        if wants(SnapObject::FootprintOrigins) {
            let side = match Side::of_layer(&footprint.layer) {
                Side::Top => CopperLayer::Top,
                Side::Bottom => CopperLayer::Bottom,
            };
            if on(1 << layer_bit(side)) {
                out.push(footprint.position);
            }
        }
        for pad in footprint
            .pads
            .iter()
            .filter(|pad| on(pad_layers(footprint, pad)))
        {
            if wants(SnapObject::PadCenters)
                && let Ok(center) = pad_world_center(footprint, pad)
            {
                out.push(center.to_mm());
            }
            let Ok(shape) = pad_shape(footprint, pad) else {
                continue;
            };
            if wants(SnapObject::PadVertices) && matches!(shape, Shape::Rect { .. }) {
                out.extend(shape.outline(0).into_iter().map(PointNm::to_mm));
            }
            if wants(SnapObject::PadEdges) {
                let ring: Vec<Point> = shape
                    .ring(EDGE_STEPS)
                    .into_iter()
                    .map(PointNm::to_mm)
                    .collect();
                out.extend(nearest_on_ring(&ring, at));
            }
        }
        if wants(SnapObject::ArcCenters) && every_layer {
            let Ok(placement) = Placement::of_footprint(footprint) else {
                continue;
            };
            for center in footprint.graphics.iter().filter_map(|g| g.center) {
                if let Ok(local) = PointNm::from_mm(center, "graphic.center") {
                    out.push(placement.to_world(local).to_mm());
                }
            }
        }
    }
    if wants(SnapObject::ViaCenters) {
        out.extend(
            board
                .vias
                .iter()
                .filter(|via| !query.skip.contains(&via.uuid) && on(via_layers(via)))
                .map(|via| via.position),
        );
    }
    let tracks: Vec<(Point, Point)> = board
        .segments
        .iter()
        .filter(|s| on(layer_mask(std::iter::once(s.layer.as_str()))))
        .map(|s| (s.start, s.end))
        .collect();
    if wants(SnapObject::TrackVertices) {
        out.extend(tracks.iter().flat_map(|(a, b)| [*a, *b]));
    }
    let lines: Vec<(Point, Point)> = board
        .graphics
        .iter()
        .filter(|_| every_layer)
        .filter(|g| g.center.is_none())
        .filter_map(|g| Some((g.start?, g.end?)))
        .chain(tracks.iter().copied())
        .collect();
    if wants(SnapObject::TrackLines) {
        out.extend(lines.iter().map(|(a, b)| nearest_on_segment(*a, *b, at)));
        for g in board.graphics.iter().filter(|_| every_layer) {
            if let Some(center) = g.center
                && g.graphic_type == "circle"
                && g.radius > 0.0
            {
                out.extend(nearest_on_circle(center, g.radius, at));
            }
        }
    }
    // Free arcs (#2373): their ends are vertices, their centres arc centres.
    if wants(SnapObject::TrackVertices) && every_layer {
        out.extend(board.free.arcs.iter().flat_map(|a| [a.start, a.end]));
    }
    if wants(SnapObject::ArcCenters) && every_layer {
        out.extend(
            board
                .free
                .arcs
                .iter()
                .filter_map(|a| a.sweep().map(|s| s.center)),
        );
    }
    if wants(SnapObject::ArcCenters) && every_layer {
        out.extend(board.graphics.iter().filter_map(|g| g.center));
    }
    if wants(SnapObject::Intersections) {
        out.extend(crossings_near(&lines, at, query.radius_mm));
    }
    if wants(SnapObject::Regions) {
        for zone in &board.zones {
            if on(layer_mask(std::iter::once(zone.layer.as_str()))) {
                out.extend(zone.outline.iter().copied());
            }
        }
    }
    if wants(SnapObject::BoardShape) {
        out.extend(board.outline.iter().copied());
    }
    if wants(SnapObject::Texts) && every_layer {
        out.extend(board.texts.iter().map(|t| t.position));
    }
    out
}

/// The point of segment `a`–`b` nearest `p`.
pub(super) fn nearest_on_segment(a: Point, b: Point, p: Point) -> Point {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len_sq = dx * dx + dy * dy;
    if len_sq <= 0.0 {
        return a;
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq).clamp(0.0, 1.0);
    Point::new(a.x + t * dx, a.y + t * dy)
}

fn nearest_on_circle(center: Point, radius: f64, p: Point) -> Option<Point> {
    let (dx, dy) = (p.x - center.x, p.y - center.y);
    let len = dx.hypot(dy);
    (len > 0.0).then(|| Point::new(center.x + dx / len * radius, center.y + dy / len * radius))
}

/// The point of the closed polygon `ring` nearest `p`.
fn nearest_on_ring(ring: &[Point], p: Point) -> Option<Point> {
    let distance = |q: &Point| (q.x - p.x).powi(2) + (q.y - p.y).powi(2);
    (0..ring.len())
        .map(|i| nearest_on_segment(ring[i], ring[(i + 1) % ring.len()], p))
        .min_by(|a, b| distance(a).total_cmp(&distance(b)))
}

/// Where two of `lines` cross, for lines that pass within `radius` of `p`.
fn crossings_near(lines: &[(Point, Point)], p: Point, radius: f64) -> Vec<Point> {
    let reach = radius * radius;
    let near: Vec<&(Point, Point)> = lines
        .iter()
        .filter(|(a, b)| {
            let q = nearest_on_segment(*a, *b, p);
            (q.x - p.x).powi(2) + (q.y - p.y).powi(2) <= reach
        })
        .collect();
    let mut out = Vec::new();
    for (i, first) in near.iter().enumerate() {
        for second in &near[i + 1..] {
            out.extend(segment_crossing(**first, **second));
        }
    }
    out
}

/// Where segments `s` and `t` cross; `None` when they do not or are
/// parallel.
pub(super) fn segment_crossing(s: (Point, Point), t: (Point, Point)) -> Option<Point> {
    let (r, q) = (
        (s.1.x - s.0.x, s.1.y - s.0.y),
        (t.1.x - t.0.x, t.1.y - t.0.y),
    );
    let cross = r.0 * q.1 - r.1 * q.0;
    if cross.abs() < 1e-12 {
        return None;
    }
    let w = (t.0.x - s.0.x, t.0.y - s.0.y);
    let u = (w.0 * q.1 - w.1 * q.0) / cross;
    let v = (w.0 * r.1 - w.1 * r.0) / cross;
    ((0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v))
        .then(|| Point::new(s.0.x + u * r.0, s.0.y + u * r.1))
}
