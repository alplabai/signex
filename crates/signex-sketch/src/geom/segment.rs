//! Segment, circle, and arc intersection helpers.
//!
//! All inputs are in plane-local mm. Returned intersections include
//! the parameter `t ∈ [0, 1]` along the segment so callers can
//! reconstruct the world-mm hit position via `seg.at(t)` and order
//! multiple intersections along the ray direction.

use super::{Point2, Sign};
use super::predicates::{orient2d, DEFAULT_TOL};

/// 2D line segment from `a` to `b`. Endpoints stored as bare points
/// so the user can build one inline from `EntityKind::Point` data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment2 {
    pub a: Point2,
    pub b: Point2,
}

impl Segment2 {
    pub const fn new(a: Point2, b: Point2) -> Self {
        Self { a, b }
    }

    /// Linearly interpolate along the segment. `t = 0` returns `a`,
    /// `t = 1` returns `b`. Outside `[0, 1]` extrapolates onto the
    /// underlying line.
    pub fn at(&self, t: f64) -> Point2 {
        Point2 {
            x: self.a.x + t * (self.b.x - self.a.x),
            y: self.a.y + t * (self.b.y - self.a.y),
        }
    }

    pub fn dx(&self) -> f64 {
        self.b.x - self.a.x
    }

    pub fn dy(&self) -> f64 {
        self.b.y - self.a.y
    }

    pub fn length_sq(&self) -> f64 {
        let dx = self.dx();
        let dy = self.dy();
        dx * dx + dy * dy
    }
}

/// 2D circle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circle2 {
    pub center: Point2,
    pub radius: f64,
}

impl Circle2 {
    pub const fn new(center: Point2, radius: f64) -> Self {
        Self { center, radius }
    }
}

/// 2D circular arc. `start_rad` and `end_rad` are angles in radians
/// measured from the centre using the standard atan2 convention
/// (positive X axis = 0, CCW positive). `sweep_ccw = true` means the
/// arc walks CCW from `start_rad` to `end_rad`. The arc may cross
/// the seam at ±π — the angular containment check handles wrap-around.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arc2 {
    pub center: Point2,
    pub radius: f64,
    pub start_rad: f64,
    pub end_rad: f64,
    pub sweep_ccw: bool,
}

impl Arc2 {
    pub fn new(center: Point2, radius: f64, start_rad: f64, end_rad: f64, sweep_ccw: bool) -> Self {
        Self {
            center,
            radius,
            start_rad,
            end_rad,
            sweep_ccw,
        }
    }

    /// `true` when the angle `theta` (any reference frame, will be
    /// normalised) lies within the arc's sweep — inclusive on both
    /// ends. Handles arcs that cross the seam.
    pub fn contains_angle(&self, theta: f64) -> bool {
        let two_pi = std::f64::consts::TAU;
        let normalise = |a: f64| -> f64 {
            let mut x = a % two_pi;
            if x < 0.0 {
                x += two_pi;
            }
            x
        };
        let s = normalise(self.start_rad);
        let mut e = normalise(self.end_rad);
        let mut p = normalise(theta);
        if self.sweep_ccw {
            // CCW from s to e — unwrap so e >= s.
            if e < s {
                e += two_pi;
            }
            if p < s {
                p += two_pi;
            }
            p <= e + DEFAULT_TOL
        } else {
            // CW: walk negative direction. Unwrap so s >= e.
            let mut s = s;
            if s < e {
                s += two_pi;
            }
            if p < e {
                p += two_pi;
            }
            p <= s + DEFAULT_TOL
        }
    }
}

/// Outcome of a segment ↔ segment intersection query.
#[derive(Debug, Clone, PartialEq)]
pub enum SegmentIntersection {
    /// No intersection — disjoint or parallel non-colinear.
    None,
    /// Single point at world position `pt`, parameters `t0` (along
    /// the first segment) and `t1` (along the second), each in
    /// `[0, 1]`.
    Point {
        pt: Point2,
        t0: f64,
        t1: f64,
    },
    /// Colinear overlap. `from` and `to` are the world endpoints of
    /// the overlapping range, ordered along the first segment's
    /// direction. Equal points = touch-only (single shared endpoint).
    Overlap {
        from: Point2,
        to: Point2,
    },
}

