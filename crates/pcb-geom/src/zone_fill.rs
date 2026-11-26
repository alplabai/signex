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
//! A shelved pour (`Zone::is_shelved_pour`) has no fill and clears nothing;
//! an unfilled one (`Zone::is_unfilled_pour`, #2506) neither, and is listed
//! as [`Unfilled::UntilRepour`].
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

/// Why a pour has no fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unfilled {
    /// Its layer is no copper layer.
    NoCopperLayer,
    /// Its outline has fewer than three corners the geometry can hold.
    Outline,
    /// Its clearance is not a length the geometry can hold.
    Clearance,
    /// It was unfilled (`Zone::unfilled`, #2506) and holds no copper until
    /// the next Repour.
    UntilRepour,
}

/// The fills of a board's pours, in board order, and why the pours with
/// none have none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ZoneFills {
    fills: Vec<ZoneFill>,
    /// `(index in PcbBoard::zones, zone, why)`, in board order.
    unfilled: Vec<(usize, Uuid, Unfilled)>,
}

impl ZoneFills {
    /// Why the pour at `index` of `PcbBoard::zones` has no fill; `None`
    /// when it has one or is no pour.
    pub fn unfilled(&self, index: usize) -> Option<Unfilled> {
        self.unfilled
            .iter()
            .find(|(at, _, _)| *at == index)
            .map(|(_, _, why)| *why)
    }

    /// Every pour with no fill: its index, id and why.
    pub fn unfilled_pours(&self) -> &[(usize, Uuid, Unfilled)] {
        &self.unfilled
    }

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

/// The fill of every pour of `board`. A shelved or unfilled pour, a pour on
/// no copper layer, or one with an outline or clearance the geometry cannot
/// hold, has no fill and clears nothing; [`ZoneFills::unfilled`] says which
/// of the last four it is (a shelved pour is left out on purpose and is not
/// listed).
pub fn zone_fills(board: &PcbBoard) -> ZoneFills {
    let inside = board_area(board);
    let order = pour_fill_order(board);
    let mut fills: Vec<ZoneFill> = Vec::with_capacity(order.len());
    let mut unfilled = Vec::new();
    for index in order {
        let zone = &board.zones[index];
        if zone.is_unfilled_pour() {
            unfilled.push((index, zone.uuid, Unfilled::UntilRepour));
            continue;
        }
        match fill_pour(board, index, zone, &fills, inside.as_deref()) {
            Ok(fill) => fills.push(fill),
            Err(why) => unfilled.push((index, zone.uuid, why)),
        }
    }
    fills.sort_by_key(|fill| fill.index);
    unfilled.sort_by_key(|(index, _, _)| *index);
    ZoneFills { fills, unfilled }
}

/// The pours of `board` in the order they fill, as indices of
/// `PcbBoard::zones`: a higher `Zone::priority` first, then board order. A
/// shelved pour does not fill and is left out; an unfilled pour keeps its
/// place but [`zone_fills`] does not fill it. The Polygon Manager's Order
/// column reads it (mockup row 178).
pub fn pour_fill_order(board: &PcbBoard) -> Vec<usize> {
    let mut order: Vec<(usize, &Zone)> = board
        .zones
        .iter()
        .enumerate()
        .filter(|(_, zone)| zone.is_pour() && !zone.is_shelved_pour())
        .collect();
    order.sort_by_key(|(index, zone)| (Reverse(zone.priority), *index));
    order.into_iter().map(|(index, _)| index).collect()
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
) -> Result<ZoneFill, Unfilled> {
    let layers = layer_mask(std::iter::once(zone.layer.as_str()));
    if layers == 0 {
        return Err(Unfilled::NoCopperLayer);
    }
    let area = zone_area(zone)
        .filter(|a| a.outer.len() >= 3)
        .ok_or(Unfilled::Outline)?;
    let clearance = mm_to_nm(zone.pour_clearance_mm(board), "zone.clearance")
        .map_err(|_| Unfilled::Clearance)?;
    let reach = area.bounds().ok_or(Unfilled::Outline)?.inflate(clearance);
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
    Ok(ZoneFill {
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
