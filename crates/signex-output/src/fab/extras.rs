//! The copper areas of a board's zones as the images draw them: a pour's
//! computed fill (#2216) and any other copper zone filled solid, the same
//! copper the canvas and the board PDF show.

use pcb_geom::{PointNm, Polygon, ZoneFills, zone_area, zone_fills};
use signex_types::pcb::{PcbBoard, Point, Zone};
use signex_types::pcb_layer::PcbLayer;

use super::{FabExtras, FilledRegion};

impl FabExtras {
    /// The extras of `board`: its zones' copper. Its text is written from
    /// the board itself (#2368), so `strokes` is empty.
    pub fn of_board(board: &PcbBoard) -> Self {
        FabExtras {
            zone_fills: zone_regions(board, &zone_fills(board)),
            strokes: Vec::new(),
        }
    }
}

/// Every copper area of `board`'s zones, in board order: each piece of a
/// pour's fill from `fills`, and the outline less holes of every other
/// copper zone that is not a rule area or keepout.
fn zone_regions(board: &PcbBoard, fills: &ZoneFills) -> Vec<FilledRegion> {
    let mut regions = Vec::new();
    for (index, zone) in board.zones.iter().enumerate() {
        let Some(layer) = zone.pcb_layer().filter(|l| l.is_copper()) else {
            continue;
        };
        if zone.is_rule_area() {
            continue;
        }
        let pieces = if zone.is_pour() {
            fills
                .of(index)
                .map(|fill| fill.pieces.clone())
                .unwrap_or_default()
        } else {
            zone_area(zone).into_iter().collect()
        };
        let net_name = zone_net_name(board, zone);
        regions.extend(pieces.iter().map(|piece| region(layer, &net_name, piece)));
    }
    regions
}

fn region(layer: PcbLayer, net_name: &str, piece: &Polygon) -> FilledRegion {
    FilledRegion {
        layer,
        net_name: net_name.to_string(),
        outline: ring_mm(&piece.outer),
        holes: piece.holes.iter().map(|hole| ring_mm(hole)).collect(),
    }
}

fn ring_mm(ring: &[PointNm]) -> Vec<Point> {
    ring.iter().map(|p| p.to_mm()).collect()
}

/// The zone's net name, else the name the board gives its net number.
fn zone_net_name(board: &PcbBoard, zone: &Zone) -> String {
    if !zone.net_name.is_empty() || zone.net == 0 {
        return zone.net_name.clone();
    }
    board
        .nets
        .iter()
        .find(|n| n.number == zone.net)
        .map(|n| n.name.clone())
        .unwrap_or_default()
}
