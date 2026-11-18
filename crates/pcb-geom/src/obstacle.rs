//! What new copper must keep clear of (#2408): every pad (footprint or
//! free), track, copper arc and via of the board, at its size on each
//! layer, and every copper area that is not a pour (Fill, Solid Region,
//! teardrop), each with its net and copper layers, no net included, and
//! the keepouts that forbid it. Via stitching, teardrops and glossing read
//! this one list, so they agree on what "clear" means: the clearance a
//! caller's rule gives between two nets, measured between pads, tracks
//! and vias as the design rule check measures it (`Shape::separation`).
//! Unlike the check, two items on no net count as different nets here, so
//! new copper errs on the safe side. Pours are not obstacles: a pour's
//! fill is recomputed and clears new copper of other nets. Rules:
//! `docs/PCB_TOOLS.md`, "Board Tools".

use signex_types::pcb::PcbBoard;
use signex_types::pcb_free::free_pad_frame;
use signex_types::pcb_sketch::PcbKeepout;

use crate::copper::{layer_bit, layer_mask, pad_pieces, via_pieces};
use crate::free::arc_shapes;
use crate::index::{ItemRef, track_shape};
use crate::pick_filter::PickKind;
use crate::rect::RectNm;
use crate::region::Polygon;
use crate::shape::Shape;
use crate::shape_overlap::{point_in_polygon, polygons_meet};
use crate::units::{GeomError, PointNm};
use crate::zone_fill::zone_area;

/// The copper an obstacle holds.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Copper {
    /// A pad, track or via.
    Shape(Shape),
    /// A copper zone: its outline less its holes.
    Area(Polygon),
}

/// One pad, track, via or copper area, with the net it is on (0: none).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Obstacle {
    pub item: ItemRef,
    pub net: u32,
    /// Copper layers it reaches, as a [`layer_bit`] mask.
    pub layers: u32,
    pub copper: Copper,
}

impl Obstacle {
    /// A pad (footprint or free) or a via: copper a new via keeps off even
    /// on its own net.
    pub fn is_landing(&self) -> bool {
        matches!(
            self.item,
            ItemRef::Pad { .. } | ItemRef::FreePad(_) | ItemRef::Via(_)
        )
    }

    fn bounds(&self) -> Option<RectNm> {
        match &self.copper {
            Copper::Shape(shape) => Some(shape.aabb()),
            Copper::Area(area) => area.bounds(),
        }
    }

    /// Whether `shape` comes within `gap` of this copper: under `gap` from a
    /// pad, track or via, as the design rule check measures it; within it
    /// of an area, touching included.
    pub fn near(&self, shape: &Shape, gap: i64) -> bool {
        let gap = gap.max(0);
        let reach = shape.aabb().inflate(gap);
        if !self.bounds().is_some_and(|b| b.intersects(&reach)) {
            return false;
        }
        match &self.copper {
            Copper::Shape(own) => shape.overlaps(own) || shape.separation(own).is_under(gap),
            Copper::Area(area) => area_meets(area, &shape.clone().inflated(gap)),
        }
    }

    /// Whether the polygon `ring` comes within `gap` of this copper,
    /// touching included. An area's holes are not looked at: safe side.
    fn near_ring(&self, ring: &[PointNm], gap: i64) -> bool {
        let gap = gap.max(0);
        match &self.copper {
            Copper::Shape(own) => own.clone().inflated(gap).overlaps_polygon(ring),
            Copper::Area(area) => {
                area.outer.len() >= 3
                    && (polygons_meet(ring, &area.outer)
                        || (0..ring.len()).any(|i| {
                            Shape::Capsule {
                                a: ring[i],
                                b: ring[(i + 1) % ring.len()],
                                radius: gap,
                            }
                            .overlaps_polygon(&area.outer)
                        }))
            }
        }
    }
}

/// Whether `shape` shares a point with `area`: it meets the outer ring and
/// does not lie wholly inside one of the holes.
pub(crate) fn area_meets(area: &Polygon, shape: &Shape) -> bool {
    if area.outer.len() < 3 || !shape.overlaps_polygon(&area.outer) {
        return false;
    }
    !area
        .holes
        .iter()
        .any(|hole| hole.len() >= 3 && inside_ring(shape, hole))
}

