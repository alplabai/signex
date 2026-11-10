//! The keepout check (#2234): what a board-sketch keepout forbids, found
//! inside it on a layer it covers. Any net counts, net 0 included: a
//! keepout keeps out copper, not a net.
//!
//! - `no_routing` forbids tracks; `no_vias` vias; `no_copper` tracks, vias
//!   and pads; `no_drilling` vias and drilled pads, on every layer the
//!   keepout covers; `no_components` a footprint whose body meets the
//!   keepout on its own side's outer copper layer.
//! - `no_pours` is not checked yet (zones are not tested against keepouts).
//!
//! One violation per keepout and item, however many kinds forbid it.

use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::CopperLayer;
use signex_types::pcb_sketch::PcbKeepout;

use crate::body::footprint_body;
use crate::copper::{layer_bit, layer_mask, pad_layers, via_layers};
use crate::index::{ItemRef, track_shape, via_shape};
use crate::rect::RectNm;
use crate::shape::Shape;
use crate::transform::{Side, pad_shape};
use crate::units::PointNm;

use super::{Violation, ViolationKind};

/// Every keepout violation of `board`, unsorted.
pub(super) fn violations(board: &PcbBoard) -> Vec<Violation> {
    let Some(stored) = board.sketch.as_ref() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for keepout in &stored.keepouts {
        if let Some(area) = Area::of(keepout) {
            area.check(board, &mut found);
        }
    }
    found
}

/// A keepout in nm with the copper layers it covers.
struct Area<'a> {
    keepout: &'a PcbKeepout,
    polygon: Vec<PointNm>,
    bounds: RectNm,
    /// Bit mask of the copper layers listed (see `copper::layer_bit`).
    layers: u32,
}

impl<'a> Area<'a> {
    /// `None` for a keepout with a corner the geometry cannot hold, or no
    /// copper layer: it can forbid nothing this check sees.
    fn of(keepout: &'a PcbKeepout) -> Option<Self> {
        let polygon = keepout
            .boundary
            .iter()
            .map(|&p| PointNm::from_mm(p, "keepout.boundary"))
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        let bounds = RectNm::bounding(polygon.iter().copied())?;
        let layers = keepout
            .layers
            .iter()
            .filter_map(|layer| layer.copper())
            .fold(0, |mask, layer| mask | 1 << layer_bit(layer));
        (layers != 0).then_some(Area {
            keepout,
            polygon,
            bounds,
            layers,
        })
    }

    fn check(&self, board: &PcbBoard, found: &mut Vec<Violation>) {
        let forbids = self.keepout.forbids;
        let copper = forbids.no_copper;
        if forbids.no_routing || copper {
            for segment in &board.segments {
                let layers = layer_mask(std::iter::once(segment.layer.as_str()));
                if let Ok(shape) = track_shape(segment) {
                    let item = ItemRef::Track(segment.uuid);
                    self.meet(item, segment.net, layers, &shape, found);
                }
            }
        }
        if forbids.no_vias || forbids.no_drilling || copper {
            for via in &board.vias {
                let layers = if forbids.no_drilling {
                    u32::MAX
                } else {
                    via_layers(via)
                };
                if let Ok(shape) = via_shape(via) {
                    self.meet(ItemRef::Via(via.uuid), via.net, layers, &shape, found);
                }
            }
        }
        for footprint in &board.footprints {
            if copper || forbids.no_drilling {
                for pad in &footprint.pads {
                    let drilled = forbids.no_drilling && pad.drill.is_some();
                    let layers = match (drilled, copper) {
                        (true, _) => u32::MAX,
                        (false, true) => pad_layers(footprint, pad),
                        (false, false) => continue,
                    };
                    let net = pad.net.as_ref().map_or(0, |n| n.number);
                    let item = ItemRef::Pad {
                        footprint: footprint.uuid,
                        pad: pad.uuid,
                    };
                    if let Ok(shape) = pad_shape(footprint, pad) {
                        self.meet(item, net, layers, &shape, found);
                    }
                }
            }
            if forbids.no_components {
                let side = match Side::of_layer(&footprint.layer) {
                    Side::Top => CopperLayer::Top,
                    Side::Bottom => CopperLayer::Bottom,
                };
                if let Ok(body) = footprint_body(footprint) {
                    let item = ItemRef::Footprint(footprint.uuid);
                    self.meet(item, 0, 1 << layer_bit(side), &body, found);
                }
            }
        }
    }

    /// Record a violation when `shape`, on `layers`, meets the keepout.
    fn meet(
        &self,
        item: ItemRef,
        net: u32,
        layers: u32,
        shape: &Shape,
        found: &mut Vec<Violation>,
    ) {
        let shared = layers & self.layers;
        let aabb = shape.aabb();
        if shared == 0 || !aabb.intersects(&self.bounds) || !shape.overlaps_polygon(&self.polygon) {
            return;
        }
        found.push(Violation {
            kind: ViolationKind::Keepout,
            items: vec![item],
            nets: (net, net),
            layers: shared,
            at: PointNm::new(
                aabb.min.x + (aabb.max.x - aabb.min.x) / 2,
                aabb.min.y + (aabb.max.y - aabb.min.y) / 2,
            ),
            measured_nm: 0,
            required_nm: 0,
            keepout: Some(self.keepout.uuid),
        });
    }
}
