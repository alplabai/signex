//! The points of board objects object snapping chooses from (#2353,
//! #2374, #2879). Fixed points (centres, vertices, origins) and points that
//! depend on the pointer (the nearest point of a track, drawing, pad,
//! region or board edge, a crossing of two curves near it).

use pcb_geom::{
    PickKind, Placement, PointNm, Shape, Side, board_graphic_placement,
    footprint_graphic_placement, layer_bit, layer_mask, pad_layers, pad_shape, pad_world_center,
    via_layers,
};
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{Footprint, Pad, PadType, PcbBoard, Point, Zone};
use signex_types::pcb_free::free_pad_frame;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};

use super::snap::{SnapObject, SnapQuery};
use super::snap_curves::{Curve, crossings, graphic_curves, ring_edges};

/// Points per quarter turn of a round pad edge.
const EDGE_STEPS: usize = 8;

/// Which points [`candidates`] gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Points {
    /// Fixed points and the nearest points of lines and edges.
    All,
    /// Fixed points only: axis snapping, where the nearest point of an edge
    /// would line the pointer up with itself.
    Fixed,
}

/// Every point of the asked-for kinds on the asked-for layers, for a
/// pointer at `at`.
pub(super) fn candidates(
    board: &PcbBoard,
    query: &SnapQuery,
    at: Point,
    points: Points,
) -> Vec<Point> {
    let nearest = points == Points::All;
    let wants = |kind: SnapObject| query.objects.contains(&kind);
    let on = |mask: u32| copper_on(query, mask);
    let drawn = |token: &str| {
        parse_pcb_layer(token).is_none_or(|layer| !query.shown.hidden.contains(&layer))
    };
    // Copper by its bits; any other layer when in scope and shown.
    let layer_on = |token: &str| match layer_mask(std::iter::once(token)) {
        0 => query.layers.has_other(parse_pcb_layer(token)) && drawn(token),
        mask => on(mask),
    };
    let net_on = |net: u32| query.net.is_none_or(|own| net == 0 || net == own);
    let mut out = Vec::new();
    // Tracks, arcs and drawings: Track/Arc Lines, and what crosses.
    let mut lines: Vec<Curve> = Vec::new();
    for footprint in board
        .footprints
        .iter()
        .filter(|fp| !query.skip.contains(&fp.uuid))
    {
        // A running route with a net lands on pads, not on part origins.
        if wants(SnapObject::FootprintOrigins) && query.net.is_none() {
            let side = match Side::of_layer(&footprint.layer) {
                Side::Top => CopperLayer::Top,
                Side::Bottom => CopperLayer::Bottom,
            };
            if on(1 << layer_bit(side)) {
                out.push(footprint.position);
            }
        }
        for pad in &footprint.pads {
            pad_points(query, footprint, pad, at, nearest, &mut out);
        }
        let Ok(placement) = Placement::of_footprint(footprint) else {
            continue;
        };
        for graphic in footprint.graphics.iter().filter(|g| layer_on(&g.layer)) {
            if wants(SnapObject::ArcCenters)
                && let Some(center) = graphic.center
                && let Ok(local) = PointNm::from_mm(center, "graphic.center")
            {
                out.push(placement.to_world(local).to_mm());
            }
            if let Ok(Some(placed)) = footprint_graphic_placement(footprint, graphic) {
                lines.extend(graphic_curves(&placed.shape));
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
        pad_points(query, &frame, pad, at, nearest, &mut out);
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
    lines.extend(tracks.iter().map(|(a, b)| Curve::Segment(*a, *b)));
    let graphics: Vec<_> = board
        .graphics
        .iter()
        .filter(|g| layer_on(&g.layer))
        .collect();
    for graphic in &graphics {
        if let Ok(Some(placed)) = board_graphic_placement(graphic) {
            lines.extend(graphic_curves(&placed.shape));
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
    lines.extend(arcs.iter().map(|a| match a.sweep() {
        Some(sweep) => Curve::Arc(sweep),
        // An arc with no circle is its chord.
        None => Curve::Segment(a.start, a.end),
    }));
    if wants(SnapObject::TrackLines) && nearest {
        out.extend(lines.iter().map(|line| line.nearest(at)));
    }
    if wants(SnapObject::ArcCenters) {
        out.extend(arcs.iter().filter_map(|a| a.sweep().map(|s| s.center)));
        out.extend(graphics.iter().filter_map(|g| g.center));
    }
    let regions: Vec<&Zone> = board
        .zones
        .iter()
        .filter(|zone| {
            // A region off copper (a keepout, a silk fill) counts on its layer.
            let kind = PickKind::of_zone(zone).unwrap_or(PickKind::Polygons);
            query.shown.shows(kind) && layer_on(&zone.layer) && net_on(zone.net)
        })
        .collect();
    let region_rings = || {
        regions
            .iter()
            .copied()
            .flat_map(|zone| std::iter::once(&zone.outline).chain(&zone.holes))
    };
    if wants(SnapObject::Regions) {
        for ring in region_rings() {
            ring_points(ring, at, nearest, &mut out);
        }
    }
    let edge = PcbLayer::Technical(TechnicalLayer::EdgeCuts);
    let board_shown = !query.shown.hidden.contains(&edge);
    if wants(SnapObject::BoardShape) && board_shown {
        ring_points(&board.outline, at, nearest, &mut out);
    }
    if wants(SnapObject::Intersections) {
        let mut curves = lines;
        for ring in region_rings() {
            curves.extend(ring_edges(ring));
        }
        if board_shown {
            curves.extend(ring_edges(&board.outline));
        }
        out.extend(crossings_near(&curves, at, query.radius_mm));
    }
    if wants(SnapObject::Texts) && query.shown.shows(PickKind::Texts) {
        out.extend(
            board
                .texts
                .iter()
                .filter(|t| layer_on(&t.layer))
                .map(|t| t.position),
        );
    }
    out
}

/// The vertices of the closed `ring` and, with `nearest`, the point of its
/// edges nearest `at` (#2879).
fn ring_points(ring: &[Point], at: Point, nearest: bool, out: &mut Vec<Point>) {
    out.extend(ring.iter().copied());
    if nearest {
        out.extend(nearest_on_ring(ring, at));
    }
}

/// Whether copper on the `mask` bits is in the scope and shown.
fn copper_on(query: &SnapQuery, mask: u32) -> bool {
    mask & query.layers.copper() & query.shown.copper != 0
}

/// The asked-for points of `pad` of `footprint`, if it is in the scope, on
/// a shown layer and on the route's net; its edge's nearest point only with
/// `nearest`.
fn pad_points(
    query: &SnapQuery,
    footprint: &Footprint,
    pad: &Pad,
    at: Point,
    nearest: bool,
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
    if wants(SnapObject::PadEdges) && nearest {
        let ring: Vec<Point> = shape
            .ring(EDGE_STEPS)
            .into_iter()
            .map(PointNm::to_mm)
            .collect();
        out.extend(nearest_on_ring(&ring, at));
    }
}

/// The point of the closed polygon `ring` nearest `p`.
fn nearest_on_ring(ring: &[Point], p: Point) -> Option<Point> {
    let distance = |q: &Point| (q.x - p.x).powi(2) + (q.y - p.y).powi(2);
    ring_edges(ring)
        .map(|edge| edge.nearest(p))
        .min_by(|a, b| distance(a).total_cmp(&distance(b)))
}

/// Where two of `curves` cross, for curves that pass within `radius` of `p`.
fn crossings_near(curves: &[Curve], p: Point, radius: f64) -> Vec<Point> {
    let reach = radius * radius;
    let near: Vec<&Curve> = curves
        .iter()
        .filter(|curve| {
            let q = curve.nearest(p);
            (q.x - p.x).powi(2) + (q.y - p.y).powi(2) <= reach
        })
        .collect();
    let mut out = Vec::new();
    for (i, first) in near.iter().enumerate() {
        for second in &near[i + 1..] {
            out.extend(crossings(first, second));
        }
    }
    out
}
