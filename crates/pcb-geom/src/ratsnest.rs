//! The ratsnest: the shortest unrouted connections that would join a
//! net's pads (#2192).
//!
//! Copper that already touches (a track across a pad, a via on a track,
//! two tracks crossing, anything on a same-net zone's copper, zones
//! overlapping each other) forms an island. A pour's copper is its fill,
//! with its holes and the clearance from other nets cut out (#2216).
//! Per net, the islands that hold pads are joined by a
//! minimum spanning forest. The answer is a pure function of the board and
//! is the same on every run: integer nm throughout, sorted output.

use std::collections::{BTreeMap, HashMap};

use rstar::{AABB, RTree, RTreeObject};
use signex_types::pcb::PcbBoard;

use crate::area_touch::{areas_meet, shape_meets_area};
use crate::copper::{CopperItem, collect_copper, layer_mask};
use crate::index::{BoardIndex, ItemRef, RectSelect};
use crate::mst::{Edge, euclidean_mst, spanning_forest};
use crate::region::Polygon;
use crate::union_find::UnionFind;
use crate::units::PointNm;
use crate::zone_fill::{ZoneFills, zone_area, zone_fills};

/// Up to this many pad pairs across a net's islands, they are joined at
/// their closest pads. Beyond it each island is reduced to one
/// representative pad. Counted in pairs, not pads: a big mostly-routed net
/// with a stray pad is few pairs and stays exact.
const EXACT_PAIR_LIMIT: usize = 1 << 22;

/// One unrouted connection of net `net`, `from <= to` in `(x, y)` order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RatLine {
    pub net: u32,
    pub from: PointNm,
    pub to: PointNm,
}

/// The ratsnest of `board`, sorted by `(net, from, to)`. Items whose
/// geometry does not convert are left out (see [`BoardIndex::rejected`]);
/// net 0 is no net. Computes the board's pour fills; a caller that holds
/// them already passes them to [`ratsnest_with_fills`].
pub fn ratsnest(board: &PcbBoard) -> Vec<RatLine> {
    ratsnest_with_fills(board, &zone_fills(board))
}

/// [`ratsnest`] with the pours' copper taken from `fills`, which must be
/// [`zone_fills`] of this same board.
pub fn ratsnest_with_fills(board: &PcbBoard, fills: &ZoneFills) -> Vec<RatLine> {
    let Some((nodes, mut islands)) = copper_islands(board, fills) else {
        return Vec::new();
    };
    // Per net: island root -> the pad centres of that island.
    let mut nets: BTreeMap<u32, BTreeMap<usize, Vec<PointNm>>> = BTreeMap::new();
    for (i, node) in nodes.iter().enumerate() {
        if let Some(center) = node.pad_center {
            let root = islands.find(i);
            nets.entry(node.net)
                .or_default()
                .entry(root)
                .or_default()
                .push(center);
        }
    }
    let mut lines = Vec::new();
    for (net, by_root) in nets {
        let groups: Vec<Vec<PointNm>> = by_root.into_values().collect();
        for (a, b) in join_islands(&groups) {
            lines.push(RatLine {
                net,
                from: a.min(b),
                to: a.max(b),
            });
        }
    }
    lines.sort_unstable();
    lines
}

/// The board's copper items and which of them touch: one union-find set per
/// island (the zone pieces are sets after the items). `None` for a board
/// with no copper.
pub(crate) fn copper_islands(
    board: &PcbBoard,
    fills: &ZoneFills,
) -> Option<(Vec<CopperItem>, UnionFind)> {
    let nodes = collect_copper(board);
    if nodes.is_empty() {
        return None;
    }
    let index = BoardIndex::build(board);
    // Every node of an identity: an arc's chords share one (#2373).
    let mut slots: HashMap<ItemRef, Vec<usize>> = HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        slots.entry(n.item).or_default().push(i);
    }
    let zones = collect_zones(board, fills);
    let mut islands = UnionFind::new(nodes.len() + zones.len());
    // The parts of one item are one copper.
    for (item, parts) in &slots {
        if item.is_multi_part() {
            for pair in parts.windows(2) {
                islands.union(pair[0], pair[1]);
            }
        }
    }
    let copies = |hit: &ItemRef| slots.get(hit).into_iter().flatten().copied();
    for (i, node) in nodes.iter().enumerate() {
        for hit in index.hit_rect(&node.shape.aabb(), RectSelect::Touching) {
            for j in copies(&hit) {
                let other = &nodes[j];
                if other.net == node.net
                    && other.layers & node.layers != 0
                    && node.shape.overlaps(&other.shape)
                {
                    islands.union(i, j);
                }
            }
        }
    }
    // A zone's copper joins every same-net item it overlaps on a shared
    // layer; the zone pieces are nodes after the items and hold no pad.
    for (z, zone) in zones.iter().enumerate() {
        let hits = zone
            .area
            .bounds()
            .map(|r| index.hit_rect(&r, RectSelect::Touching));
        for hit in hits.into_iter().flatten() {
            for j in copies(&hit) {
                let other = &nodes[j];
                if other.net == zone.net
                    && other.layers & zone.layers != 0
                    && shape_meets_area(&other.shape, &zone.area)
                {
                    islands.union(nodes.len() + z, j);
                }
            }
        }
    }
    // Same-net zone copper on a shared layer that meets is one copper.
    let tree = RTree::bulk_load(
        zones
            .iter()
            .enumerate()
            .filter_map(|(z, zone)| ZoneBox::new(z, zone))
            .collect(),
    );
    for entry in tree.iter() {
        let zone = &zones[entry.zone];
        for other in tree.locate_in_envelope_intersecting(&entry.envelope) {
            let peer = &zones[other.zone];
            if other.zone > entry.zone
                && peer.net == zone.net
                && peer.layers & zone.layers != 0
                && areas_meet(&zone.area, &peer.area)
            {
                islands.union(nodes.len() + entry.zone, nodes.len() + other.zone);
            }
        }
    }
    Some((nodes, islands))
}