/// Whether the convex `shape` lies wholly inside `ring`, off its edge: no
/// edge of the ring meets it and its middle is inside.
fn inside_ring(shape: &Shape, ring: &[PointNm]) -> bool {
    let b = shape.aabb();
    let middle = PointNm::new(
        b.min.x + (b.max.x - b.min.x) / 2,
        b.min.y + (b.max.y - b.min.y) / 2,
    );
    point_in_polygon(ring, middle)
        && !(0..ring.len()).any(|i| {
            shape.overlaps(&Shape::Capsule {
                a: ring[i],
                b: ring[(i + 1) % ring.len()],
                radius: 0,
            })
        })
}

/// Every pad (footprint and free), track, copper arc and via whose geometry
/// converts, and every copper zone that is neither a pour nor a rule area.
/// A track, arc or zone on no copper layer is not copper and is left out.
pub(crate) fn copper_obstacles(board: &PcbBoard) -> Vec<Obstacle> {
    let mut out = Vec::new();
    for footprint in &board.footprints {
        for pad in &footprint.pads {
            let item = ItemRef::Pad {
                footprint: footprint.uuid,
                pad: pad.uuid,
            };
            let net = pad.net.as_ref().map_or(0, |n| n.number);
            push_pieces(&mut out, item, net, pad_pieces(footprint, pad));
        }
    }
    // Free pads (#2373), in the board's own frame.
    let frame = free_pad_frame();
    for pad in &board.free.pads {
        let net = pad.net.as_ref().map_or(0, |n| n.number);
        push_pieces(
            &mut out,
            ItemRef::FreePad(pad.uuid),
            net,
            pad_pieces(&frame, pad),
        );
    }
    for segment in &board.segments {
        let layers = layer_mask(std::iter::once(segment.layer.as_str()));
        if let (Ok(shape), true) = (track_shape(segment), layers != 0) {
            out.push(Obstacle {
                item: ItemRef::Track(segment.uuid),
                net: segment.net,
                layers,
                copper: Copper::Shape(shape),
            });
        }
    }
    // Free copper arcs (#2373), one obstacle per chord.
    for arc in &board.free.arcs {
        let layers = layer_mask(std::iter::once(arc.layer.as_str()));
        if layers == 0 {
            continue;
        }
        let shapes = arc_shapes(arc).unwrap_or_default();
        out.extend(shapes.into_iter().map(|shape| Obstacle {
            item: ItemRef::Arc(arc.uuid),
            net: arc.net,
            layers,
            copper: Copper::Shape(shape),
        }));
    }
    for via in &board.vias {
        push_pieces(&mut out, ItemRef::Via(via.uuid), via.net, via_pieces(via));
    }
    for zone in &board.zones {
        let layers = layer_mask(std::iter::once(zone.layer.as_str()));
        if zone.is_pour() || zone.is_rule_area() || layers == 0 {
            continue;
        }
        if let Some(area) = zone_area(zone).filter(|a| a.outer.len() >= 3) {
            out.push(Obstacle {
                item: ItemRef::Zone {
                    zone: zone.uuid,
                    kind: PickKind::of_zone(zone).unwrap_or(PickKind::Regions),
                },
                net: zone.net,
                layers,
                copper: Copper::Area(area),
            });
        }
    }
    out
}

/// A pad or via as one obstacle per group of layers sharing a size
/// (#2401); none when its geometry does not convert.
fn push_pieces(
    out: &mut Vec<Obstacle>,
    item: ItemRef,
    net: u32,
    pieces: Result<Vec<(u32, Shape)>, GeomError>,
) {
    out.extend(
        pieces
            .unwrap_or_default()
            .into_iter()
            .map(|(layers, shape)| Obstacle {
                item,
                net,
                layers,
                copper: Copper::Shape(shape),
            }),
    );
}

/// What new copper is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NewCopper {
    /// A via: kept off pads and vias of its own net as well.
    Via,
    /// A track or a teardrop: joins its own net's copper freely.
    Route,
}

