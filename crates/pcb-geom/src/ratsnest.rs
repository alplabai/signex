//! The ratsnest: the shortest unrouted connections that would join a
//! net's pads (#2192).
//!
//! Copper that already touches (a track across a pad, a via on a track,
//! two tracks crossing, anything on a same-net zone's outline, zones
//! overlapping each other)
//! forms an island. Per net, the islands that hold pads are joined by a
//! minimum spanning forest. The answer is a pure function of the board and
//! is the same on every run: integer nm throughout, sorted output.

use std::collections::{BTreeMap, HashMap};

use rstar::{AABB, RTree, RTreeObject};
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{Footprint, Pad, PadType, PcbBoard, ViaType};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};

use crate::index::{BoardIndex, ItemRef, RectSelect, track_shape, via_shape};
use crate::mst::{Edge, euclidean_mst, spanning_forest};
use crate::rect::RectNm;
use crate::shape::Shape;
use crate::shape_overlap::polygons_meet;
use crate::transform::{Side, pad_shape, pad_world_center};
use crate::union_find::UnionFind;
use crate::units::PointNm;

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

/// A copper item of a numbered net, with where it touches other copper.
struct Node {
    item: ItemRef,
    net: u32,
    /// Bit `n` set: the copper layer with [`layer_bit`] `n`.
    layers: u32,
    /// The item's copper; it joins any same-net copper it overlaps.
    shape: Shape,
    /// Set for a pad: its centre, where a ratsnest line ends.
    pad_center: Option<PointNm>,
}

/// The ratsnest of `board`, sorted by `(net, from, to)`. Items whose
/// geometry does not convert are left out (see [`BoardIndex::rejected`]);
/// net 0 is no net.
pub fn ratsnest(board: &PcbBoard) -> Vec<RatLine> {
    let nodes = collect_nodes(board);
    if nodes.is_empty() {
        return Vec::new();
    }
    let index = BoardIndex::build(board);
    let slot: HashMap<ItemRef, usize> =
        nodes.iter().enumerate().map(|(i, n)| (n.item, i)).collect();
    let zones = collect_zones(board);
    let mut islands = UnionFind::new(nodes.len() + zones.len());
    for (i, node) in nodes.iter().enumerate() {
        for hit in index.hit_rect(&node.shape.aabb(), RectSelect::Touching) {
            let Some(&j) = slot.get(&hit) else { continue };
            let other = &nodes[j];
            if other.net == node.net
                && other.layers & node.layers != 0
                && node.shape.overlaps(&other.shape)
            {
                islands.union(i, j);
            }
        }
    }
    // A zone joins every same-net item it overlaps on a shared layer; the
    // zones are nodes after the items and hold no pad.
    for (z, zone) in zones.iter().enumerate() {
        let rect = RectNm::bounding(zone.outline.iter().copied());
        let hits = rect.map(|r| index.hit_rect(&r, RectSelect::Touching));
        for hit in hits.into_iter().flatten() {
            let Some(&j) = slot.get(&hit) else { continue };
            let other = &nodes[j];
            if other.net == zone.net
                && other.layers & zone.layers != 0
                && other.shape.overlaps_polygon(&zone.outline)
            {
                islands.union(nodes.len() + z, j);
            }
        }
    }
    // Same-net zones on a shared layer whose outlines meet are one copper.
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
                && polygons_meet(&zone.outline, &peer.outline)
            {
                islands.union(nodes.len() + entry.zone, nodes.len() + other.zone);
            }
        }
    }
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
        let r = RectNm::bounding(copper.outline.iter().copied())?;
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

/// A zone's copper: its OUTLINE polygon, since the model stores no filled
/// polygon (the fill is not computed or saved). Holes and the clearance
/// the fill keeps from other nets are ignored, so a pad in a hole or a
/// pad the fill would isolate still counts as joined.
struct ZoneCopper {
    net: u32,
    layers: u32,
    outline: Vec<PointNm>,
}

