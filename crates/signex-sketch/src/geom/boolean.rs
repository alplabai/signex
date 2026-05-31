//! Polygon boolean operations.
//!
//! Phase 2 stage 1: polygon-against-convex-clip via the
//! Sutherland-Hodgman recursive-edge-clip algorithm. Handles any
//! subject polygon (convex or concave) clipped against a convex
//! clip polygon. Returns the intersection of the two as a single
//! polygon ring.
//!
//! General-polygon ↔ general-polygon booleans (Vatti, Greiner-
//! Hormann) are deferred — the convex-clip case covers viewport
//! clipping, courtyard clipping, and mask-window cutouts (where
//! the clip is always a rectangle or a fixed shape) which cover
//! ~80% of the practical use sites in the editor today.

use super::predicates::signed_area;
use super::Point2;

/// Clip the (possibly concave) `subject` polygon against the
/// **convex** `clip` polygon. Returns a single polygon ring
/// representing the intersection. An empty result means the
/// subject lies entirely outside the clip, or the inputs are
/// degenerate.
///
/// Algorithm: walk every edge of `clip`. For each edge, treat it
/// as a half-plane and pass the current subject through it,
/// keeping vertices on the inside and inserting intersection
/// points where edges cross the half-plane. The output of one
/// iteration becomes the input of the next.
///
/// The convex constraint on `clip` is what makes this work
/// without branching into separate output rings — any subject
/// edge can cross a clip edge at most twice (once entering,
/// once exiting), and the kept-inside vertices stay connected.
pub fn intersect_convex_clip(subject: &[Point2], clip: &[Point2]) -> Vec<Point2> {
    if subject.len() < 3 || clip.len() < 3 {
        return Vec::new();
    }
    // Normalise clip winding to CCW so the half-plane test is
    // consistent. We're going to test "is point P on the LEFT of
    // (a -> b)" for each edge (a, b) of the clip; that's the
    // inside half-plane only when the polygon is CCW.
    let clip_ccw: Vec<Point2> = if signed_area(clip) >= 0.0 {
        clip.to_vec()
    } else {
        clip.iter().rev().copied().collect()
    };

    let mut out: Vec<Point2> = subject.to_vec();
    let n = clip_ccw.len();
    for i in 0..n {
        if out.is_empty() {
            return out;
        }
        let a = clip_ccw[i];
        let b = clip_ccw[(i + 1) % n];
        out = clip_against_edge(&out, a, b);
    }
    out
}

/// Clip `subject` (a closed ring) against a single half-plane
/// defined by the directed edge `a → b`. Inside = left of the
/// edge (CCW convention). Returns a new ring.
fn clip_against_edge(subject: &[Point2], a: Point2, b: Point2) -> Vec<Point2> {
    if subject.is_empty() {
        return Vec::new();
    }
    let n = subject.len();
    let mut out: Vec<Point2> = Vec::with_capacity(n + 4);
    for i in 0..n {
        let curr = subject[i];
        let prev = subject[(i + n - 1) % n];
        let curr_inside = is_inside(curr, a, b);
        let prev_inside = is_inside(prev, a, b);
        match (prev_inside, curr_inside) {
            (true, true) => {
                out.push(curr);
            }
            (true, false) => {
                if let Some(pt) = line_edge_intersection(prev, curr, a, b) {
                    out.push(pt);
                }
            }
            (false, true) => {
                if let Some(pt) = line_edge_intersection(prev, curr, a, b) {
                    out.push(pt);
                }
                out.push(curr);
            }
            (false, false) => {
                // Both outside — emit nothing.
            }
        }
    }
    out
}

/// `true` when `p` is on the inside (left side) of the directed
/// edge `a → b`. Inclusive — points exactly on the edge count as
/// inside so the polygon's own vertices on a clip boundary aren't
/// dropped.
fn is_inside(p: Point2, a: Point2, b: Point2) -> bool {
    let cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
    cross >= -1e-12
}

