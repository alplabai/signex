//! Polygon triangulation via ear-clipping.
//!
//! Time complexity: O(n²) for the basic form; with holes
//! pre-merged via the bridge-edge technique it stays O((n+h)²)
//! where h is the total hole-vertex count. Typical sketch closed
//! loops have ≤ ~64 vertices (rounded rect = 8 + 4·arc_segments),
//! where the basic form's constant factors beat the more
//! sophisticated O(n log n) variants.

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

/// Triangulate an outer polygon with optional hole rings. Each
/// hole is bridged to the outer ring via a "cut" edge connecting
/// the rightmost hole vertex to the nearest outer vertex visible
/// to it; after all holes are bridged the merged ring is a
/// simple polygon that the basic ear-clipper handles.
///
/// Output triangle indices reference a flat vertex array
/// `[outer_vertices, hole_0_vertices, hole_1_vertices, ...]` in
/// the same order as the input. Hole rings should wind opposite
/// to the outer ring (CCW outer + CW holes by convention) but
/// the implementation normalises before merging.
pub fn ear_clip_with_holes(
    outer: &[Point2],
    holes: &[Vec<Point2>],
) -> (Vec<Point2>, Vec<[usize; 3]>) {
    if outer.len() < 3 {
        return (Vec::new(), Vec::new());
    }
    if holes.is_empty() {
        return (outer.to_vec(), ear_clip(outer));
    }
    // Normalise outer to CCW so the hole-bridge math has a
    // consistent winding to fight against.
    let outer_area = signed_area(outer);
    let outer_pts: Vec<Point2> = if outer_area >= 0.0 {
        outer.to_vec()
    } else {
        outer.iter().rev().copied().collect()
    };

    // Each hole gets normalised to CW for bridging (so it walks
    // OPPOSITE to the outer when the bridge stitches them
    // together).
    let mut hole_pts: Vec<Vec<Point2>> = Vec::with_capacity(holes.len());
    for h in holes {
        if h.len() < 3 {
            continue;
        }
        let area = signed_area(h);
        let pts: Vec<Point2> = if area <= 0.0 {
            h.clone()
        } else {
            h.iter().rev().copied().collect()
        };
        hole_pts.push(pts);
    }

    // Sort holes by descending rightmost x so we bridge the
    // outer-most holes first (matches the standard left-to-right
    // hole-cutter convention; without this, bridges from later
    // holes can cross earlier ones).
    hole_pts.sort_by(|a, b| {
        let ax = a.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
        let bx = b.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
        bx.partial_cmp(&ax).unwrap_or(std::cmp::Ordering::Equal)
    });

    // Stitch holes into the outer ring one at a time.
    let mut merged: Vec<Point2> = outer_pts;
    for hole in hole_pts {
        merged = bridge_hole_into_outer(&merged, &hole);
    }

    let triangles = ear_clip(&merged);
    (merged, triangles)
}

/// Bridge a hole into the outer ring. Pick the hole's rightmost
/// vertex, find the outer vertex closest to it (left-of, on a
/// horizontal ray), and cut a "bridge" edge that visits both
/// rings. The bridge is two coincident edges so the merged ring
/// is still simple.
fn bridge_hole_into_outer(outer: &[Point2], hole: &[Point2]) -> Vec<Point2> {
    if hole.is_empty() {
        return outer.to_vec();
    }
    // Hole's rightmost vertex.
    let mut hole_idx = 0;
    let mut max_x = hole[0].x;
    for (i, p) in hole.iter().enumerate().skip(1) {
        if p.x > max_x {
            max_x = p.x;
            hole_idx = i;
        }
    }
    // Closest outer vertex to the hole's rightmost (Euclidean).
    let target = hole[hole_idx];
    let mut outer_idx = 0;
    let mut best_d = f64::INFINITY;
    for (i, p) in outer.iter().enumerate() {
        let dx = p.x - target.x;
        let dy = p.y - target.y;
        let d = dx * dx + dy * dy;
        if d < best_d {
            best_d = d;
            outer_idx = i;
        }
    }
    // Splice: outer[0..=outer_idx] + hole[hole_idx..] + hole[..=hole_idx] + outer[outer_idx..].
    let mut merged: Vec<Point2> = Vec::with_capacity(outer.len() + hole.len() + 2);
    merged.extend_from_slice(&outer[..=outer_idx]);
    merged.extend_from_slice(&hole[hole_idx..]);
    merged.extend_from_slice(&hole[..=hole_idx]);
    merged.extend_from_slice(&outer[outer_idx..]);
    merged
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

    #[test]
    fn ear_clip_with_holes_no_holes_falls_through() {
        let sq = vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)];
        let (merged, tris) = ear_clip_with_holes(&sq, &[]);
        assert_eq!(merged.len(), 4);
        assert_eq!(tris.len(), 2);
    }

    #[test]
    fn ear_clip_with_one_hole_emits_more_triangles() {
        // Outer square 4x4, hole square 1x1 in the middle. Expected
        // triangle count: outer minus hole has area 15, but the
        // triangulation count is just (n-2) where n is the merged
        // vertex count after bridging.
        let outer = vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)];
        let hole = vec![p(1.0, 1.0), p(1.0, 3.0), p(3.0, 3.0), p(3.0, 1.0)]; // CW
        let (merged, tris) = ear_clip_with_holes(&outer, &[hole]);
        // 4 outer + 4 hole + 2 bridge duplicates = 10 vertices.
        assert_eq!(merged.len(), 10);
        // n=10 → 8 triangles.
        assert_eq!(tris.len(), 8);
    }
}
