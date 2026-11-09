//! The three checks. Candidate pairs come from the board index, one
//! bounding-box query per item, so the cost follows what is near each item,
//! not the square of the item count.

use std::collections::HashMap;

use signex_types::pcb::PcbBoard;

use crate::copper::{CopperItem, collect_copper};
use crate::index::{BoardIndex, ItemRef};
use crate::ratsnest::ratsnest;
use crate::rect::RectNm;
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
    let slot: HashMap<ItemRef, usize> =
        items.iter().enumerate().map(|(i, c)| (c.item, i)).collect();
    let reach = rules.largest();
    let mut found = Vec::new();
    for (i, a) in items.iter().enumerate() {
        let near = a.shape.aabb().inflate(reach);
        for hit in index.bounds_meeting(&near) {
            // Each pair once, from its lower slot.
            let Some(&j) = slot.get(&hit.item).filter(|&&j| j > i) else {
                continue;
            };
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
            at: overlap_center(&a.shape.aabb(), &b.shape.aabb()),
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

/// The centre of where two bounding boxes meet.
fn overlap_center(a: &RectNm, b: &RectNm) -> PointNm {
    let lo = PointNm::new(a.min.x.max(b.min.x), a.min.y.max(b.min.y));
    let hi = PointNm::new(a.max.x.min(b.max.x), a.max.y.min(b.max.y));
    PointNm::new(lo.x + (hi.x - lo.x) / 2, lo.y + (hi.y - lo.y) / 2)
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
