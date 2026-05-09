//! Curve × curve intersections — Circle × Circle, Arc × Circle,
//! Arc × Arc.
//!
//! Built on the Circle × Circle quadratic root-finder; arc filters
//! reuse the angular containment helpers on `Arc2`.

use super::segment::{Arc2, Circle2};
use super::Point2;

/// Find the intersection points of two circles. Returns 0, 1, or
/// 2 hit points.
///
/// Algorithm — standard:
///   d = |c1 - c0|
///   - If d > r0 + r1 → disjoint (0 hits).
///   - If d < |r0 - r1| → one circle inside the other (0 hits).
///   - Else two roots at the perpendicular foot:
///       a = (r0² - r1² + d²) / (2 d)
///       h = sqrt(r0² - a²)
///       midpoint = c0 + a * (c1 - c0) / d
///       hits = midpoint ± h * perpendicular(c1 - c0) / d
///   - Tangent (d == r0 + r1 or d == |r0 - r1|) collapses to one
///     hit at the tangency point.
pub fn circle_circle_intersections(a: Circle2, b: Circle2) -> Vec<Point2> {
    let dx = b.center.x - a.center.x;
    let dy = b.center.y - a.center.y;
    let d_sq = dx * dx + dy * dy;
    if d_sq < 1e-24 {
        // Concentric circles — either coincident (infinite hits,
        // we return empty) or nested (0 hits).
        return Vec::new();
    }
    let d = d_sq.sqrt();
    let r_sum = a.radius + b.radius;
    let r_diff = (a.radius - b.radius).abs();
    if d > r_sum + 1e-12 || d < r_diff - 1e-12 {
        return Vec::new();
    }
    let aa = (a.radius * a.radius - b.radius * b.radius + d_sq) / (2.0 * d);
    let h_sq = a.radius * a.radius - aa * aa;
    let h = h_sq.max(0.0).sqrt();
    let mid = Point2::new(a.center.x + aa * dx / d, a.center.y + aa * dy / d);
    let perp_x = -dy / d;
    let perp_y = dx / d;
    if h < 1e-9 {
        // Tangent — single point.
        return vec![mid];
    }
    vec![
        Point2::new(mid.x + h * perp_x, mid.y + h * perp_y),
        Point2::new(mid.x - h * perp_x, mid.y - h * perp_y),
    ]
}

/// Find intersection points of an arc and a circle. Wraps
/// `circle_circle_intersections` and filters by the arc's angular
/// range.
pub fn arc_circle_intersections(arc: Arc2, circle: Circle2) -> Vec<Point2> {
    let a_circle = Circle2::new(arc.center, arc.radius);
    circle_circle_intersections(a_circle, circle)
        .into_iter()
        .filter(|pt| {
            let theta = (pt.y - arc.center.y).atan2(pt.x - arc.center.x);
            arc.contains_angle(theta)
        })
        .collect()
}

/// Find intersection points of two arcs. Wraps the circle solver
/// and filters BOTH arcs' angular ranges.
pub fn arc_arc_intersections(a: Arc2, b: Arc2) -> Vec<Point2> {
    let circle_a = Circle2::new(a.center, a.radius);
    let circle_b = Circle2::new(b.center, b.radius);
    circle_circle_intersections(circle_a, circle_b)
        .into_iter()
        .filter(|pt| {
            let theta_a = (pt.y - a.center.y).atan2(pt.x - a.center.x);
            let theta_b = (pt.y - b.center.y).atan2(pt.x - b.center.x);
            a.contains_angle(theta_a) && b.contains_angle(theta_b)
        })
        .collect()
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
    fn circles_two_intersection() {
        // Unit circle at origin and unit circle at (1, 0). They
        // cross at (0.5, ±√3/2).
        let a = Circle2::new(p(0.0, 0.0), 1.0);
        let b = Circle2::new(p(1.0, 0.0), 1.0);
        let hits = circle_circle_intersections(a, b);
        assert_eq!(hits.len(), 2);
        let h = (3.0_f64).sqrt() / 2.0;
        assert!(hits.iter().any(|pt| close(*pt, p(0.5, h), 1e-9)));
        assert!(hits.iter().any(|pt| close(*pt, p(0.5, -h), 1e-9)));
    }

    #[test]
    fn circles_disjoint_no_intersection() {
        let a = Circle2::new(p(0.0, 0.0), 1.0);
        let b = Circle2::new(p(10.0, 0.0), 1.0);
        assert!(circle_circle_intersections(a, b).is_empty());
    }

    #[test]
    fn circles_nested_no_intersection() {
        let outer = Circle2::new(p(0.0, 0.0), 5.0);
        let inner = Circle2::new(p(0.5, 0.0), 1.0);
        assert!(circle_circle_intersections(outer, inner).is_empty());
    }

    #[test]
    fn circles_tangent_single_point() {
        // Externally tangent: distance == r0 + r1 == 2.
        let a = Circle2::new(p(0.0, 0.0), 1.0);
        let b = Circle2::new(p(2.0, 0.0), 1.0);
        let hits = circle_circle_intersections(a, b);
        assert_eq!(hits.len(), 1);
        assert!(close(hits[0], p(1.0, 0.0), 1e-7));
    }

    #[test]
    fn circles_concentric_returns_empty() {
        let a = Circle2::new(p(0.0, 0.0), 1.0);
        let b = Circle2::new(p(0.0, 0.0), 1.0);
        assert!(circle_circle_intersections(a, b).is_empty());
    }

    #[test]
    fn arc_circle_filters_outside_sweep() {
        // Quarter arc 0..π/2 on unit circle at origin. The full
        // circle of unit radius offset to (1, 0) crosses at
        // (0.5, ±√3/2) — only the +√3/2 hit lies in the arc's
        // first-quadrant sweep.
        let arc = Arc2::new(
            p(0.0, 0.0),
            1.0,
            0.0,
            std::f64::consts::FRAC_PI_2,
            true,
        );
        let circle = Circle2::new(p(1.0, 0.0), 1.0);
        let hits = arc_circle_intersections(arc, circle);
        assert_eq!(hits.len(), 1);
        let h = (3.0_f64).sqrt() / 2.0;
        assert!(close(hits[0], p(0.5, h), 1e-9));
    }

    #[test]
    fn arc_arc_both_sweeps_contain_hit() {
        // Two unit-radius arcs crossing at the standard (0.5, ±h)
        // points. Pick sweeps that include one hit each.
        let h = (3.0_f64).sqrt() / 2.0;
        // Arc A centred at origin, sweeping the upper half (0..π).
        let a = Arc2::new(p(0.0, 0.0), 1.0, 0.0, std::f64::consts::PI, true);
        // Arc B centred at (1, 0), sweeping its left half (π/2..π).
        let b = Arc2::new(
            p(1.0, 0.0),
            1.0,
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::PI,
            true,
        );
        let hits = arc_arc_intersections(a, b);
        assert_eq!(hits.len(), 1);
        assert!(close(hits[0], p(0.5, h), 1e-9));
    }

    #[test]
    fn arc_arc_no_overlap_in_sweeps() {
        // Same circles as before but the second arc is on its
        // RIGHT half (270°..360°) — no shared hit lies in both
        // sweeps.
        let a = Arc2::new(p(0.0, 0.0), 1.0, 0.0, std::f64::consts::PI, true);
        let b = Arc2::new(
            p(1.0, 0.0),
            1.0,
            -std::f64::consts::FRAC_PI_2,
            0.0,
            true,
        );
        let hits = arc_arc_intersections(a, b);
        assert!(hits.is_empty());
    }
}
