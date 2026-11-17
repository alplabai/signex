//! One row per net of a board (#2403): what the Net Inspector lists on a
//! board and `query_board_nets` answers. Routed length and membership come
//! from `net_length`, the connections still to route from the ratsnest, so
//! every surface reads the same numbers.

use std::collections::BTreeMap;

use signex_types::pcb::PcbBoard;

use crate::net_length::net_routed_length_mm;
use crate::ratsnest::ratsnest;

/// A net of a board and how far it is routed.
#[derive(Debug, Clone, PartialEq)]
pub struct NetReport {
    pub net: u32,
    pub name: String,
    /// The summed centre-line length of its tracks in mm; 0 with none.
    pub routed_length_mm: f64,
    pub tracks: usize,
    pub vias: usize,
    pub pads: usize,
    /// Connections between its pads still to route (ratsnest lines).
    pub unrouted: usize,
}

impl NetReport {
    /// Every pad of the net is joined by copper.
    pub fn is_routed(&self) -> bool {
        self.unrouted == 0
    }
}

/// Every net of `board`'s net table but net 0, by name (then number).
pub fn net_reports(board: &PcbBoard) -> Vec<NetReport> {
    let mut unrouted: BTreeMap<u32, usize> = BTreeMap::new();
    for line in ratsnest(board) {
        *unrouted.entry(line.net).or_default() += 1;
    }
    let mut rows: Vec<NetReport> = board
        .nets
        .iter()
        .filter(|def| def.number != 0)
        .map(|def| {
            let net = def.number;
            let pads = board
                .footprints
                .iter()
                .flat_map(|fp| fp.pads.iter())
                .filter(|pad| pad.net.as_ref().is_some_and(|n| n.number == net))
                .count();
            NetReport {
                net,
                name: def.name.clone(),
                routed_length_mm: net_routed_length_mm(board, net).unwrap_or(0.0),
                tracks: board.segments.iter().filter(|s| s.net == net).count(),
                vias: board.vias.iter().filter(|v| v.net == net).count(),
                pads,
                unrouted: unrouted.get(&net).copied().unwrap_or(0),
            }
        })
        .collect();
    rows.sort_by(|a, b| a.name.cmp(&b.name).then(a.net.cmp(&b.net)));
    rows
}

/// The net of `board` named `name`, compared without case; `None` for an
/// unknown name and for net 0.
pub fn net_named(board: &PcbBoard, name: &str) -> Option<u32> {
    let wanted = name.trim().to_lowercase();
    board
        .nets
        .iter()
        .find(|def| def.number != 0 && def.name.to_lowercase() == wanted)
        .map(|def| def.number)
}

#[cfg(test)]
#[path = "net_report_tests.rs"]
mod tests;