/// Intersection of the segment `(p1, p2)` with the LINE through
/// `(a, b)` (extended infinitely, not the segment). Sutherland-
/// Hodgman crosses the half-plane edge at the parametric point
/// where the cross-product flips sign; the intersection always
/// lies on the segment iff the prev/curr inside flags differ.
fn line_edge_intersection(p1: Point2, p2: Point2, a: Point2, b: Point2) -> Option<Point2> {
    let dx_ab = b.x - a.x;
    let dy_ab = b.y - a.y;
    let dx_p = p2.x - p1.x;
    let dy_p = p2.y - p1.y;
    let denom = dx_ab * dy_p - dy_ab * dx_p;
    if denom.abs() < 1e-12 {
        return None;
    }
    let t = ((a.x - p1.x) * dy_ab - (a.y - p1.y) * dx_ab) / -denom;
    Some(Point2::new(p1.x + t * dx_p, p1.y + t * dy_p))
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

    fn area(poly: &[Point2]) -> f64 {
        signed_area(poly).abs()
    }

    #[test]
    fn empty_inputs_return_empty() {
        let sq = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        assert!(intersect_convex_clip(&[], &sq).is_empty());
        assert!(intersect_convex_clip(&sq, &[]).is_empty());
    }

    #[test]
    fn full_overlap_returns_subject() {
        // Subject inside clip → result equals subject.
        let inner = vec![p(0.25, 0.25), p(0.75, 0.25), p(0.75, 0.75), p(0.25, 0.75)];
        let outer = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let out = intersect_convex_clip(&inner, &outer);
        assert!((area(&out) - 0.25).abs() < 1e-9);
    }

    #[test]
    fn no_overlap_returns_empty() {
        let a = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let b = vec![p(2.0, 2.0), p(3.0, 2.0), p(3.0, 3.0), p(2.0, 3.0)];
        let out = intersect_convex_clip(&a, &b);
        assert!(out.is_empty() || area(&out) < 1e-12);
    }

    #[test]
    fn partial_overlap_quarter_square() {
        let a = vec![p(0.0, 0.0), p(2.0, 0.0), p(2.0, 2.0), p(0.0, 2.0)];
        let b = vec![p(1.0, 1.0), p(3.0, 1.0), p(3.0, 3.0), p(1.0, 3.0)];
        let out = intersect_convex_clip(&a, &b);
        // Intersection is the unit square (1,1)-(2,2), area 1.
        assert!((area(&out) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn concave_subject_against_convex_clip() {
        // L-shape clipped against a square cutting through the
        // concave corner.
        let l_shape = vec![
            p(0.0, 0.0),
            p(2.0, 0.0),
            p(2.0, 1.0),
            p(1.0, 1.0),
            p(1.0, 2.0),
            p(0.0, 2.0),
        ];
        let clip = vec![p(-1.0, -1.0), p(3.0, -1.0), p(3.0, 1.5), p(-1.0, 1.5)];
        let out = intersect_convex_clip(&l_shape, &clip);
        // L-shape area below y=1.5 = 2 (full bottom rectangle 2x1)
        // + (0.5 strip on the left where x<1, y in [1, 1.5]) = 2.5.
        assert!((area(&out) - 2.5).abs() < 1e-9);
    }

    #[test]
    fn cw_clip_winding_normalised() {
        // Same as full_overlap but the clip polygon is CW. The
        // implementation should normalise it internally.
        let inner = vec![p(0.25, 0.25), p(0.75, 0.25), p(0.75, 0.75), p(0.25, 0.75)];
        let outer_cw = vec![p(0.0, 0.0), p(0.0, 1.0), p(1.0, 1.0), p(1.0, 0.0)];
        let out = intersect_convex_clip(&inner, &outer_cw);
        assert!((area(&out) - 0.25).abs() < 1e-9);
    }

    #[test]
    fn point_on_clip_edge_kept() {
        // Subject vertex exactly on the clip's bottom edge — must
        // stay in the result, not be dropped as "outside".
        let subject = vec![p(0.0, 0.0), p(1.0, 0.0), p(0.5, 1.0)];
        let clip = vec![p(-1.0, 0.0), p(2.0, 0.0), p(2.0, 2.0), p(-1.0, 2.0)];
        let out = intersect_convex_clip(&subject, &clip);
        // The triangle is fully on the inside / boundary; result
        // should preserve its area = 0.5.
        assert!((area(&out) - 0.5).abs() < 1e-9);
        // And the (0,0) and (1,0) vertices land on the boundary.
        assert!(out.iter().any(|v| close(*v, p(0.0, 0.0), 1e-9)));
        assert!(out.iter().any(|v| close(*v, p(1.0, 0.0), 1e-9)));
    }
}