/// Whether `shape` of `net` on `layers` comes closer than `clearance`
/// allows to an obstacle of another net (no net counts as another), or, for
/// a via, overlaps or comes that close to a pad or via of its own net.
/// Obstacles `skip` names are passed over.
pub(crate) fn clashes<F: Fn(u32, u32) -> i64>(
    shape: &Shape,
    new: NewCopper,
    net: u32,
    layers: u32,
    obstacles: &[Obstacle],
    clearance: &F,
    skip: impl Fn(ItemRef) -> bool,
) -> bool {
    obstacles.iter().any(|o| {
        if o.layers & layers == 0 || skip(o.item) {
            return false;
        }
        let other_net = o.net != net || o.net == 0;
        if !other_net && !(new == NewCopper::Via && o.is_landing()) {
            return false;
        }
        o.near(shape, clearance(net, o.net))
    })
}

/// Whether the polygon `ring` of `net` on `layers` comes closer than
/// `clearance` allows to copper of another net. Touching the clearance line
/// counts, so the answer errs on the safe side.
pub(crate) fn ring_clashes<F: Fn(u32, u32) -> i64>(
    ring: &[PointNm],
    net: u32,
    layers: u32,
    obstacles: &[Obstacle],
    clearance: &F,
) -> bool {
    obstacles.iter().any(|o| {
        o.layers & layers != 0
            && (o.net != net || o.net == 0)
            && o.near_ring(ring, clearance(net, o.net))
    })
}

/// A keepout boundary in nm with the copper layers it covers and what it
/// forbids of new copper.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct KeepoutArea {
    pub polygon: Vec<PointNm>,
    pub layers: u32,
    pub forbids_vias: bool,
    pub forbids_routes: bool,
}

impl KeepoutArea {
    fn of(keepout: &PcbKeepout) -> Option<Self> {
        let layers = keepout
            .layers
            .iter()
            .filter_map(|layer| layer.copper())
            .fold(0, |mask, layer| mask | 1 << layer_bit(layer));
        let polygon = keepout
            .boundary
            .iter()
            .map(|&p| PointNm::from_mm(p, "keepout.boundary"))
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        let kinds = keepout.forbids;
        (layers != 0 && polygon.len() >= 3).then_some(KeepoutArea {
            polygon,
            layers,
            forbids_vias: kinds.no_copper || kinds.no_vias || kinds.no_drilling,
            forbids_routes: kinds.no_copper || kinds.no_routing,
        })
    }

    /// Whether the keepout forbids `new` and meets `shape` on `layers`.
    pub fn bars(&self, new: NewCopper, layers: u32, shape: &Shape) -> bool {
        self.forbids(new) && self.layers & layers != 0 && shape.overlaps_polygon(&self.polygon)
    }

    /// Whether the keepout forbids `new` and meets the polygon `ring`.
    pub fn bars_ring(&self, new: NewCopper, layers: u32, ring: &[PointNm]) -> bool {
        self.forbids(new) && self.layers & layers != 0 && polygons_meet(&self.polygon, ring)
    }

    fn forbids(&self, new: NewCopper) -> bool {
        match new {
            NewCopper::Via => self.forbids_vias,
            NewCopper::Route => self.forbids_routes,
        }
    }
}

/// The board-sketch keepouts and the keepouts drawn on the board, as the
/// design rule check reads them.
pub(crate) fn keepout_areas(board: &PcbBoard) -> Vec<KeepoutArea> {
    let drawn: Vec<PcbKeepout> = board.zones.iter().filter_map(|z| z.as_keepout()).collect();
    board
        .sketch
        .iter()
        .flat_map(|stored| stored.keepouts.iter())
        .chain(&drawn)
        .filter_map(KeepoutArea::of)
        .collect()
}

/// Whether the disc of `radius` about `center` lies inside `area`, clear of
/// its outer ring and of every hole by at least `margin`.
pub(crate) fn disc_inside(area: &Polygon, center: PointNm, radius: i64, margin: i64) -> bool {
    if area.outer.len() < 3 || !point_in_polygon(&area.outer, center) {
        return false;
    }
    if area
        .holes
        .iter()
        .any(|hole| hole.len() >= 3 && point_in_polygon(hole, center))
    {
        return false;
    }
    let disc = Shape::Disc {
        center,
        radius: radius.max(0) + margin.max(0),
    };
    let rings = std::iter::once(&area.outer).chain(area.holes.iter());
    !rings.into_iter().any(|ring| {
        (0..ring.len()).any(|i| {
            disc.overlaps(&Shape::Capsule {
                a: ring[i],
                b: ring[(i + 1) % ring.len()],
                radius: 0,
            })
        })
    })
}