/// Intersect two line segments. Returns `None` for parallel-disjoint
/// pairs, `Point` for the standard cross-intersection (or a shared
/// endpoint), and `Overlap` for colinear segments that share more
/// than a single point.
///
/// Algorithm: solve `A + t*(B-A) = C + s*(D-C)` for (t, s). The
/// determinant of the 2x2 system tells us the lines aren't parallel;
/// when it's zero we fall through to a colinear-overlap check using
/// projection onto the longest axis.
pub fn segment_segment_intersection(p: Segment2, q: Segment2) -> SegmentIntersection {
    let r = (p.dx(), p.dy());
    let s = (q.dx(), q.dy());
    // Cross product of the two direction vectors.
    let denom = r.0 * s.1 - r.1 * s.0;
    let qmp = (q.a.x - p.a.x, q.a.y - p.a.y);
    if denom.abs() <= DEFAULT_TOL * (r.0.abs() + r.1.abs() + s.0.abs() + s.1.abs()).max(1.0) {
        // Parallel — colinear iff (q-p) × r == 0 too.
        let cross = qmp.0 * r.1 - qmp.1 * r.0;
        if cross.abs()
            > DEFAULT_TOL * (qmp.0.abs() + qmp.1.abs() + r.0.abs() + r.1.abs()).max(1.0)
        {
            return SegmentIntersection::None;
        }
        // Colinear: parameterise q on p's line via projection along
        // the dominant axis. Avoids divide-by-near-zero on degenerate
        // r where both components are tiny.
        let len_sq = r.0 * r.0 + r.1 * r.1;
        if len_sq <= DEFAULT_TOL {
            // p is a point — it intersects iff q contains it.
            return if (q.a == p.a) || (q.b == p.a) {
                SegmentIntersection::Point {
                    pt: p.a,
                    t0: 0.0,
                    t1: if q.a == p.a { 0.0 } else { 1.0 },
                }
            } else {
                SegmentIntersection::None
            };
        }
        let t_q_a = (qmp.0 * r.0 + qmp.1 * r.1) / len_sq;
        let t_q_b = ((q.b.x - p.a.x) * r.0 + (q.b.y - p.a.y) * r.1) / len_sq;
        let (t_lo, t_hi) = if t_q_a <= t_q_b {
            (t_q_a, t_q_b)
        } else {
            (t_q_b, t_q_a)
        };
        let lo = t_lo.max(0.0);
        let hi = t_hi.min(1.0);
        if hi + DEFAULT_TOL < lo {
            return SegmentIntersection::None;
        }
        let from = p.at(lo);
        let to = p.at(hi);
        if (hi - lo).abs() <= DEFAULT_TOL {
            // Single touch point.
            return SegmentIntersection::Point {
                pt: from,
                t0: lo,
                t1: 0.0,
            };
        }
        return SegmentIntersection::Overlap { from, to };
    }
    // Non-parallel: solve for t and s. Both must lie in [0, 1] for a
    // proper segment intersection.
    let t = (qmp.0 * s.1 - qmp.1 * s.0) / denom;
    let u = (qmp.0 * r.1 - qmp.1 * r.0) / denom;
    if (-DEFAULT_TOL..=1.0 + DEFAULT_TOL).contains(&t)
        && (-DEFAULT_TOL..=1.0 + DEFAULT_TOL).contains(&u)
    {
        SegmentIntersection::Point {
            pt: p.at(t),
            t0: t.clamp(0.0, 1.0),
            t1: u.clamp(0.0, 1.0),
        }
    } else {
        SegmentIntersection::None
    }
}

