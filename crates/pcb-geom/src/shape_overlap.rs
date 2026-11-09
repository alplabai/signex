//! Whether two copper shapes share a point (#2192): the join test the
//! ratsnest uses, so a track across a pad or two overlapping pads count.
//!
//! A disc and a track are a segment with a radius. A rectangle is a convex
//! polygon; a rounded rectangle is the polygon of its corner arcs
//! ([`ROUND_STEPS`] points each), under-read by less than the sagitta.

use crate::shape::{
    ROUND_STEPS, Shape, closest_on_segment, orientation, segments_intersect, within,
};
use crate::units::PointNm;

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

    /// Whether the shape shares a point with the simple polygon `poly`
    /// (corners in order, either winding, convex or not); touching counts.
    /// The shape is a stadium or a convex polygon, so it meets `poly`
    /// exactly when an edge of `poly` reaches it, or it lies wholly inside
    /// `poly`, or `poly` lies wholly inside it. Integer nm; no floats
    /// beyond `outline`'s corner arcs.
    ///
    /// A `poly` with no area (fewer than 3 distinct corners, or all
    /// collinear) is a point or a zero-width segment, like a flat pad: it
    /// meets only what touches that segment, never the plane around it. An
    /// empty `poly` meets nothing.
    pub fn overlaps_polygon(&self, poly: &[PointNm]) -> bool {
        if poly.is_empty() {
            return false;
        }
        if let Some((a, b)) = flat_segment(poly) {
            return self.overlaps(&Shape::Capsule { a, b, radius: 0 });
        }
        let edges = (0..poly.len()).map(|i| (poly[i], poly[(i + 1) % poly.len()]));
        match self.core() {
            Core::Stadium { a, b, radius } => {
                point_in_polygon(poly, a)
                    || edges
                        .into_iter()
                        .any(|(c, d)| segment_gap_within(a, b, c, d, radius.max(0)))
            }
            Core::Polygon(own) => {
                own.len() >= 3
                    && (own.iter().any(|&p| point_in_polygon(poly, p))
                        || inside_convex(&own, poly[0])
                        || edges.into_iter().any(|(c, d)| {
                            (0..own.len())
                                .any(|i| segments_intersect(own[i], own[(i + 1) % own.len()], c, d))
                        }))
            }
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
            Shape::Rect { .. } | Shape::RoundRect { .. } => {
                flat_to_stadium(self.outline(ROUND_STEPS))
            }
        }
    }
}

/// A polygon with no area (a zero-size or zero-width pad) is a point or a
/// segment: the convex tests would read it as containing every point on its
/// line, so it becomes the segment between its two farthest corners.
fn flat_to_stadium(poly: Vec<PointNm>) -> Core {
    let Some(&first) = poly.first() else {
        return Core::Polygon(poly);
    };
    let far = |from: PointNm| {
        poly.iter()
            .copied()
            .max_by_key(|p| p.distance_sq(from))
            .unwrap_or(from)
    };
    let a = far(first);
    let b = far(a);
    if poly.iter().all(|&p| orientation(a, b, p) == 0) {
        Core::Stadium { a, b, radius: 0 }
    } else {
        Core::Polygon(poly)
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

/// The segment a polygon with no area (all corners collinear, or one or two
/// distinct points) collapses to, its two farthest corners; `None` for an
/// empty ring or one with area. A point gives a zero-length segment.
fn flat_segment(poly: &[PointNm]) -> Option<(PointNm, PointNm)> {
    let first = *poly.first()?;
    let far = |from: PointNm| {
        poly.iter()
            .copied()
            .max_by_key(|p| p.distance_sq(from))
            .unwrap_or(from)
    };
    let a = far(first);
    let b = far(a);
    poly.iter()
        .all(|&p| orientation(a, b, p) == 0)
        .then_some((a, b))
}

/// Whether two simple polygons (corners in order, convex or not) share a
/// point; touching counts. Two simple polygons meet exactly when an edge of
/// one meets an edge of the other, or one holds a corner of the other
/// (wholly inside). Integer nm. A ring with no area is the point or
/// zero-width segment it collapses to (see [`Shape::overlaps_polygon`]), so
/// it never "holds" a corner and is never held; an empty ring meets nothing.
pub(crate) fn polygons_meet(a: &[PointNm], b: &[PointNm]) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }
    match (flat_segment(a), flat_segment(b)) {
        (Some((p, q)), Some((r, t))) => segments_intersect(p, q, r, t),
        (Some((p, q)), None) => Shape::Capsule {
            a: p,
            b: q,
            radius: 0,
        }
        .overlaps_polygon(b),
        (None, Some((r, t))) => Shape::Capsule {
            a: r,
            b: t,
            radius: 0,
        }
        .overlaps_polygon(a),
        (None, None) => {
            let edge = |p: &[PointNm], i: usize| (p[i], p[(i + 1) % p.len()]);
            (0..a.len()).any(|i| {
                let (p, q) = edge(a, i);
                (0..b.len()).any(|j| {
                    let (r, t) = edge(b, j);
                    segments_intersect(p, q, r, t)
                })
            }) || point_in_polygon(a, b[0])
                || point_in_polygon(b, a[0])
        }
    }
}

/// Whether `p` lies in or on a simple polygon (even-odd crossing count).
fn point_in_polygon(poly: &[PointNm], p: PointNm) -> bool {
    let mut inside = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if orientation(a, b, p) == 0 && crate::rect::RectNm::new(a, b).contains_point(p) {
            return true;
        }
        if (a.y > p.y) != (b.y > p.y) {
            // The edge crosses the ray from p towards +x when p is left of
            // it: sign of the turn, flipped for a downward edge.
            let left = orientation(a, b, p) > 0;
            if left == (b.y > a.y) {
                inside = !inside;
            }
        }
    }
    inside
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
