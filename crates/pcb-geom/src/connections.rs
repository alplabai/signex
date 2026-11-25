//! How much of a board is routed (#2460): the connections its nets need,
//! how many are still ratsnest lines, and which pads each unrouted net
//! still has to join.
//!
//! A net needs one connection less than it has pad islands, counting only
//! pads: pads whose copper overlaps on a shared layer are one island. The
//! unrouted connections are the ratsnest lines, which already join copper
//! through tracks, vias and pour fills, so `routed = total - unrouted` never
//! goes below zero. A connection a same-net pour will make once it reaches
//! the pads is still unrouted; the net says so with `pour_will_connect`.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use crate::copper::collect_copper;
use crate::index::{BoardIndex, ItemRef, RectSelect};
use crate::ratsnest::RatLine;
use crate::union_find::UnionFind;
use crate::units::PointNm;

/// A pad as a person names it: its part's designator and its number. A pad
/// with no part has an empty designator.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct PadName {
    pub designator: String,
    pub pad: String,
}

/// What one net still lacks.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NetUnrouted {
    /// Its ratsnest lines, in ratsnest order.
    pub lines: Vec<RatLine>,
    /// The pads each line joins, in the same order.
    pub pads: Vec<(PadName, PadName)>,
    /// A pour of the same net is on the board: once it reaches these pads
    /// it makes the connection.
    pub pour_will_connect: bool,
}

/// The connections of a board: how many its nets need, how many are still
/// unrouted, and the unrouted ones by net number.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConnectionCounts {
    pub total: usize,
    pub unrouted: usize,
    /// Only the nets with an unrouted connection.
    pub per_net: BTreeMap<u32, NetUnrouted>,
}

impl ConnectionCounts {
    /// The connections already made.
    pub fn routed(&self) -> usize {
        self.total.saturating_sub(self.unrouted)
    }

    /// The nets with an unrouted connection, the most unrouted first, then
    /// by net number.
    pub fn nets_by_unrouted(&self) -> Vec<u32> {
        let mut nets: Vec<(usize, u32)> = self
            .per_net
            .iter()
            .map(|(net, unrouted)| (unrouted.lines.len(), *net))
            .collect();
        nets.sort_by_key(|&(lines, net)| (std::cmp::Reverse(lines), net));
        nets.into_iter().map(|(_, net)| net).collect()
    }
}

/// The connection counts of `board`, whose ratsnest is `lines` (from
/// [`crate::ratsnest_with_fills`] of this board).
pub fn connection_counts(board: &PcbBoard, lines: &[RatLine]) -> ConnectionCounts {
    let names = pad_names(board);
    let pads: Vec<_> = collect_copper(board)
        .into_iter()
        .filter(|node| node.pad_center.is_some())
        .collect();
    let slot: HashMap<ItemRef, usize> = pads.iter().enumerate().map(|(i, n)| (n.item, i)).collect();
    let index = BoardIndex::build(board);
    let mut islands = UnionFind::new(pads.len());
    for (i, node) in pads.iter().enumerate() {
        for hit in index.hit_rect(&node.shape.aabb(), RectSelect::Touching) {
            let Some(&j) = slot.get(&hit) else { continue };
            let other = &pads[j];
            if other.net == node.net
                && other.layers & node.layers != 0
                && node.shape.overlaps(&other.shape)
            {
                islands.union(i, j);
            }
        }
    }
    let mut roots: BTreeMap<u32, BTreeSet<usize>> = BTreeMap::new();
    for (i, pad) in pads.iter().enumerate() {
        roots.entry(pad.net).or_default().insert(islands.find(i));
    }
    let total = roots.values().map(|r| r.len().saturating_sub(1)).sum();
    // A pad centre names the pad a ratsnest line ends on.
    let at: HashMap<(u32, PointNm), &PadName> = pads
        .iter()
        .filter_map(|n| {
            let name = names.get(&pad_id(n.item)?)?;
            Some(((n.net, n.pad_center?), name))
        })
        .collect();
    let name_at = |net: u32, p: PointNm| {
        at.get(&(net, p))
            .map(|name| (*name).clone())
            .unwrap_or_default()
    };
    let mut per_net: BTreeMap<u32, NetUnrouted> = BTreeMap::new();
    for line in lines {
        let net = per_net.entry(line.net).or_default();
        net.lines.push(*line);
        net.pads
            .push((name_at(line.net, line.from), name_at(line.net, line.to)));
    }
    // A shelved or unfilled pour holds no copper, so it will connect nothing.
    for zone in board.zones.iter().filter(|z| z.is_filling_pour()) {
        if let Some(net) = per_net.get_mut(&zone.net) {
            net.pour_will_connect = true;
        }
    }
    ConnectionCounts {
        total,
        unrouted: lines.len(),
        per_net,
    }
}

/// The pad an item is, if it is one.
fn pad_id(item: ItemRef) -> Option<Uuid> {
    match item {
        ItemRef::Pad { pad, .. } | ItemRef::FreePad(pad) => Some(pad),
        _ => None,
    }
}

/// Every pad of `board` by its id, with the name a person reads.
fn pad_names(board: &PcbBoard) -> HashMap<Uuid, PadName> {
    let parts = board.footprints.iter().flat_map(|fp| {
        fp.pads.iter().map(|pad| {
            let name = PadName {
                designator: fp.reference.clone(),
                pad: pad.number.clone(),
            };
            (pad.uuid, name)
        })
    });
    let free = board.free.pads.iter().map(|pad| {
        let name = PadName {
            designator: String::new(),
            pad: pad.number.clone(),
        };
        (pad.uuid, name)
    });
    parts.chain(free).collect()
}

#[cfg(test)]
#[path = "connections_tests.rs"]
mod tests;