/// Intersect a segment with a circle. Returns 0, 1, or 2 hit points
/// with their `t` values along the segment in ascending order.
///
/// Algorithm: solve `|A + t*d - C|² = r²` for `t`, a quadratic in
/// `t` with coefficients
/// ```text
///   a = d·d
///   b = 2 * d·(A - C)
///   c = (A - C)·(A - C) - r²
/// ```
/// The discriminant `b² - 4ac` discriminates the three cases. Both
/// roots are filtered to `[0, 1]` so the returned hits lie on the
/// segment, not the extended line.
pub fn segment_circle_intersections(seg: Segment2, circle: Circle2) -> Vec<(Point2, f64)> {
    let d = (seg.dx(), seg.dy());
    let f = (seg.a.x - circle.center.x, seg.a.y - circle.center.y);
    let a = d.0 * d.0 + d.1 * d.1;
    if a <= DEFAULT_TOL {
        // Degenerate segment (a == b).
        let dist_sq = f.0 * f.0 + f.1 * f.1;
        if (dist_sq - circle.radius * circle.radius).abs() <= DEFAULT_TOL {
            return vec![(seg.a, 0.0)];
        }
        return Vec::new();
    }
    let b = 2.0 * (f.0 * d.0 + f.1 * d.1);
    let c = f.0 * f.0 + f.1 * f.1 - circle.radius * circle.radius;
    let disc = b * b - 4.0 * a * c;
    if disc < -DEFAULT_TOL {
        return Vec::new();
    }
    let disc_clamped = disc.max(0.0);
    let sqrt_disc = disc_clamped.sqrt();
    let t0 = (-b - sqrt_disc) / (2.0 * a);
    let t1 = (-b + sqrt_disc) / (2.0 * a);
    let mut out = Vec::new();
    let on_seg = |t: f64| t >= -DEFAULT_TOL && t <= 1.0 + DEFAULT_TOL;
    if on_seg(t0) {
        let tc = t0.clamp(0.0, 1.0);
        out.push((seg.at(tc), tc));
    }
    // Skip the second root when discriminant is effectively zero
    // (tangent — t0 == t1).
    if disc_clamped > DEFAULT_TOL && on_seg(t1) {
        let tc = t1.clamp(0.0, 1.0);
        out.push((seg.at(tc), tc));
    }
    out
}

/// Intersect a segment with an arc. Built on top of
/// `segment_circle_intersections` with an angular containment filter
/// so only points lying within the arc's sweep are returned.
pub fn segment_arc_intersections(seg: Segment2, arc: Arc2) -> Vec<(Point2, f64)> {
    let circle = Circle2::new(arc.center, arc.radius);
    segment_circle_intersections(seg, circle)
        .into_iter()
        .filter(|(pt, _)| {
            let theta = (pt.y - arc.center.y).atan2(pt.x - arc.center.x);
            arc.contains_angle(theta)
        })
        .collect()
}

