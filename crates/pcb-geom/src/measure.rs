//! Measuring on a board (#2403): the straight distance between two points,
//! and the gap between the copper or bodies of two items, edge to edge.
//! Both answer in mm with the two points the gap runs between, so a canvas
//! can draw the ruler and an agent can read the numbers.

use signex_types::pcb::{PcbBoard, Point};

use crate::index::{BoardIndex, ItemRef};
use crate::shape::Shape;
use crate::units::PointNm;

/// A measured span: where it runs and how long it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub from: Point,
    pub to: Point,
    /// The straight length in mm.
    pub distance_mm: f64,
    /// `to.x - from.x` in mm.
    pub dx_mm: f64,
    /// `to.y - from.y` in mm.
    pub dy_mm: f64,
}

impl Span {
    /// The span from `from` to `to`.
    pub fn between(from: Point, to: Point) -> Self {
        let (dx, dy) = (to.x - from.x, to.y - from.y);
        Span {
            from,
            to,
            distance_mm: dx.hypot(dy),
            dx_mm: dx,
            dy_mm: dy,
        }
    }
}

/// How far the outline of a shape sits from its core (segment or polygon).
fn reach(shape: &Shape) -> i64 {
    match *shape {
        Shape::Disc { radius, .. }
        | Shape::Capsule { radius, .. }
        | Shape::RoundRect { radius, .. } => radius.max(0),
        Shape::Rect { .. } => 0,
    }
}

/// `p` moved `by` nm towards `toward`.
fn step_towards(p: PointNm, toward: PointNm, by: i64) -> PointNm {
    let length = (p.distance_sq(toward) as f64).sqrt();
    if length <= 0.0 || by == 0 {
        return p;
    }
    let t = by as f64 / length;
    PointNm::new(
        p.x + ((toward.x - p.x) as f64 * t).round() as i64,
        p.y + ((toward.y - p.y) as f64 * t).round() as i64,
    )
}

/// The gap between two shapes and the outline points it runs between; a
/// zero-length span at the midpoint when they touch or overlap.
fn shape_gap(a: &Shape, b: &Shape) -> (i64, PointNm, PointNm) {
    let separation = a.separation(b);
    let gap = separation.distance_nm();
    if gap == 0 {
        let mid = separation.midpoint();
        return (0, mid, mid);
    }
    let (core_a, core_b) = separation.between;
    (
        gap,
        step_towards(core_a, core_b, reach(a)),
        step_towards(core_b, core_a, reach(b)),
    )
}

/// The edge-to-edge gap between items `a` and `b` of `board`: the closest
/// approach of their copper (a pad, track or via), body (a footprint) or
/// outline edges (a drawn area). `None` when either is not on the board or
/// its geometry cannot be held.
pub fn edge_gap(board: &PcbBoard, a: ItemRef, b: ItemRef) -> Option<Span> {
    let index = BoardIndex::build_selectable(board);
    let shapes_of = |item: ItemRef| -> Vec<Shape> {
        index
            .items()
            .filter(|indexed| indexed.item == item)
            .map(|indexed| indexed.shape)
            .collect()
    };
    let (shapes_a, shapes_b) = (shapes_of(a), shapes_of(b));
    let closest = shapes_a
        .iter()
        .flat_map(|sa| shapes_b.iter().map(move |sb| shape_gap(sa, sb)))
        .min_by_key(|(gap, ..)| *gap)?;
    let (_, from, to) = closest;
    Some(Span::between(from.to_mm(), to.to_mm()))
}

#[cfg(test)]
#[path = "measure_tests.rs"]
mod tests;
