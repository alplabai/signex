//! Whether two copper shapes share a point (#2192): the join test the
//! ratsnest uses, so a track across a pad or two overlapping pads count.
//!
//! A disc and a track are a segment with a radius. A rectangle is a convex
//! polygon; an ellipse is the polygon of its [`ELLIPSE_STEPS`] outline
//! points, so an ellipse is under-read by less than its sagitta.

use crate::shape::{Shape, closest_on_segment, orientation, segments_intersect, within};
use crate::units::PointNm;

/// Outline points standing in for an ellipse.
const ELLIPSE_STEPS: usize = 32;

/// A shape reduced to what the overlap test needs.
enum Core {
    /// Every point within `radius` of the segment `a`-`b`.
    Stadium { a: PointNm, b: PointNm, radius: i64 },
    /// A convex polygon, corners in order.
    Polygon(Vec<PointNm>),
}

impl Shape {
    /// Whether the two shapes share a point; touching counts.
    pub fn overlaps(&self, other: &Shape) -> bool {
        match (self.core(), other.core()) {
            (
                Core::Stadium { a, b, radius },
                Core::Stadium {
                    a: c,
                    b: d,
                    radius: s,
                },
            ) => segment_gap_within(a, b, c, d, radius.max(0) + s.max(0)),
            (Core::Stadium { a, b, radius }, Core::Polygon(poly))
            | (Core::Polygon(poly), Core::Stadium { a, b, radius }) => {
                stadium_meets_polygon(a, b, radius, &poly)
            }
            (Core::Polygon(p), Core::Polygon(q)) => !(separated(&p, &q) || separated(&q, &p)),
        }
    }

    fn core(&self) -> Core {
        match *self {
            Shape::Disc { center, radius } => Core::Stadium {
                a: center,
                b: center,
                radius,
            },
            Shape::Capsule { a, b, radius } => Core::Stadium { a, b, radius },
            Shape::Rect { .. } | Shape::Ellipse { .. } => {
                Core::Polygon(self.outline(ELLIPSE_STEPS))
            }
        }
    }
}

/// Whether segments `a`-`b` and `c`-`d` come within `limit` of each other.
fn segment_gap_within(a: PointNm, b: PointNm, c: PointNm, d: PointNm, limit: i64) -> bool {
    segments_intersect(a, b, c, d)
        || within(a.distance_sq(closest_on_segment(c, d, a)), limit)
        || within(b.distance_sq(closest_on_segment(c, d, b)), limit)
        || within(c.distance_sq(closest_on_segment(a, b, c)), limit)
        || within(d.distance_sq(closest_on_segment(a, b, d)), limit)
}

fn stadium_meets_polygon(a: PointNm, b: PointNm, radius: i64, poly: &[PointNm]) -> bool {
    if poly.len() < 3 {
        return false;
    }
    if inside_convex(poly, a) || inside_convex(poly, b) {
        return true;
    }
    let radius = radius.max(0);
    (0..poly.len()).any(|i| segment_gap_within(a, b, poly[i], poly[(i + 1) % poly.len()], radius))
}

/// Whether `p` lies in (or on) a convex polygon of either winding.
fn inside_convex(poly: &[PointNm], p: PointNm) -> bool {
    let turns = (0..poly.len()).map(|i| orientation(poly[i], poly[(i + 1) % poly.len()], p));
    let (mut left, mut right) = (false, false);
    for turn in turns {
        left |= turn > 0;
        right |= turn < 0;
    }
    !(left && right)
}

/// Whether an edge normal of `poly` separates it from `other`.
fn separated(poly: &[PointNm], other: &[PointNm]) -> bool {
    (0..poly.len()).any(|i| {
        let edge = poly[(i + 1) % poly.len()].minus(poly[i]);
        let normal = PointNm::new(-edge.y, edge.x);
        if normal == PointNm::ORIGIN {
            return false;
        }
        let span = |pts: &[PointNm]| {
            let dots = pts.iter().map(|p| p.dot(normal));
            (dots.clone().min().unwrap_or(0), dots.max().unwrap_or(0))
        };
        let (a_min, a_max) = span(poly);
        let (b_min, b_max) = span(other);
        a_max < b_min || b_max < a_min
    })
}

#[cfg(test)]
#[path = "shape_overlap_tests.rs"]
mod tests;
