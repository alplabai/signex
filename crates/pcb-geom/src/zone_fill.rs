//! Polygon pour fills (#2216): the copper a pour holds.
//!
//! A pour's fill is its outline less its holes, less the copper of every
//! other net on its layer grown by the pour's clearance, less the keepouts
//! that keep pours off that layer, inside the board outline. Copper of the
//! pour's own net is filled over and joined solidly: no thermal relief
//! spokes. Pours fill in priority order (a higher `Zone::priority` first,
//! then board order), and each clears the fills of other nets' pours filled
//! before it. Islands below the pour's minimum area, and with
//! `remove_dead_islands` those touching no copper of its net, are dropped.
//! A shelved pour (`Zone::is_shelved_pour`) has no fill and clears nothing.
//! Rules and what is not modelled: `docs/PCB_TOOLS.md`, "Polygon Pour".

mod islands;
mod obstacles;

use std::cmp::Reverse;

use signex_types::pcb::{PcbBoard, Zone};
use uuid::Uuid;

use crate::copper::layer_mask;
use crate::region::{Polygon, difference, intersection, offset};
use crate::units::{PointNm, mm_to_nm};
use obstacles::Obstacles;

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
    /// How many islands the pour's island rules dropped.
    pub islands_removed: usize,
}

impl ZoneFill {
    /// The copper area of the fill, in mm².
    pub fn area_mm2(&self) -> f64 {
        let twice: i128 = self.pieces.iter().map(Polygon::twice_area).sum();
        twice as f64 / 2.0e12
    }
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

/// The fill of every pour of `board`. A shelved pour, a pour on no copper
/// layer, or one with an outline or clearance the geometry cannot hold, has
/// no fill.
pub fn zone_fills(board: &PcbBoard) -> ZoneFills {
    let inside = board_area(board);
    let mut order: Vec<(usize, &Zone)> = board
        .zones
        .iter()
        .enumerate()
        .filter(|(_, zone)| zone.is_pour() && !zone.is_shelved_pour())
        .collect();
    order.sort_by_key(|(index, zone)| (Reverse(zone.priority), *index));
    let mut fills: Vec<ZoneFill> = Vec::with_capacity(order.len());
    for (index, zone) in order {
        if let Some(fill) = fill_pour(board, index, zone, &fills, inside.as_deref()) {
            fills.push(fill);
        }
    }
    fills.sort_by_key(|fill| fill.index);
    ZoneFills { fills }
}

/// The copper area a zone's outline and usable holes enclose; `None` for an
/// outline the geometry cannot hold.
pub fn zone_area(zone: &Zone) -> Option<Polygon> {
    let outer = ring_nm(&zone.outline)?;
    let holes = zone
        .usable_holes()
        .into_iter()
        .filter_map(|i| ring_nm(&zone.holes[i]))
        .collect();
    Some(Polygon::new(outer, holes))
}

/// The area the board outline encloses less the sketch's through cutouts;
/// `None` for a board with no outline (or one the geometry cannot hold),
/// whose pours are not clipped.
fn board_area(board: &PcbBoard) -> Option<Vec<Polygon>> {
    let outline = ring_nm(&board.outline).filter(|r| r.len() >= 3)?;
    let cutouts: Vec<Polygon> = board
        .sketch
        .iter()
        .flat_map(|sketch| &sketch.cutouts)
        .filter(|cutout| cutout.through)
        .filter_map(|cutout| ring_nm(&cutout.boundary).filter(|r| r.len() >= 3))
        .map(|ring| Polygon::new(ring, Vec::new()))
        .collect();
    Some(difference(&[Polygon::new(outline, Vec::new())], &cutouts))
}

fn fill_pour(
    board: &PcbBoard,
    index: usize,
    zone: &Zone,
    earlier: &[ZoneFill],
    inside: Option<&[Polygon]>,
) -> Option<ZoneFill> {
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
        earlier,
    }
    .collect();
    let mut pieces = difference(&[area], &obstacles);
    if let Some(inside) = inside {
        pieces = intersection(&pieces, inside);
    }
    if let Ok(min) = mm_to_nm(zone.min_thickness, "zone.min_thickness")
        && min > 1
    {
        pieces = without_thin_parts(&pieces, min);
    }
    let (pieces, islands_removed) = islands::drop_islands(board, zone, layers, pieces);
    Some(ZoneFill {
        zone: zone.uuid,
        index,
        net: zone.net,
        layers,
        pieces,
        islands_removed,
    })
}

/// `pieces` less every part narrower than `width`: shrunk by half of it and
/// grown back. The grown corners lie on or inside the arcs they stand for,
/// so the result never reaches past the pieces.
fn without_thin_parts(pieces: &[Polygon], width: i64) -> Vec<Polygon> {
    let half = width / 2;
    let opened = offset(&offset(pieces, -half), half);
    intersection(&opened, pieces)
}

/// A ring of board points in nm; `None` when a point is beyond what the
/// geometry holds.
fn ring_nm(points: &[signex_types::pcb::Point]) -> Option<Vec<PointNm>> {
    points
        .iter()
        .map(|&p| PointNm::from_mm(p, "zone.outline"))
        .collect::<Result<Vec<_>, _>>()
        .ok()
}

#[cfg(test)]
#[path = "zone_fill_tests.rs"]
mod tests;
