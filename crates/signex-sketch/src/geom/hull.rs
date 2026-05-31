//! Convex hull via the monotone-chain method, O(n log n).
//!
//! Strategy:
//!   1. Sort points lexicographically by `(x, y)`.
//!   2. Build the lower hull walking points left-to-right, popping
//!      the back of the stack while the turn at the new point is a
//!      right turn (i.e. not a left turn).
//!   3. Build the upper hull walking right-to-left with the same
//!      rule.
//!   4. Concatenate, dropping the duplicated end point of each
//!      subhull.
//!
//! The output is a CCW polygon in standard orientation. Duplicate
//! and colinear points are handled — colinear points on the hull
//! edge are dropped (only the extreme endpoints survive).

use super::predicates::{orient2d, Sign};
use super::Point2;

/// Build the convex hull of `points`. Returns the hull vertices in
/// counter-clockwise order. An input with fewer than 3 distinct
/// points returns the deduplicated input as-is (a zero-area hull).
pub fn convex_hull(points: &[Point2]) -> Vec<Point2> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let mut sorted: Vec<Point2> = points.to_vec();
    sorted.sort_by(|a, b| {
        a.x.partial_cmp(&b.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
    });
    sorted.dedup();
    if sorted.len() < 3 {
        return sorted;
    }

    // Lower hull.
    let mut lower: Vec<Point2> = Vec::with_capacity(sorted.len());
    for &pt in sorted.iter() {
        while lower.len() >= 2 {
            let a = lower[lower.len() - 2];
            let b = lower[lower.len() - 1];
            // Pop b when (a, b, pt) is NOT a left turn (i.e. right
            // turn or colinear). This drops colinear points along
            // the hull edge so the output has only the extreme
            // vertices.
            if matches!(orient2d(a, b, pt), Sign::Positive) {
                break;
            }
            lower.pop();
        }
        lower.push(pt);
    }

    // Upper hull.
    let mut upper: Vec<Point2> = Vec::with_capacity(sorted.len());
    for &pt in sorted.iter().rev() {
        while upper.len() >= 2 {
            let a = upper[upper.len() - 2];
            let b = upper[upper.len() - 1];
            if matches!(orient2d(a, b, pt), Sign::Positive) {
                break;
            }
            upper.pop();
        }
        upper.push(pt);
    }

    // Stitch — drop the last point of each subhull because it's the
    // first point of the other.
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    #[test]
    fn empty_input_returns_empty_hull() {
        assert!(convex_hull(&[]).is_empty());
    }

    #[test]
    fn single_point_returns_itself() {
        let hull = convex_hull(&[p(1.0, 2.0)]);
        assert_eq!(hull, vec![p(1.0, 2.0)]);
    }

    #[test]
    fn two_points_return_two_points() {
        let hull = convex_hull(&[p(0.0, 0.0), p(1.0, 1.0)]);
        assert_eq!(hull.len(), 2);
    }

    #[test]
    fn square_corners_are_the_hull() {
        let pts = [p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let hull = convex_hull(&pts);
        // Hull is the square itself, CCW-ordered starting at (0, 0).
        assert_eq!(hull.len(), 4);
        // Verify CCW.
        use super::super::predicates::signed_area;
        assert!(signed_area(&hull) > 0.0);
    }

    #[test]
    fn interior_point_is_excluded() {
        let pts = [
            p(0.0, 0.0),
            p(2.0, 0.0),
            p(2.0, 2.0),
            p(0.0, 2.0),
            p(1.0, 1.0), // interior
        ];
        let hull = convex_hull(&pts);
        assert_eq!(hull.len(), 4);
        assert!(!hull.contains(&p(1.0, 1.0)));
    }

    #[test]
    fn colinear_points_along_edge_are_dropped() {
        // Three points on the bottom edge — only the two endpoints
        // should survive in the final hull.
        let pts = [
            p(0.0, 0.0),
            p(0.5, 0.0),
            p(1.0, 0.0),
            p(1.0, 1.0),
            p(0.0, 1.0),
        ];
        let hull = convex_hull(&pts);
        assert_eq!(hull.len(), 4);
        assert!(!hull.contains(&p(0.5, 0.0)));
    }

    #[test]
    fn duplicate_points_are_deduplicated() {
        let pts = [
            p(0.0, 0.0),
            p(0.0, 0.0),
            p(1.0, 0.0),
            p(1.0, 1.0),
            p(0.0, 1.0),
        ];
        let hull = convex_hull(&pts);
        assert_eq!(hull.len(), 4);
    }

    #[test]
    fn hexagon_round_trip() {
        let pts: Vec<Point2> = (0..6)
            .map(|i| {
                let t = i as f64 / 6.0 * std::f64::consts::TAU;
                Point2::new(t.cos(), t.sin())
            })
            .collect();
        let hull = convex_hull(&pts);
        assert_eq!(hull.len(), 6);
    }
}
