//! Object snapping on a board (#2353): the point of a board object the
//! pointer is pulled to while footprints are carried. Pure: it reads the
//! board and changes nothing.

use std::collections::BTreeSet;

use pcb_geom::{Side, layer_bit, layer_mask, pad_layers, pad_world_center, via_layers};
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

/// One kind of point the pointer can snap to, in the order the Snap Options
/// list shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SnapObject {
    TrackVertices,
    TrackLines,
    ArcCenters,
    Intersections,
    PadCenters,
    PadVertices,
    PadEdges,
    ViaCenters,
    Regions,
    BoardShape,
    FootprintOrigins,
    Body3dPoints,
    Texts,
}

impl SnapObject {
    pub const ALL: [SnapObject; 13] = [
        SnapObject::TrackVertices,
        SnapObject::TrackLines,
        SnapObject::ArcCenters,
        SnapObject::Intersections,
        SnapObject::PadCenters,
        SnapObject::PadVertices,
        SnapObject::PadEdges,
        SnapObject::ViaCenters,
        SnapObject::Regions,
        SnapObject::BoardShape,
        SnapObject::FootprintOrigins,
        SnapObject::Body3dPoints,
        SnapObject::Texts,
    ];

    /// The kinds snapped to on a new session.
    pub const DEFAULT_ON: [SnapObject; 2] = [SnapObject::PadCenters, SnapObject::ViaCenters];

    /// Whether [`object_snap_point`] finds points of this kind. Lines,
    /// edges, intersections, arc centres and 3D bodies need geometry the
    /// board does not offer as points yet.
    pub fn is_supported(self) -> bool {
        matches!(
            self,
            SnapObject::TrackVertices
                | SnapObject::PadCenters
                | SnapObject::ViaCenters
                | SnapObject::Regions
                | SnapObject::BoardShape
                | SnapObject::FootprintOrigins
                | SnapObject::Texts
        )
    }
}

/// What object snapping looks for.
#[derive(Debug, Clone, Copy)]
pub struct SnapQuery<'a> {
    pub objects: &'a BTreeSet<SnapObject>,
    /// The copper layers whose objects count, as `pcb_geom::layer_bit`
    /// bits; `u32::MAX` for every layer. Texts count only on every layer,
    /// the board shape on any.
    pub layers: u32,
    /// Footprints whose pads and origins are left out: the carried ones.
    pub skip: &'a BTreeSet<Uuid>,
    /// How far from the pointer a point may be, mm.
    pub radius_mm: f64,
}

/// The snap point nearest `at` within the query's radius, if any.
pub fn object_snap_point(board: &PcbBoard, at: Point, query: &SnapQuery) -> Option<Point> {
    if !query.radius_mm.is_finite() || query.radius_mm <= 0.0 {
        return None;
    }
    let reach = query.radius_mm * query.radius_mm;
    let distance = |p: &Point| (p.x - at.x).powi(2) + (p.y - at.y).powi(2);
    candidates(board, query)
        .into_iter()
        .filter(|p| p.x.is_finite() && p.y.is_finite())
        .map(|p| (distance(&p), p))
        .filter(|(d, _)| *d <= reach)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, p)| p)
}

/// Every point of the asked-for kinds on the asked-for layers.
fn candidates(board: &PcbBoard, query: &SnapQuery) -> Vec<Point> {
    let wants = |kind: SnapObject| query.objects.contains(&kind);
    let on = |mask: u32| mask & query.layers != 0;
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
        if wants(SnapObject::PadCenters) {
            out.extend(
                footprint
                    .pads
                    .iter()
                    .filter(|pad| on(pad_layers(footprint, pad)))
                    .filter_map(|pad| pad_world_center(footprint, pad).ok())
                    .map(|p| p.to_mm()),
            );
        }
    }
    if wants(SnapObject::ViaCenters) {
        out.extend(
            board
                .vias
                .iter()
                .filter(|via| on(via_layers(via)))
                .map(|via| via.position),
        );
    }
    if wants(SnapObject::TrackVertices) {
        for segment in &board.segments {
            if on(layer_mask(std::iter::once(segment.layer.as_str()))) {
                out.extend([segment.start, segment.end]);
            }
        }
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
    if wants(SnapObject::Texts) && query.layers == u32::MAX {
        out.extend(board.texts.iter().map(|t| t.position));
    }
    out
}

#[cfg(test)]
#[path = "snap_tests.rs"]
mod tests;
