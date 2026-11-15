//! Polygon pour fills (#2216): the copper a pour holds.
//!
//! A pour's fill is its outline less its holes, less the copper of every
//! other net on its layer grown by the pour's clearance, less the keepouts
//! that keep pours off that layer. Copper of the pour's own net is filled
//! over and joined solidly: no thermal relief spokes. Pours do not clear
//! each other, so two pours of different nets that overlap both fill the
//! overlap. Rules and what is not modelled: `docs/PCB_TOOLS.md`,
//! "Polygon Pour".

use signex_types::pcb::{PcbBoard, Zone};
use signex_types::pcb_layer::PcbLayer;
use uuid::Uuid;

use crate::copper::{layer_mask, pad_layers, via_layers};
use crate::index::{track_shape, via_shape};
use crate::rect::RectNm;
use crate::region::{Polygon, chord_sag, difference, offset};
use crate::shape::Shape;
use crate::transform::pad_shape;
use crate::units::{PointNm, mm_to_nm};

/// Points per quarter turn of a round obstacle's clearance ring.
const OBSTACLE_ARC_STEPS: usize = 8;

/// The fill of one pour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneFill {
    pub zone: Uuid,
    /// The pour's position in `PcbBoard::zones`.
    pub index: usize,
    pub net: u32,
    /// The pour's copper layer, as a [`crate::layer_bit`] mask.
    pub layers: u32,
    /// The separate areas of copper; empty when nothing is left to fill.
    pub pieces: Vec<Polygon>,
}

/// The fills of a board's pours, in board order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ZoneFills {
    fills: Vec<ZoneFill>,
}

impl ZoneFills {
    /// The fill of the zone at `index` of `PcbBoard::zones`, if it is a
    /// pour that could be filled.
    pub fn of(&self, index: usize) -> Option<&ZoneFill> {
        self.fills
            .binary_search_by_key(&index, |f| f.index)
            .ok()
            .map(|i| &self.fills[i])
    }

    pub fn iter(&self) -> impl Iterator<Item = &ZoneFill> {
        self.fills.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.fills.is_empty()
    }
}

/// The fill of every pour of `board`. A pour on no copper layer, or with
/// an outline or clearance the geometry cannot hold, has no fill.
pub fn zone_fills(board: &PcbBoard) -> ZoneFills {
    let fills = board
        .zones
        .iter()
        .enumerate()
        .filter(|(_, zone)| zone.is_pour())
        .filter_map(|(index, zone)| fill_pour(board, index, zone))
        .collect();
    ZoneFills { fills }
}

/// The copper area a zone's outline and usable holes enclose; `None` for an
/// outline the geometry cannot hold.
pub fn zone_area(zone: &Zone) -> Option<Polygon> {
    let ring = |points: &[signex_types::pcb::Point]| {
        points
            .iter()
            .map(|&p| PointNm::from_mm(p, "zone.outline"))
            .collect::<Result<Vec<_>, _>>()
            .ok()
    };
    let outer = ring(&zone.outline)?;
    let holes = zone
        .usable_holes()
        .into_iter()
        .filter_map(|i| ring(&zone.holes[i]))
        .collect();
    Some(Polygon::new(outer, holes))
}

fn fill_pour(board: &PcbBoard, index: usize, zone: &Zone) -> Option<ZoneFill> {
    let layers = layer_mask(std::iter::once(zone.layer.as_str()));
    if layers == 0 {
        return None;
    }
    let area = zone_area(zone).filter(|a| a.outer.len() >= 3)?;
    let clearance = mm_to_nm(zone.pour_clearance_mm(board), "zone.clearance").ok()?;
    let reach = area.bounds()?.inflate(clearance);
    let obstacles = Obstacles {
        board,
        zone,
        layers,
        clearance,
        reach,
    }
    .collect();
    let mut pieces = difference(&[area], &obstacles);
    if let Ok(min) = mm_to_nm(zone.min_thickness, "zone.min_thickness")
        && min > 1
    {
        pieces = without_thin_parts(&pieces, min);
    }
    Some(ZoneFill {
        zone: zone.uuid,
        index,
        net: zone.net,
        layers,
        pieces,
    })
}

/// `pieces` less every part narrower than `width`: shrunk by half of it and
/// grown back. The grown corners lie on or inside the arcs they stand for,
/// so the result never reaches past the pieces.
fn without_thin_parts(pieces: &[Polygon], width: i64) -> Vec<Polygon> {
    let half = width / 2;
    let opened = offset(&offset(pieces, -half), half);
    crate::region::intersection(&opened, pieces)
}

/// What a pour on `layers` must stay clear of.
struct Obstacles<'a> {
    board: &'a PcbBoard,
    zone: &'a Zone,
    layers: u32,
    clearance: i64,
    /// Nothing outside this rectangle can reach the fill.
    reach: RectNm,
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

    fn collect(&self) -> Vec<Polygon> {
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
        self.sketch_keepouts(&mut out);
        out
    }

    /// Keepout zones on the pour's layer, and solid copper zones of other
    /// nets grown by the clearance. Other pours are not cleared.
    fn zones(&self, out: &mut Vec<Polygon>) {
        for other in &self.board.zones {
            if other.is_pour() {
                continue;
            }
            if let Some(keepout) = other.as_keepout() {
                if self.keeps_out(&keepout.layers)
                    && let Some(area) = ring_nm(&keepout.boundary)
                {
                    out.push(Polygon::new(area, Vec::new()));
                }
                continue;
            }
            let on_layer = layer_mask(std::iter::once(other.layer.as_str())) & self.layers != 0;
            if !on_layer || !self.clears(other.net) || other.is_rule_area() {
                continue;
            }
            let Some(area) = zone_area(other).filter(|a| a.outer.len() >= 3) else {
                continue;
            };
            if area
                .bounds()
                .is_some_and(|b| b.inflate(self.clearance).intersects(&self.reach))
            {
                let by = self.clearance + chord_sag(self.clearance) + 2;
                out.extend(offset(&[area], by));
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
                && let Some(area) = ring_nm(&keepout.boundary)
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

fn ring_nm(points: &[signex_types::pcb::Point]) -> Option<Vec<PointNm>> {
    let ring = points
        .iter()
        .map(|&p| PointNm::from_mm(p, "keepout.boundary"))
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (ring.len() >= 3).then_some(ring)
}

#[cfg(test)]
#[path = "zone_fill_tests.rs"]
mod tests;
