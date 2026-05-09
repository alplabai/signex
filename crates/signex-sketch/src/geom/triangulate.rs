//! Polygon triangulation via ear-clipping.
//!
//! Time complexity: O(n²). Typical sketch closed loops have ≤ ~64
//! vertices (rounded rect = 8 + 4·arc_segments), where the basic
//! form's constant factors beat the more sophisticated O(n log n)
//! variants. Hole support is deferred — outer simple polygons only.

use super::predicates::{orient2d, signed_area, Sign};
use super::Point2;

/// Triangulate a simple polygon. Returns a list of triangles, each
/// as a triple of indices into the original `polygon` slice.
///
/// Convention:
/// - Input must be a closed simple polygon (no last-equals-first
///   duplicate).
/// - Winding is normalised internally — CW input gets a logical
///   reversal so the ear-detection always sees CCW.
/// - Output triangles are in the input's original index space and
///   in the input's original winding order.
///
/// Returns an empty Vec for fewer than three vertices, or when the
/// polygon is self-intersecting / has collinear vertices that make
/// every candidate degenerate.
pub fn ear_clip(polygon: &[Point2]) -> Vec<[usize; 3]> {
    if polygon.len() < 3 {
        return Vec::new();
    }

    // Detect winding so the ear test always operates on CCW.
    let area = signed_area(polygon);
    if area.abs() <= super::predicates::DEFAULT_TOL {
        return Vec::new();
    }
    let ccw = area > 0.0;

    // Working index ring. We'll repeatedly pop ears off this list.
    // Stored in CCW order — for CW input we walk in reverse.
    let mut indices: Vec<usize> = if ccw {
        (0..polygon.len()).collect()
    } else {
        (0..polygon.len()).rev().collect()
    };

    let mut triangles: Vec<[usize; 3]> = Vec::with_capacity(polygon.len().saturating_sub(2));

    // Bounded loop — every successful ear-clip removes one vertex,
    // so the loop terminates in at most n iterations. The `safety`
    // counter catches the pathological case where no ear is found
    // (degenerate input) so we don't infinite-loop.
    let mut safety = polygon.len() * polygon.len();
    while indices.len() > 3 {
        if safety == 0 {
            return Vec::new();
        }
        safety -= 1;

        let mut ear: Option<usize> = None;
        let n = indices.len();
        for i in 0..n {
            let i_prev = indices[(i + n - 1) % n];
            let i_curr = indices[i];
            let i_next = indices[(i + 1) % n];
            if is_ear(polygon, &indices, i_prev, i_curr, i_next) {
                ear = Some(i);
                break;
            }
        }

        match ear {
            Some(idx) => {
                let n = indices.len();
                let i_prev = indices[(idx + n - 1) % n];
                let i_curr = indices[idx];
                let i_next = indices[(idx + 1) % n];
                if ccw {
                    triangles.push([i_prev, i_curr, i_next]);
                } else {
                    // Mirror back to the input's CW orientation so
                    // each output triangle winds the same way as
                    // the original polygon.
                    triangles.push([i_next, i_curr, i_prev]);
                }
                indices.remove(idx);
            }
            None => return Vec::new(),
        }
    }

    // Three indices remain — emit the final triangle.
    if indices.len() == 3 {
        let i_prev = indices[0];
        let i_curr = indices[1];
        let i_next = indices[2];
        if ccw {
            triangles.push([i_prev, i_curr, i_next]);
        } else {
            triangles.push([i_next, i_curr, i_prev]);
        }
    }

    triangles
}

/// `true` when the triangle at index `(prev, curr, next)` is an ear
/// of the CCW-oriented polygon: convex AND no other polygon vertex
/// lies inside it. Operates on the CCW-canonicalised view via
/// `ring`.
fn is_ear(
    polygon: &[Point2],
    ring: &[usize],
    prev: usize,
    curr: usize,
    next: usize,
) -> bool {
    let a = polygon[prev];
    let b = polygon[curr];
    let c = polygon[next];
    // Convex test on CCW polygon: orient2d must be Positive.
    if !matches!(orient2d(a, b, c), Sign::Positive) {
        return false;
    }
    // No other vertex of the polygon may lie strictly inside the
    // triangle. We allow on-edge / on-vertex matches at the three
    // corners themselves.
    for &i in ring.iter() {
        if i == prev || i == curr || i == next {
            continue;
        }
        if point_in_triangle(polygon[i], a, b, c) {
            return false;
        }
    }
    true
}

/// Strict interior test for a point in a triangle. Uses three
/// orientation predicates — when all three return the same sign,
/// the point is on the appropriate side of every edge. Boundary
/// hits (Sign::Zero) count as "outside" so a vertex coincident
/// with an edge doesn't disqualify the candidate ear.
fn point_in_triangle(p: Point2, a: Point2, b: Point2, c: Point2) -> bool {
    let s_ab = orient2d(a, b, p);
    let s_bc = orient2d(b, c, p);
    let s_ca = orient2d(c, a, p);
    matches!(
        (s_ab, s_bc, s_ca),
        (Sign::Positive, Sign::Positive, Sign::Positive)
            | (Sign::Negative, Sign::Negative, Sign::Negative)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    #[test]
    fn empty_polygon_no_triangles() {
        assert!(ear_clip(&[]).is_empty());
    }

    #[test]
    fn two_vertex_polygon_no_triangles() {
        assert!(ear_clip(&[p(0.0, 0.0), p(1.0, 0.0)]).is_empty());
    }

    #[test]
    fn triangle_emits_one_triangle() {
        let pts = [p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0)];
        let tris = ear_clip(&pts);
        assert_eq!(tris.len(), 1);
        assert_eq!(tris[0], [0, 1, 2]);
    }

    #[test]
    fn convex_quad_emits_two_triangles() {
        let pts = [p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let tris = ear_clip(&pts);
        assert_eq!(tris.len(), 2);
    }

    #[test]
    fn convex_quad_cw_winding_still_works() {
        let pts = [p(0.0, 0.0), p(0.0, 1.0), p(1.0, 1.0), p(1.0, 0.0)];
        let tris = ear_clip(&pts);
        assert_eq!(tris.len(), 2);
    }

    #[test]
    fn concave_l_shape() {
        // L-shaped polygon: 6 vertices, 4 triangles expected.
        //   (0,0) - (2,0) - (2,1) - (1,1) - (1,2) - (0,2)
        let pts = [
            p(0.0, 0.0),
            p(2.0, 0.0),
            p(2.0, 1.0),
            p(1.0, 1.0),
            p(1.0, 2.0),
            p(0.0, 2.0),
        ];
        let tris = ear_clip(&pts);
        assert_eq!(tris.len(), 4, "L-shape should triangulate into 4 tris");
    }

    #[test]
    fn convex_pentagon_emits_three_triangles() {
        let pts: Vec<Point2> = (0..5)
            .map(|i| {
                let t = i as f64 / 5.0 * std::f64::consts::TAU + std::f64::consts::FRAC_PI_2;
                Point2::new(t.cos(), t.sin())
            })
            .collect();
        let tris = ear_clip(&pts);
        assert_eq!(tris.len(), 3);
    }

    #[test]
    fn degenerate_zero_area_returns_empty() {
        // Three colinear points — area = 0.
        let pts = [p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0)];
        let tris = ear_clip(&pts);
        assert!(tris.is_empty());
    }
}
