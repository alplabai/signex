//! The keepout check (#2234): what a board-sketch keepout, or a keepout
//! drawn on the board (#2373, `Zone::as_keepout`), forbids, found inside it
//! on a layer it covers. Any net counts, net 0 included: a
//! keepout keeps out copper, not a net.
//!
//! - `no_routing` forbids tracks; `no_vias` vias; `no_copper` tracks, vias
//!   and pads; `no_drilling` vias and drilled pads, on every layer the
//!   keepout covers; `no_components` a footprint whose body meets the
//!   keepout on its own side's outer copper layer.
//! - `no_pours` is not checked yet (zones are not tested against keepouts).
//!
//! One violation per keepout and item, however many kinds forbid it. A
//! keepout whose boundary does not convert, and a footprint body a
//! `no_components` keepout needs that does not, are reported as unchecked
//! (#2300); a track, via or pad that does not convert is already in the
//! copper index's `skipped`.

use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::CopperLayer;
use signex_types::pcb_sketch::PcbKeepout;

use pcb_geom::{
    GeomError, ItemRef, PointNm, RectNm, Rejected, Shape, Side, footprint_body, layer_bit,
    layer_mask, pad_layers, pad_shape, track_shape, via_layers, via_shape,
};

use super::{SkippedKeepout, Violation, ViolationKind};

/// What the keepout check found and what it could not check.
#[derive(Default)]
pub(super) struct Checked {
    pub(super) violations: Vec<Violation>,
    /// Footprint bodies that did not convert, once each.
    pub(super) bodies: Vec<Rejected>,
    pub(super) keepouts: Vec<SkippedKeepout>,
}

/// Every keepout violation of `board`, unsorted, and what was not checked.
pub(super) fn check(board: &PcbBoard) -> Checked {
    let mut out = Checked::default();
    // Keepouts drawn on the board (#2373) are checked like the sketch's.
    let drawn: Vec<PcbKeepout> = board.zones.iter().filter_map(|z| z.as_keepout()).collect();
    let sketch = board.sketch.iter().flat_map(|stored| stored.keepouts.iter());
    for keepout in sketch.chain(&drawn) {
        match Area::of(keepout) {
            Ok(Some(area)) => area.check(board, &mut out),
            Ok(None) => {}
            Err(error) => out.keepouts.push(SkippedKeepout {
                keepout: keepout.uuid,
                error,
            }),
        }
    }
    out
}

/// A keepout in nm with the copper layers it covers.
struct Area<'a> {
    keepout: &'a PcbKeepout,
    polygon: Vec<PointNm>,
    bounds: RectNm,
    /// Bit mask of the copper layers listed (see `pcb_geom::layer_bit`).
    layers: u32,
}

impl<'a> Area<'a> {
    /// `Ok(None)` for a keepout with no copper layer or no corner: it can
    /// forbid nothing this check sees. An error for a corner the geometry
    /// cannot hold: the keepout is not checked, and the run says so.
    fn of(keepout: &'a PcbKeepout) -> Result<Option<Self>, GeomError> {
        let layers = keepout
            .layers
            .iter()
            .filter_map(|layer| layer.copper())
            .fold(0, |mask, layer| mask | 1 << layer_bit(layer));
        if layers == 0 {
            return Ok(None);
        }
        let polygon = keepout
            .boundary
            .iter()
            .map(|&p| PointNm::from_mm(p, "keepout.boundary"))
            .collect::<Result<Vec<_>, _>>()?;
        let Some(bounds) = RectNm::bounding(polygon.iter().copied()) else {
            return Ok(None);
        };
        Ok(Some(Area {
            keepout,
            polygon,
            bounds,
            layers,
        }))
    }

    fn check(&self, board: &PcbBoard, out: &mut Checked) {
        let found = &mut out.violations;
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
                let item = ItemRef::Footprint(footprint.uuid);
                match footprint_body(footprint) {
                    Ok(body) => self.meet(item, 0, 1 << layer_bit(side), &body, found),
                    // Once per footprint, however many keepouts needed it.
                    Err(error) if !out.bodies.iter().any(|r| r.item == item) => {
                        out.bodies.push(Rejected { item, error });
                    }
                    Err(_) => {}
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
            rule: None,
        });
    }
}