/// `true` when the three points `a`, `b`, `c` form a left turn (or
/// `b` is colinear and on the segment from `a` to `c`). Useful for
/// convex-hull-style "remove right turns" loops where colinear-on-
/// segment points should NOT be removed.
pub fn left_turn_or_colinear(a: Point2, b: Point2, c: Point2) -> bool {
    !matches!(orient2d(a, b, c), Sign::Negative)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    fn close(a: Point2, b: Point2, tol: f64) -> bool {
        (a.x - b.x).abs() < tol && (a.y - b.y).abs() < tol
    }

    #[test]
    fn cross_intersection() {
        let s1 = Segment2::new(p(0.0, 0.0), p(2.0, 2.0));
        let s2 = Segment2::new(p(0.0, 2.0), p(2.0, 0.0));
        match segment_segment_intersection(s1, s2) {
            SegmentIntersection::Point { pt, t0, t1 } => {
                assert!(close(pt, p(1.0, 1.0), 1e-9));
                assert!((t0 - 0.5).abs() < 1e-9);
                assert!((t1 - 0.5).abs() < 1e-9);
            }
            other => panic!("expected Point, got {other:?}"),
        }
    }

    #[test]
    fn parallel_disjoint() {
        let s1 = Segment2::new(p(0.0, 0.0), p(2.0, 0.0));
        let s2 = Segment2::new(p(0.0, 1.0), p(2.0, 1.0));
        assert_eq!(
            segment_segment_intersection(s1, s2),
            SegmentIntersection::None
        );
    }

    #[test]
    fn colinear_overlap() {
        let s1 = Segment2::new(p(0.0, 0.0), p(2.0, 0.0));
        let s2 = Segment2::new(p(1.0, 0.0), p(3.0, 0.0));
        match segment_segment_intersection(s1, s2) {
            SegmentIntersection::Overlap { from, to } => {
                assert!(close(from, p(1.0, 0.0), 1e-9));
                assert!(close(to, p(2.0, 0.0), 1e-9));
            }
            other => panic!("expected Overlap, got {other:?}"),
        }
    }

    #[test]
    fn t_intersection_at_endpoint() {
        // s2 ends at the midpoint of s1.
        let s1 = Segment2::new(p(0.0, 0.0), p(2.0, 0.0));
        let s2 = Segment2::new(p(1.0, 1.0), p(1.0, 0.0));
        match segment_segment_intersection(s1, s2) {
            SegmentIntersection::Point { pt, .. } => {
                assert!(close(pt, p(1.0, 0.0), 1e-9));
            }
            other => panic!("expected Point, got {other:?}"),
        }
    }

    #[test]
    fn miss_outside_segment_range() {
        let s1 = Segment2::new(p(0.0, 0.0), p(1.0, 1.0));
        // Lines cross at (3, 3) — outside both segments.
        let s2 = Segment2::new(p(2.0, 4.0), p(4.0, 2.0));
        assert_eq!(
            segment_segment_intersection(s1, s2),
            SegmentIntersection::None
        );
    }

    #[test]
    fn segment_circle_two_hits() {
        let seg = Segment2::new(p(-2.0, 0.0), p(2.0, 0.0));
        let circle = Circle2::new(p(0.0, 0.0), 1.0);
        let hits = segment_circle_intersections(seg, circle);
        assert_eq!(hits.len(), 2);
        assert!(close(hits[0].0, p(-1.0, 0.0), 1e-9));
        assert!(close(hits[1].0, p(1.0, 0.0), 1e-9));
    }

    #[test]
    fn segment_circle_tangent_one_hit() {
        let seg = Segment2::new(p(-2.0, 1.0), p(2.0, 1.0));
        let circle = Circle2::new(p(0.0, 0.0), 1.0);
        let hits = segment_circle_intersections(seg, circle);
        assert_eq!(hits.len(), 1);
        assert!(close(hits[0].0, p(0.0, 1.0), 1e-7));
    }

    #[test]
    fn segment_circle_miss() {
        let seg = Segment2::new(p(-2.0, 2.0), p(2.0, 2.0));
        let circle = Circle2::new(p(0.0, 0.0), 1.0);
        assert!(segment_circle_intersections(seg, circle).is_empty());
    }

    #[test]
    fn segment_circle_partial_inside_segment() {
        // Segment starts inside the circle and exits.
        let seg = Segment2::new(p(0.0, 0.0), p(2.0, 0.0));
        let circle = Circle2::new(p(0.0, 0.0), 1.0);
        let hits = segment_circle_intersections(seg, circle);
        assert_eq!(hits.len(), 1);
        assert!(close(hits[0].0, p(1.0, 0.0), 1e-9));
    }

    #[test]
    fn arc_angle_containment_within_quadrant() {
        let arc = Arc2::new(p(0.0, 0.0), 1.0, 0.0, std::f64::consts::FRAC_PI_2, true);
        assert!(arc.contains_angle(std::f64::consts::FRAC_PI_4));
        assert!(!arc.contains_angle(-std::f64::consts::FRAC_PI_4));
    }

    #[test]
    fn arc_seam_crossing() {
        // Arc from -45° to 45°, CCW — crosses the 0 / 2π seam.
        let arc = Arc2::new(
            p(0.0, 0.0),
            1.0,
            -std::f64::consts::FRAC_PI_4,
            std::f64::consts::FRAC_PI_4,
            true,
        );
        assert!(arc.contains_angle(0.0));
        assert!(!arc.contains_angle(std::f64::consts::PI));
    }

    #[test]
    fn segment_arc_filters_outside_sweep() {
        // Quarter-arc from 0° to 90°. A horizontal segment through
        // the centre crosses the FULL circle at (-1, 0) and (1, 0)
        // — only (1, 0) (angle 0°) lies in the arc's sweep, so we
        // expect a single hit.
        let seg = Segment2::new(p(-2.0, 0.0), p(2.0, 0.0));
        let arc = Arc2::new(p(0.0, 0.0), 1.0, 0.0, std::f64::consts::FRAC_PI_2, true);
        let hits = segment_arc_intersections(seg, arc);
        assert_eq!(hits.len(), 1);
        assert!(close(hits[0].0, p(1.0, 0.0), 1e-9));
    }
}
