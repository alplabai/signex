//! What a pour's fill keeps clear of (#2216): other nets' copper on its
//! layer grown by the clearance, keepouts, and the fills of other nets'
//! pours filled before it.

use signex_types::pcb::{PcbBoard, Zone};
use signex_types::pcb_layer::PcbLayer;

use super::{ZoneFill, ring_nm, zone_area};
use crate::copper::{layer_mask, pad_layers, via_layers};
use crate::index::{track_shape, via_shape};
use crate::rect::RectNm;
use crate::region::{Polygon, chord_sag, offset};
use crate::shape::Shape;
use crate::transform::pad_shape;

/// Points per quarter turn of a round obstacle's clearance ring.
const OBSTACLE_ARC_STEPS: usize = 8;

/// What a pour on `layers` must stay clear of.
pub(super) struct Obstacles<'a> {
    pub board: &'a PcbBoard,
    pub zone: &'a Zone,
    pub layers: u32,
    pub clearance: i64,
    /// Nothing outside this rectangle can reach the fill.
    pub reach: RectNm,
    /// The fills of the pours filled before this one (a higher priority).
    pub earlier: &'a [ZoneFill],
}

impl Obstacles<'_> {
    /// Whether copper of `net` is cleared: any net but the pour's own, and
    /// every net when the pour has none.
    fn clears(&self, net: u32) -> bool {
        self.zone.net == 0 || net != self.zone.net
    }

    /// `shape` grown by the clearance, if it can reach the fill.
    fn grown(&self, shape: Shape) -> Option<Polygon> {
        let grown = shape.inflated(self.clearance);
        grown
            .aabb()
            .intersects(&self.reach)
            .then(|| Polygon::new(grown.covering_ring(OBSTACLE_ARC_STEPS), Vec::new()))
    }

    /// `areas` grown by the clearance, when they can reach the fill. The
    /// offset's rounded corners fall inside the true arc by at most the
    /// chord sag, so the offset adds it back.
    fn grown_areas(&self, areas: &[Polygon], out: &mut Vec<Polygon>) {
        let near: Vec<Polygon> = areas
            .iter()
            .filter(|area| {
                area.bounds()
                    .is_some_and(|b| b.inflate(self.clearance).intersects(&self.reach))
            })
            .cloned()
            .collect();
        if near.is_empty() {
            return;
        }
        let by = self.clearance + chord_sag(self.clearance) + 2;
        out.extend(offset(&near, by));
    }

    pub(super) fn collect(&self) -> Vec<Polygon> {
        let mut out = Vec::new();
        for footprint in &self.board.footprints {
            for pad in &footprint.pads {
                let net = pad.net.as_ref().map_or(0, |n| n.number);
                if pad_layers(footprint, pad) & self.layers == 0 || !self.clears(net) {
                    continue;
                }
                out.extend(pad_shape(footprint, pad).ok().and_then(|s| self.grown(s)));
            }
        }
        for segment in &self.board.segments {
            let on_layer = layer_mask(std::iter::once(segment.layer.as_str())) & self.layers != 0;
            if on_layer && self.clears(segment.net) {
                out.extend(track_shape(segment).ok().and_then(|s| self.grown(s)));
            }
        }
        for via in &self.board.vias {
            if via_layers(via) & self.layers != 0 && self.clears(via.net) {
                out.extend(via_shape(via).ok().and_then(|s| self.grown(s)));
            }
        }
        self.zones(&mut out);
        self.earlier_fills(&mut out);
        self.sketch_keepouts(&mut out);
        out
    }

    /// Keepout zones on the pour's layer, and solid copper zones of other
    /// nets grown by the clearance. Other pours are cleared through their
    /// fills ([`Self::earlier_fills`]).
    fn zones(&self, out: &mut Vec<Polygon>) {
        for other in &self.board.zones {
            if other.is_pour() {
                continue;
            }
            if let Some(keepout) = other.as_keepout() {
                if self.keeps_out(&keepout.layers)
                    && let Some(area) = ring_nm(&keepout.boundary).filter(|r| r.len() >= 3)
                {
                    out.push(Polygon::new(area, Vec::new()));
                }
                continue;
            }
            let on_layer = layer_mask(std::iter::once(other.layer.as_str())) & self.layers != 0;
            if !on_layer || !self.clears(other.net) || other.is_rule_area() {
                continue;
            }
            if let Some(area) = zone_area(other).filter(|a| a.outer.len() >= 3) {
                self.grown_areas(std::slice::from_ref(&area), out);
            }
        }
    }

    /// The fills of other nets' pours on the layer that filled before this
    /// one, grown by this pour's clearance (pour-to-pour clearance). A pour
    /// of the same net may overlap them.
    fn earlier_fills(&self, out: &mut Vec<Polygon>) {
        for fill in self.earlier {
            if fill.layers & self.layers != 0 && self.clears(fill.net) {
                self.grown_areas(&fill.pieces, out);
            }
        }
    }

    /// Board-sketch keepouts that forbid copper or pours on the layer.
    fn sketch_keepouts(&self, out: &mut Vec<Polygon>) {
        let Some(sketch) = &self.board.sketch else {
            return;
        };
        for keepout in &sketch.keepouts {
            let forbids = keepout.forbids.no_copper || keepout.forbids.no_pours;
            if forbids
                && self.keeps_out(&keepout.layers)
                && let Some(area) = ring_nm(&keepout.boundary).filter(|r| r.len() >= 3)
            {
                out.push(Polygon::new(area, Vec::new()));
            }
        }
    }

    fn keeps_out(&self, layers: &[PcbLayer]) -> bool {
        layers.iter().any(|layer| {
            layer
                .copper()
                .is_some_and(|c| (1 << crate::copper::layer_bit(c)) & self.layers != 0)
        })
    }
}
