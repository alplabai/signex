//! What new copper must keep clear of (#2408): every pad, track and via of
//! the board with its net and copper layers, no net included, and the
//! keepouts that forbid it. Via stitching, teardrops and glossing read
//! this one list, so they agree on what "clear" means: the clearance a
//! caller's rule gives between two nets, measured as the design rule
//! check measures it (`Shape::separation`).

use signex_types::pcb::PcbBoard;
use signex_types::pcb_sketch::PcbKeepout;

use crate::copper::{layer_bit, layer_mask, pad_layers, via_layers};
use crate::index::{ItemRef, track_shape, via_shape};
use crate::region::Polygon;
use crate::shape::Shape;
use crate::shape_overlap::{point_in_polygon, polygons_meet};
use crate::transform::pad_shape;
use crate::units::PointNm;

/// One pad, track or via, with the net it is on (0: none).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Obstacle {
    pub item: ItemRef,
    pub net: u32,
    /// Copper layers it reaches, as a [`layer_bit`] mask.
    pub layers: u32,
    pub shape: Shape,
}

impl Obstacle {
    /// A pad or a via: copper a new via keeps off even on its own net.
    pub fn is_landing(&self) -> bool {
        matches!(self.item, ItemRef::Pad { .. } | ItemRef::Via(_))
    }
}

/// Every pad, track and via whose geometry converts. A track on no copper
/// layer is not copper and is left out.
pub(crate) fn copper_obstacles(board: &PcbBoard) -> Vec<Obstacle> {
    let mut out = Vec::new();
    for footprint in &board.footprints {
        for pad in &footprint.pads {
            if let Ok(shape) = pad_shape(footprint, pad) {
                out.push(Obstacle {
                    item: ItemRef::Pad {
                        footprint: footprint.uuid,
                        pad: pad.uuid,
                    },
                    net: pad.net.as_ref().map_or(0, |n| n.number),
                    layers: pad_layers(footprint, pad),
                    shape,
                });
            }
        }
    }
    for segment in &board.segments {
        let layers = layer_mask(std::iter::once(segment.layer.as_str()));
        if let (Ok(shape), true) = (track_shape(segment), layers != 0) {
            out.push(Obstacle {
                item: ItemRef::Track(segment.uuid),
                net: segment.net,
                layers,
                shape,
            });
        }
    }
    for via in &board.vias {
        if let Ok(shape) = via_shape(via) {
            out.push(Obstacle {
                item: ItemRef::Via(via.uuid),
                net: via.net,
                layers: via_layers(via),
                shape,
            });
        }
    }
    out
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
    let bounds = shape.aabb();
    obstacles.iter().any(|o| {
        if o.layers & layers == 0 || skip(o.item) {
            return false;
        }
        let other_net = o.net != net || o.net == 0;
        if !other_net && !(new == NewCopper::Via && o.is_landing()) {
            return false;
        }
        let gap = clearance(net, o.net).max(0);
        o.shape.aabb().inflate(gap).intersects(&bounds)
            && (shape.overlaps(&o.shape) || shape.separation(&o.shape).is_under(gap))
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
            && o.shape
                .inflated(clearance(net, o.net).max(0))
                .overlaps_polygon(ring)
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
