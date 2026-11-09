//! How far apart two copper shapes are (#2222): the measure behind the
//! clearance check, exact in `i128` nm like [`Shape::overlaps`].
//!
//! A disc, a track and an oval are a segment with a radius; a rectangle is
//! a convex polygon, and a round-rect is its inset rectangle with the corner
//! radius (see `shape_overlap`). The foot of a perpendicular
//! rounds to the nearest nm, so a diagonal gap is read to within 1 nm.

use crate::shape::{Shape, closest_on_segment, segments_intersect};
use crate::shape_overlap::{Core, inside_convex, separated};
use crate::units::PointNm;

/// The distance between two shapes, kept as a squared distance so a limit
/// is compared with no rounding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Separation {
    /// Squared distance between the shapes' cores (segments or polygons).
    dist_sq: i128,
    /// What the cores are swollen by: the sum of the two radii.
    reach: i64,
    /// The nearest point of each shape's core. Meaningless when the cores
    /// meet (distance 0): use [`Shape::overlaps`] and the bounds then.
    pub between: (PointNm, PointNm),
}

impl Separation {
    /// Whether the gap is smaller than `limit`; a gap of exactly `limit` is
    /// not. A limit of 0 or less is never undercut.
    pub fn is_under(&self, limit: i64) -> bool {
        if limit <= 0 {
            return false;
        }
        let reach = i128::from(limit) + i128::from(self.reach);
        self.dist_sq < reach * reach
    }

    /// The gap in whole nm, rounded down; 0 when the shapes touch or overlap.
    pub fn distance_nm(&self) -> i64 {
        let gap = (self.dist_sq as f64).sqrt() - self.reach as f64;
        gap.max(0.0).floor() as i64
    }

    /// The midpoint of [`Self::between`].
    pub fn midpoint(&self) -> PointNm {
        let (a, b) = self.between;
        PointNm::new(a.x + (b.x - a.x) / 2, a.y + (b.y - a.y) / 2)
    }
}

impl Shape {
    /// How far apart the two shapes are.
    pub fn separation(&self, other: &Shape) -> Separation {
        let (a, b) = (self.core(), other.core());
        let reach = radius(&a).max(0) + radius(&b).max(0);
        let (dist_sq, between) = core_distance(&a, &b);
        Separation {
            dist_sq,
            reach,
            between,
        }
    }
}

fn radius(core: &Core) -> i64 {
    match core {
        Core::Stadium { radius, .. } => *radius,
        Core::Polygon(_, radius) => *radius,
    }
}

/// The corners of a core and whether its last corner joins its first.
fn outline(core: &Core) -> (Vec<PointNm>, bool) {
    match core {
        Core::Stadium { a, b, .. } => (vec![*a, *b], false),
        Core::Polygon(poly, _) => (poly.clone(), true),
    }
}

fn core_distance(a: &Core, b: &Core) -> (i128, (PointNm, PointNm)) {
    let (pa, closed_a) = outline(a);
    let (pb, closed_b) = outline(b);
    let first = |p: &[PointNm]| p.first().copied().unwrap_or(PointNm::ORIGIN);
    if cores_meet(a, b) {
        return (0, (first(&pa), first(&pb)));
    }
    let from_a = nearest(&pa, &pb, closed_b, false);
    let from_b = nearest(&pb, &pa, closed_a, true);
    if from_b.0 < from_a.0 { from_b } else { from_a }
}

/// Whether the two cores share a point, touching included.
fn cores_meet(a: &Core, b: &Core) -> bool {
    match (a, b) {
        (Core::Stadium { a: p, b: q, .. }, Core::Stadium { a: r, b: s, .. }) => {
            segments_intersect(*p, *q, *r, *s)
        }
        (Core::Stadium { a: p, b: q, .. }, Core::Polygon(poly, _))
        | (Core::Polygon(poly, _), Core::Stadium { a: p, b: q, .. }) => {
            poly.len() >= 3
                && (inside_convex(poly, *p)
                    || inside_convex(poly, *q)
                    || (0..poly.len())
                        .any(|i| segments_intersect(*p, *q, poly[i], poly[(i + 1) % poly.len()])))
        }
        (Core::Polygon(p, _), Core::Polygon(q, _)) => {
            p.len() >= 3 && q.len() >= 3 && !(separated(p, q) || separated(q, p))
        }
    }
}

/// The nearest pair between a corner of `corners` and an edge of `edges`
/// (a closed ring, or a single segment): `(distance squared, (point on the
/// first shape, point on the second))`. `swap` says `corners` is the second.
fn nearest(
    corners: &[PointNm],
    edges: &[PointNm],
    closed: bool,
    swap: bool,
) -> (i128, (PointNm, PointNm)) {
    let count = if closed { edges.len() } else { 1 };
    let mut best = (i128::MAX, (PointNm::ORIGIN, PointNm::ORIGIN));
    for &corner in corners {
        for i in 0..count {
            let (e0, e1) = (edges[i], edges[(i + 1) % edges.len()]);
            let foot = closest_on_segment(e0, e1, corner);
            let d = corner.distance_sq(foot);
            if d < best.0 {
                best = (d, if swap { (foot, corner) } else { (corner, foot) });
            }
        }
    }
    best
}

#[cfg(test)]
#[path = "shape_gap_tests.rs"]
mod tests;
