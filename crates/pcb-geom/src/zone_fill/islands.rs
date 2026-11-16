//! Island removal of a pour's fill (#2216): the pieces below the pour's
//! minimum island area, and with `remove_dead_islands` the pieces that touch
//! no copper of the pour's net (dead copper). A pour with no net keeps its
//! islands under the second rule: no copper is "its net".

use signex_types::pcb::{PcbBoard, Zone};

use super::zone_area;
use crate::area_touch::{areas_meet, shape_meets_area};
use crate::copper::{layer_mask, pad_layers, via_layers};
use crate::index::{track_shape, via_shape};
use crate::region::Polygon;
use crate::shape::Shape;
use crate::transform::pad_shape;

/// Twice an area of one mm², in nm².
const TWICE_NM2_PER_MM2: f64 = 2.0e12;

/// `pieces` less the islands `zone`'s rules drop, and how many were dropped.
pub(super) fn drop_islands(
    board: &PcbBoard,
    zone: &Zone,
    layers: u32,
    pieces: Vec<Polygon>,
) -> (Vec<Polygon>, usize) {
    let min_twice = zone
        .pour_min_island_mm2()
        .map(|mm2| (mm2 * TWICE_NM2_PER_MM2).ceil() as i128);
    let dead = zone.remove_dead_islands && zone.net != 0;
    if min_twice.is_none() && !dead {
        return (pieces, 0);
    }
    let anchors = if dead {
        NetCopper::of(board, zone, layers)
    } else {
        NetCopper::default()
    };
    let before = pieces.len();
    let kept: Vec<Polygon> = pieces
        .into_iter()
        .filter(|piece| min_twice.is_none_or(|min| piece.twice_area() >= min))
        .filter(|piece| !dead || anchors.touches(piece))
        .collect();
    let removed = before - kept.len();
    (kept, removed)
}

/// The copper of a pour's net on its layer, other than pours: what keeps an
/// island of its fill alive.
#[derive(Default)]
struct NetCopper {
    shapes: Vec<Shape>,
    areas: Vec<Polygon>,
}

impl NetCopper {
    fn of(board: &PcbBoard, zone: &Zone, layers: u32) -> Self {
        let net = zone.net;
        let mut shapes = Vec::new();
        for footprint in &board.footprints {
            for pad in &footprint.pads {
                let on_net = pad.net.as_ref().is_some_and(|n| n.number == net);
                if on_net && pad_layers(footprint, pad) & layers != 0 {
                    shapes.extend(pad_shape(footprint, pad).ok());
                }
            }
        }
        let on_layer = |name: &str| layer_mask(std::iter::once(name)) & layers != 0;
        for segment in &board.segments {
            if segment.net == net && on_layer(&segment.layer) {
                shapes.extend(track_shape(segment).ok());
            }
        }
        for via in &board.vias {
            if via.net == net && via_layers(via) & layers != 0 {
                shapes.extend(via_shape(via).ok());
            }
        }
        let areas = board
            .zones
            .iter()
            .filter(|other| other.uuid != zone.uuid && !other.is_pour() && !other.is_rule_area())
            .filter(|other| other.net == net && on_layer(&other.layer))
            .filter_map(zone_area)
            .filter(|area| area.outer.len() >= 3)
            .collect();
        NetCopper { shapes, areas }
    }

    fn touches(&self, piece: &Polygon) -> bool {
        self.shapes.iter().any(|s| shape_meets_area(s, piece))
            || self.areas.iter().any(|a| areas_meet(a, piece))
    }
}