fn collect_zones(board: &PcbBoard) -> Vec<ZoneCopper> {
    board
        .zones
        .iter()
        .filter_map(|zone| {
            let layers = layer_mask(std::iter::once(zone.layer.as_str()));
            let outline: Result<Vec<PointNm>, _> = zone
                .outline
                .iter()
                .map(|&p| PointNm::from_mm(p, "zone.outline"))
                .collect();
            match outline {
                Ok(outline) if zone.net != 0 && layers != 0 && outline.len() >= 3 => {
                    Some(ZoneCopper {
                        net: zone.net,
                        layers,
                        outline,
                    })
                }
                _ => None,
            }
        })
        .collect()
}

fn collect_nodes(board: &PcbBoard) -> Vec<Node> {
    let mut nodes = Vec::new();
    for footprint in &board.footprints {
        for pad in &footprint.pads {
            let Some(net) = pad.net.as_ref().map(|n| n.number).filter(|&n| n != 0) else {
                continue;
            };
            let (Ok(center), Ok(shape)) =
                (pad_world_center(footprint, pad), pad_shape(footprint, pad))
            else {
                continue;
            };
            nodes.push(Node {
                item: ItemRef::Pad {
                    footprint: footprint.uuid,
                    pad: pad.uuid,
                },
                net,
                layers: pad_layers(footprint, pad),
                shape,
                pad_center: Some(center),
            });
        }
    }
    for segment in &board.segments {
        let layers = layer_mask(std::iter::once(segment.layer.as_str()));
        if segment.net == 0 || layers == 0 {
            continue;
        }
        if let Ok(shape) = track_shape(segment) {
            nodes.push(Node {
                item: ItemRef::Track(segment.uuid),
                net: segment.net,
                layers,
                shape,
                pad_center: None,
            });
        }
    }
    for via in &board.vias {
        if via.net == 0 {
            continue;
        }
        let layers = match via.via_type {
            ViaType::Through => u32::MAX,
            ViaType::Blind | ViaType::Micro => span_mask(via.layers.iter().map(String::as_str)),
        };
        if let (Ok(shape), true) = (via_shape(via), layers != 0) {
            nodes.push(Node {
                item: ItemRef::Via(via.uuid),
                net: via.net,
                layers,
                shape,
                pad_center: None,
            });
        }
    }
    nodes
}

/// A through-hole pad reaches every copper layer; a surface pad the copper
/// layers it lists, or its footprint's side when it lists none.
fn pad_layers(footprint: &Footprint, pad: &Pad) -> u32 {
    if pad.pad_type == PadType::Thru {
        return u32::MAX;
    }
    match layer_mask(pad.layers.iter().map(String::as_str)) {
        0 => match Side::of_layer(&footprint.layer) {
            Side::Top => 1 << layer_bit(CopperLayer::Top),
            Side::Bottom => 1 << layer_bit(CopperLayer::Bottom),
        },
        mask => mask,
    }
}

/// Bit position of a copper layer: top 0, inner `n` at `n`, bottom 31.
fn layer_bit(layer: CopperLayer) -> u32 {
    match layer {
        CopperLayer::Top => 0,
        CopperLayer::Inner(inner) => u32::from(inner.index()),
        CopperLayer::Bottom => 31,
    }
}

fn copper_bits<'a>(names: impl Iterator<Item = &'a str>) -> impl Iterator<Item = u32> {
    names
        .filter_map(parse_pcb_layer)
        .filter_map(PcbLayer::copper)
        .map(layer_bit)
}

fn layer_mask<'a>(names: impl Iterator<Item = &'a str>) -> u32 {
    copper_bits(names).fold(0, |mask, bit| mask | 1 << bit)
}

/// Every copper layer from the topmost to the bottommost one named.
fn span_mask<'a>(names: impl Iterator<Item = &'a str>) -> u32 {
    let bits: Vec<u32> = copper_bits(names).collect();
    match (bits.iter().min(), bits.iter().max()) {
        (Some(&lo), Some(&hi)) => (lo..=hi).fold(0, |mask, bit| mask | 1 << bit),
        _ => 0,
    }
}

#[cfg(test)]
#[path = "ratsnest_tests.rs"]
mod tests;
