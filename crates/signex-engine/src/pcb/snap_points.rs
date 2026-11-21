//! The points of board objects object snapping chooses from (#2353,
//! #2374). Fixed points (centres, vertices, origins) and points that depend
//! on the pointer (the nearest point of a track or pad edge, a crossing of
//! two tracks near it).

use std::f64::consts::TAU;

use pcb_geom::{
    PickKind, Placement, PointNm, Shape, Side, layer_bit, layer_mask, pad_layers, pad_shape,
    pad_world_center, via_layers,
};
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{Footprint, Pad, PadType, PcbBoard, Point};
use signex_types::pcb_arc::TrackArc;
use signex_types::pcb_free::free_pad_frame;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};

use super::snap::{SnapObject, SnapQuery};

/// Points per quarter turn of a round pad edge.
const EDGE_STEPS: usize = 8;

/// Every point of the asked-for kinds on the asked-for layers, for a
/// pointer at `at`.
pub(super) fn candidates(board: &PcbBoard, query: &SnapQuery, at: Point) -> Vec<Point> {
    let wants = |kind: SnapObject| query.objects.contains(&kind);
    let on = |mask: u32| copper_on(query, mask);
    // Graphics and texts sit on no copper layer: only All Layers sees them.
    let every_layer = query.layers == u32::MAX;
    let drawn = |token: &str| {
        parse_pcb_layer(token).is_none_or(|layer| !query.shown.hidden.contains(&layer))
    };
    // On copper by the scope; off copper only on All Layers, if shown.
    let layer_on = |token: &str| match layer_mask(std::iter::once(token)) {
        0 => every_layer && drawn(token),
        mask => on(mask),
    };
    let net_on = |net: u32| query.net.is_none_or(|own| net == 0 || net == own);
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
            if on(1 << layer_bit(side))
                && origin_on_net(query, footprint)
                && !foreign_pad_covers(board, query, footprint.position, 1 << layer_bit(side))
            {
                out.push(footprint.position);
            }
        }
        for pad in &footprint.pads {
            pad_points(query, footprint, pad, at, &mut out);
        }
        if wants(SnapObject::ArcCenters) && every_layer {
            let Ok(placement) = Placement::of_footprint(footprint) else {
                continue;
            };
            let centers = footprint
                .graphics
                .iter()
                .filter(|g| drawn(&g.layer))
                .filter_map(|g| g.center);
            for center in centers {
                if let Ok(local) = PointNm::from_mm(center, "graphic.center") {
                    out.push(placement.to_world(local).to_mm());
                }
            }
        }
    }
    // A free pad has no footprint: it stands in its own frame.
    let frame = free_pad_frame();
    for pad in board
        .free
        .pads
        .iter()
        .filter(|pad| !query.skip.contains(&pad.uuid))
    {
        pad_points(query, &frame, pad, at, &mut out);
    }
    if wants(SnapObject::ViaCenters)
        && !query.shown.through_hidden
        && query.shown.shows(PickKind::Vias)
    {
        out.extend(
            board
                .vias
                .iter()
                .filter(|via| !query.skip.contains(&via.uuid) && on(via_layers(via)))
                .filter(|via| net_on(via.net))
                .map(|via| via.position),
        );
    }
    let tracks: Vec<(Point, Point)> = board
        .segments
        .iter()
        .filter(|_| query.shown.shows(PickKind::Tracks))
        .filter(|s| !query.skip.contains(&s.uuid))
        .filter(|s| on(layer_mask(std::iter::once(s.layer.as_str()))))
        .filter(|s| net_on(s.net))
        .map(|s| (s.start, s.end))
        .collect();
    if wants(SnapObject::TrackVertices) {
        out.extend(tracks.iter().flat_map(|(a, b)| [*a, *b]));
    }
    let lines: Vec<(Point, Point)> = board
        .graphics
        .iter()
        .filter(|g| every_layer && drawn(&g.layer))
        .filter(|g| g.center.is_none())
        .filter_map(|g| Some((g.start?, g.end?)))
        .chain(tracks.iter().copied())
        .collect();
    if wants(SnapObject::TrackLines) {
        out.extend(lines.iter().map(|(a, b)| nearest_on_segment(*a, *b, at)));
        for g in board
            .graphics
            .iter()
            .filter(|g| every_layer && drawn(&g.layer))
        {
            if let Some(center) = g.center
                && g.graphic_type == "circle"
                && g.radius > 0.0
            {
                out.extend(nearest_on_circle(center, g.radius, at));
            }
        }
    }
    // Free arcs (#2373): their ends are vertices, their centres arc centres,
    // and their curve a line. One on a copper layer counts on that layer.
    let arcs: Vec<_> = board
        .free
        .arcs
        .iter()
        .filter(|_| query.shown.shows(PickKind::Arcs))
        .filter(|a| !query.skip.contains(&a.uuid))
        .filter(|a| layer_on(&a.layer) && net_on(a.net))
        .collect();
    if wants(SnapObject::TrackVertices) {
        out.extend(arcs.iter().flat_map(|a| [a.start, a.end]));
    }
    if wants(SnapObject::TrackLines) {
        out.extend(arcs.iter().map(|a| nearest_on_arc(a, at)));
    }
    if wants(SnapObject::ArcCenters) {
        out.extend(arcs.iter().filter_map(|a| a.sweep().map(|s| s.center)));
    }
    if wants(SnapObject::ArcCenters) && every_layer {
        out.extend(
            board
                .graphics
                .iter()
                .filter(|g| drawn(&g.layer))
                .filter_map(|g| g.center),
        );
    }
    if wants(SnapObject::Intersections) {
        out.extend(crossings_near(&lines, at, query.radius_mm));
    }
    if wants(SnapObject::Regions) {
        for zone in &board.zones {
            // A region off copper (a keepout, a silk fill) counts on All Layers.
            let kind = PickKind::of_zone(zone).unwrap_or(PickKind::Polygons);
            if query.shown.shows(kind) && layer_on(&zone.layer) && net_on(zone.net) {
                out.extend(zone.outline.iter().copied());
            }
        }
    }
    let edge = PcbLayer::Technical(TechnicalLayer::EdgeCuts);
    if wants(SnapObject::BoardShape) && !query.shown.hidden.contains(&edge) {
        out.extend(board.outline.iter().copied());
    }
    if wants(SnapObject::Texts) && every_layer && query.shown.shows(PickKind::Texts) {
        out.extend(
            board
                .texts
                .iter()
                .filter(|t| drawn(&t.layer))
                .map(|t| t.position),
        );
    }
    out
}

