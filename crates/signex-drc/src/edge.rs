//! Board-edge clearance (#2435): netted copper closer to the board outline
//! than a written rule's minimum, one finding per item at its closest
//! point. The outline is the board's saved outline; cutouts inside the
//! board are not edges here yet.

use std::collections::HashMap;

use signex_types::pcb::PcbBoard;

use pcb_geom::{ItemRef, PointNm, Shape, collect_copper};

use crate::near::{gap_under, near_pairs};
use crate::user_rules::LimitKind;
use crate::{DrcRules, Violation};

pub(crate) fn violations(board: &PcbBoard, rules: &DrcRules) -> Vec<Violation> {
    if !rules.sets(LimitKind::MinEdgeClearance) {
        return Vec::new();
    }
    let edges = outline_edges(board);
    if edges.is_empty() {
        return Vec::new();
    }
    let copper = collect_copper(board);
    let mut shapes: Vec<Shape> = edges.clone();
    shapes.extend(copper.iter().map(|c| c.shape.clone()));
    let limits: Vec<_> = copper
        .iter()
        .map(|c| rules.object_limit(LimitKind::MinEdgeClearance, c.net, c.layers))
        .collect();
    let reach = rules.widest_limit(LimitKind::MinEdgeClearance);
    let n = edges.len();
    let pairs = near_pairs(&shapes, reach, |i, j| i < n && j >= n);
    // The closest edge of each item (an arc's chords are one item).
    let mut worst: HashMap<ItemRef, Violation> = HashMap::new();
    for (edge, slot) in pairs {
        let item = &copper[slot - n];
        let Some((limit, rule)) = &limits[slot - n] else {
            continue;
        };
        let Some((gap, at)) = gap_under(&shapes[edge], &item.shape, *limit) else {
            continue;
        };
        let held = worst.get(&item.item);
        if held.is_some_and(|v| v.measured_nm <= gap) {
            continue;
        }
        worst.insert(
            item.item,
            Violation {
                kind: LimitKind::MinEdgeClearance.violation(),
                items: vec![item.item],
                nets: (item.net, item.net),
                layers: item.layers,
                at,
                measured_nm: gap,
                required_nm: *limit,
                keepout: None,
                rule: Some(rule.clone()),
            },
        );
    }
    worst.into_values().collect()
}

/// The outline's sides as zero-width capsules, closed.
fn outline_edges(board: &PcbBoard) -> Vec<Shape> {
    let points: Vec<PointNm> = board
        .outline
        .iter()
        .filter_map(|&p| PointNm::from_mm(p, "board outline").ok())
        .collect();
    if points.len() < 2 {
        return Vec::new();
    }
    let mut edges: Vec<Shape> = points
        .windows(2)
        .map(|w| Shape::Capsule {
            a: w[0],
            b: w[1],
            radius: 0,
        })
        .collect();
    if points.len() > 2 {
        edges.push(Shape::Capsule {
            a: points[points.len() - 1],
            b: points[0],
            radius: 0,
        });
    }
    edges
}
