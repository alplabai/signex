//! The three checks. Candidate pairs come from the board index, one
//! bounding-box query per item, so the cost follows what is near each item,
//! not the square of the item count.

use std::collections::{HashMap, HashSet};

use signex_types::pcb::PcbBoard;

use crate::copper::{CopperItem, collect_copper};
use crate::index::{BoardIndex, ItemRef};
use crate::ratsnest::ratsnest;
use crate::units::PointNm;

use super::{DrcReport, DrcRules, Violation, ViolationKind};

/// Check `board` against `rules`.
pub fn run(board: &PcbBoard, rules: &DrcRules) -> DrcReport {
    let items = collect_copper(board);
    let index = BoardIndex::build(board);
    let mut violations = copper_pairs(&items, &index, rules);
    violations.extend(unconnected(board, &items));
    violations.sort_unstable();
    DrcReport {
        rules: rules.clone(),
        violations,
        skipped: index.rejected().to_vec(),
    }
}

/// Shorts and clearance violations between copper of different nets.
fn copper_pairs(items: &[CopperItem], index: &BoardIndex, rules: &DrcRules) -> Vec<Violation> {
    // The first copy of a repeated identity is the one the index holds.
    let mut slot: HashMap<ItemRef, usize> = HashMap::new();
    for (i, c) in items.iter().enumerate() {
        slot.entry(c.item).or_insert(i);
    }
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    let mut found = Vec::new();
    for (i, a) in items.iter().enumerate() {
        let near = a.shape.aabb().inflate(rules.reach_of(a.net));
        for hit in index.bounds_meeting(&near) {
            let Some(&j) = slot.get(&hit.item).filter(|&&j| j != i) else {
                continue;
            };
            // A pair is looked at once, from whichever side finds it first.
            if !seen.insert((i.min(j), i.max(j))) {
                continue;
            }
            if let Some(v) = check_pair(a, &items[j], rules) {
                found.push(v);
            }
        }
    }
    found
}

fn check_pair(a: &CopperItem, b: &CopperItem, rules: &DrcRules) -> Option<Violation> {
    let shared = a.layers & b.layers;
    if a.net == b.net || shared == 0 {
        return None;
    }
    let required = rules.required(a.net, b.net);
    let ((first, second), nets) = if a.item <= b.item {
        ((a, b), (a.net, b.net))
    } else {
        ((b, a), (b.net, a.net))
    };
    let items = vec![first.item, second.item];
    if a.shape.overlaps(&b.shape) {
        return Some(Violation {
            kind: ViolationKind::Short,
            items,
            nets,
            layers: shared,
            at: shared_point(a, b),
            measured_nm: 0,
            required_nm: required,
        });
    }
    let gap = a.shape.separation(&b.shape);
    gap.is_under(required).then(|| Violation {
        kind: ViolationKind::Clearance,
        items,
        nets,
        layers: shared,
        at: gap.midpoint(),
        // Rounding down can only land under the rule here, never on it.
        measured_nm: gap.distance_nm().min(required - 1),
        required_nm: required,
    })
}

/// A point on both coppers where they meet: the first of a few candidates
/// (the centre of where the bounding boxes meet, then each shape's own
/// centre) that lies on both. A diagonal track across a pad corner can have
/// the first fall on neither; with none on both, the first is used.
fn shared_point(a: &CopperItem, b: &CopperItem) -> PointNm {
    let (ra, rb) = (a.shape.aabb(), b.shape.aabb());
    let lo = PointNm::new(ra.min.x.max(rb.min.x), ra.min.y.max(rb.min.y));
    let hi = PointNm::new(ra.max.x.min(rb.max.x), ra.max.y.min(rb.max.y));
    let meet = mid(lo, hi);
    [meet, mid(ra.min, ra.max), mid(rb.min, rb.max)]
        .into_iter()
        .find(|&p| a.shape.contains(p, 0) && b.shape.contains(p, 0))
        .unwrap_or(meet)
}

fn mid(a: PointNm, b: PointNm) -> PointNm {
    PointNm::new(a.x + (b.x - a.x) / 2, a.y + (b.y - a.y) / 2)
}

/// One violation per link the ratsnest still draws.
fn unconnected(board: &PcbBoard, items: &[CopperItem]) -> Vec<Violation> {
    let mut pads: HashMap<(u32, PointNm), ItemRef> = HashMap::new();
    for pad in items.iter().filter(|c| c.pad_center.is_some()) {
        if let Some(center) = pad.pad_center {
            pads.entry((pad.net, center))
                .and_modify(|held| *held = (*held).min(pad.item))
                .or_insert(pad.item);
        }
    }
    ratsnest(board)
        .into_iter()
        .map(|line| {
            let mut items: Vec<ItemRef> = [line.from, line.to]
                .iter()
                .filter_map(|&end| pads.get(&(line.net, end)).copied())
                .collect();
            items.sort_unstable();
            items.dedup();
            let length = (line.from.distance_sq(line.to) as f64).sqrt().floor() as i64;
            Violation {
                kind: ViolationKind::Unconnected,
                items,
                nets: (line.net, line.net),
                layers: 0,
                at: PointNm::new(
                    line.from.x + (line.to.x - line.from.x) / 2,
                    line.from.y + (line.to.y - line.from.y) / 2,
                ),
                measured_nm: length,
                required_nm: 0,
            }
        })
        .collect()
}