/// The pad-to-pad links that join `groups` (one list of pad centres per
/// island) into a single tree.
fn join_islands(groups: &[Vec<PointNm>]) -> Vec<(PointNm, PointNm)> {
    if groups.len() < 2 {
        return Vec::new();
    }
    if cross_island_pairs(groups) > EXACT_PAIR_LIMIT {
        let reps: Vec<PointNm> = groups.iter().map(|g| representative(g)).collect();
        return euclidean_mst(&reps)
            .into_iter()
            .map(|e| (reps[e.a], reps[e.b]))
            .collect();
    }
    let mut edges = Vec::new();
    let mut ends: HashMap<(usize, usize), (PointNm, PointNm)> = HashMap::new();
    for i in 0..groups.len() {
        for j in i + 1..groups.len() {
            let best = groups[i]
                .iter()
                .flat_map(|&a| groups[j].iter().map(move |&b| (a.distance_sq(b), a, b)))
                .min();
            if let Some((weight, a, b)) = best {
                edges.push(Edge { a: i, b: j, weight });
                ends.insert((i, j), (a, b));
            }
        }
    }
    spanning_forest(groups.len(), &edges)
        .into_iter()
        .filter_map(|e| ends.get(&(e.a, e.b)).copied())
        .collect()
}

/// How many pad pairs lie in different islands: `(P^2 - sum n_i^2) / 2`.
fn cross_island_pairs(groups: &[Vec<PointNm>]) -> usize {
    let pads: usize = groups.iter().map(Vec::len).sum();
    let same: usize = groups.iter().map(|g| g.len() * g.len()).sum();
    pads.saturating_mul(pads).saturating_sub(same) / 2
}

/// The pad nearest the island's centroid, ties to the lower `(x, y)`.
fn representative(pads: &[PointNm]) -> PointNm {
    let n = pads.len() as i128;
    let sum_x: i128 = pads.iter().map(|p| i128::from(p.x)).sum();
    let sum_y: i128 = pads.iter().map(|p| i128::from(p.y)).sum();
    // Compare n*p with the sums, so no division rounds.
    let off = |p: &PointNm| {
        let dx = i128::from(p.x) * n - sum_x;
        let dy = i128::from(p.y) * n - sum_y;
        dx * dx + dy * dy
    };
    pads.iter()
        .copied()
        .min_by_key(|p| (off(p), *p))
        .unwrap_or(PointNm::ORIGIN)
}

/// A zone's bounding box in the R-tree that pairs overlapping zones.
struct ZoneBox {
    zone: usize,
    envelope: AABB<[i128; 2]>,
}

impl ZoneBox {
    fn new(zone: usize, copper: &ZoneCopper) -> Option<Self> {
        let r = copper.area.bounds()?;
        Some(ZoneBox {
            zone,
            envelope: AABB::from_corners(
                [i128::from(r.min.x), i128::from(r.min.y)],
                [i128::from(r.max.x), i128::from(r.max.y)],
            ),
        })
    }
}

impl RTreeObject for ZoneBox {
    type Envelope = AABB<[i128; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

/// A zone's copper on its one layer: for a pour, one piece of its fill
/// (holes and the clearance from other nets cut out); for any other zone,
/// its outline less its usable holes.
struct ZoneCopper {
    net: u32,
    layers: u32,
    area: Polygon,
}

fn collect_zones(board: &PcbBoard, fills: &ZoneFills) -> Vec<ZoneCopper> {
    let mut out = Vec::new();
    for (index, zone) in board.zones.iter().enumerate() {
        let layers = layer_mask(std::iter::once(zone.layer.as_str()));
        if zone.net == 0 || layers == 0 {
            continue;
        }
        if zone.is_pour() {
            let pieces = fills
                .of(index)
                .map(|f| f.pieces.as_slice())
                .unwrap_or_default();
            out.extend(pieces.iter().map(|area| ZoneCopper {
                net: zone.net,
                layers,
                area: area.clone(),
            }));
        } else if let Some(area) = zone_area(zone).filter(|a| !a.outer.is_empty()) {
            out.push(ZoneCopper {
                net: zone.net,
                layers,
                area,
            });
        }
    }
    out
}

#[cfg(test)]
#[path = "ratsnest_tests.rs"]
mod tests;