/// Whether copper on the `mask` bits is in the scope and shown.
fn copper_on(query: &SnapQuery, mask: u32) -> bool {
    mask & query.layers & query.shown.copper != 0
}

/// Whether a running route may snap to `footprint`'s origin: the part has a
/// pad on the route's net, or no pad on any net.
fn origin_on_net(query: &SnapQuery, footprint: &Footprint) -> bool {
    let Some(own) = query.net else {
        return true;
    };
    let mut nets = footprint
        .pads
        .iter()
        .filter_map(|pad| pad.net.as_ref())
        .map(|net| net.number)
        .filter(|net| *net != 0)
        .peekable();
    nets.peek().is_none() || nets.any(|net| net == own)
}

/// Whether a pad of a net other than the running route's covers `point` on
/// a copper layer of `layers`, so a route landing there would short to it.
fn foreign_pad_covers(board: &PcbBoard, query: &SnapQuery, point: Point, layers: u32) -> bool {
    let Some(own) = query.net else {
        return false;
    };
    let Ok(at) = PointNm::from_mm(point, "footprint origin") else {
        return false;
    };
    let covers = |footprint: &Footprint, pad: &Pad| {
        let net = pad.net.as_ref().map_or(0, |net| net.number);
        net != 0
            && net != own
            && pad_layers(footprint, pad) & layers != 0
            && pad_shape(footprint, pad).is_ok_and(|shape| shape.contains(at, 0))
    };
    let frame = free_pad_frame();
    board
        .footprints
        .iter()
        .any(|footprint| footprint.pads.iter().any(|pad| covers(footprint, pad)))
        || board.free.pads.iter().any(|pad| covers(&frame, pad))
}

/// The asked-for points of `pad` of `footprint`, if it is in the scope, on
/// a shown layer and on the route's net.
fn pad_points(
    query: &SnapQuery,
    footprint: &Footprint,
    pad: &Pad,
    at: Point,
    out: &mut Vec<Point>,
) {
    let net = pad.net.as_ref().map_or(0, |net| net.number);
    let through = matches!(pad.pad_type, PadType::Thru | PadType::NpThru);
    if !query.shown.shows(PickKind::Pads)
        || !copper_on(query, pad_layers(footprint, pad))
        || (query.shown.through_hidden && through)
        || query.net.is_some_and(|own| net != 0 && net != own)
    {
        return;
    }
    let wants = |kind: SnapObject| query.objects.contains(&kind);
    if wants(SnapObject::PadCenters)
        && let Ok(center) = pad_world_center(footprint, pad)
    {
        out.push(center.to_mm());
    }
    let Ok(shape) = pad_shape(footprint, pad) else {
        return;
    };
    if wants(SnapObject::PadVertices) && matches!(shape, Shape::Rect { .. } | Shape::Polygon { .. })
    {
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

/// The point of `arc` nearest `p`: on the curve when `p` faces it, else the
/// nearer end. An arc with no circle is its chord.
fn nearest_on_arc(arc: &TrackArc, p: Point) -> Point {
    let Some(sweep) = arc.sweep() else {
        return nearest_on_segment(arc.start, arc.end, p);
    };
    let angle = (p.y - sweep.center.y).atan2(p.x - sweep.center.x);
    let turn = (angle - sweep.start_angle).rem_euclid(TAU);
    let inside = if sweep.sweep >= 0.0 {
        turn <= sweep.sweep
    } else {
        turn == 0.0 || turn - TAU >= sweep.sweep
    };
    if inside {
        return sweep.at(angle);
    }
    let distance = |q: Point| (q.x - p.x).powi(2) + (q.y - p.y).powi(2);
    if distance(arc.start) <= distance(arc.end) {
        arc.start
    } else {
        arc.end
    }
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
