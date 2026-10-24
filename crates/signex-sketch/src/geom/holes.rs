//! Hole bridging and a degeneracy-tolerant ear clipper for polygons with
//! cut-outs.
//!
//! Rows and grids of rectangular holes, holes sharing an outer edge and
//! equal-x holes are ordinary pour geometry, and they leave collinear and
//! coincident vertices in the bridged ring that the strict [`super::ear_clip`]
//! rejects. This module bridges holes with the visible-vertex method and
//! clips the result with a tolerant ear test (see [`super::tolerant_clip`]).
//!
//! Holes may touch the outer ring or each other (at a vertex, or along part of
//! an edge): rings are first given matching vertices where one touches the
//! interior of another's edge, and a hole whose rightmost vertex already sits
//! on the ring is bridged there with a zero-length bridge. `Zone.holes`
//! guarantees strictly interior, disjoint holes and the app drops the rest
//! (#901); this stays correct for direct callers. Holes that overlap or cross
//! are not supported and can yield an inexact triangulation.

use super::Point2;
use super::aabb_index::Aabb;
use super::point_grid::PointGrid;
use super::predicates::{Sign, orient2d};

/// Bridge every hole into `outer` (CCW) and return the merged ring.
///
/// Holes (CW) are bridged in descending rightmost-x order. A hole is joined
/// at its rightmost vertex `M` to a vertex visible from it, found by casting
/// a ray from `M` towards +x: any hole not yet bridged lies entirely left of
/// `M.x`, so it cannot stand between `M` and the ray's first hit, and the
/// bridge never crosses an unbridged hole.
pub(super) fn merge_holes(mut outer: Vec<Point2>, mut holes: Vec<Vec<Point2>>) -> Vec<Point2> {
    split_touched_edges(&mut outer, &mut holes);
    let max_x = |h: &[Point2]| h.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
    holes.sort_by(|a, b| {
        max_x(b)
            .partial_cmp(&max_x(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    holes
        .iter()
        .fold(outer, |ring, hole| bridge_hole(&ring, hole))
}

/// Splice `hole` into `ring` with a zero-width bridge from the hole's
/// rightmost vertex to the ring vertex visible along the +x ray.
///
/// A hole whose rightmost vertex already lies on the ring (it touches the
/// outline, or a hole bridged earlier) needs no ray: the ray would run along
/// the touched edge and bridge to its far end, laying the hole's boundary over
/// the ring's. The bridge is then zero length, at that point.
fn bridge_hole(ring: &[Point2], hole: &[Point2]) -> Vec<Point2> {
    let mut hole_idx = 0;
    for (i, p) in hole.iter().enumerate().skip(1) {
        let best = hole[hole_idx];
        if p.x > best.x || (p.x == best.x && p.y < best.y) {
            hole_idx = i;
        }
    }
    let m = hole[hole_idx];
    let around = [
        hole[(hole_idx + hole.len() - 1) % hole.len()],
        hole[(hole_idx + 1) % hole.len()],
    ];
    let ring_idx = coincident_vertex(ring, m, around)
        .or_else(|| visible_vertex(ring, m))
        .unwrap_or_else(|| nearest_vertex(ring, m));

    let mut merged = Vec::with_capacity(ring.len() + hole.len() + 2);
    merged.extend_from_slice(&ring[..=ring_idx]);
    merged.extend_from_slice(&hole[hole_idx..]);
    merged.extend_from_slice(&hole[..=hole_idx]);
    merged.extend_from_slice(&ring[ring_idx..]);
    merged
}

/// The ring vertex `m` coincides with, if any: a hole vertex that touches the
/// ring. `around` are the hole's two neighbours of `m`; among several
/// coincident ring copies (earlier bridges leave them) the one that opens
/// towards both is taken.
fn coincident_vertex(ring: &[Point2], m: Point2, around: [Point2; 2]) -> Option<usize> {
    let same = |&j: &usize| ring[j] == m;
    let open_towards = |&j: &usize| around.iter().all(|&q| in_cone(ring, j, q));
    let n = ring.len();
    (0..n)
        .filter(same)
        .find(open_towards)
        .or_else(|| (0..n).find(same))
}

/// Insert every vertex of one ring that lies in the interior of another
/// ring's edge into that edge. Rings that touch along part of an edge (a hole
/// sharing a stretch of the outline, two holes sharing a stretch of side)
/// then carry matching vertices on both sides, so the bridged ring's
/// overlapping boundary pieces cancel pairwise in the clipper instead of
/// forming T-junctions it cannot resolve. Every vertex goes into a
/// [`PointGrid`], and each edge asks only for the vertices under its own
/// bounding box, so the cost follows the edges' local neighbourhoods rather
/// than outline length times hole count.
fn split_touched_edges(outer: &mut Vec<Point2>, holes: &mut [Vec<Point2>]) {
    let items: Vec<(Point2, usize)> = std::iter::once(&*outer)
        .chain(holes.iter())
        .enumerate()
        .flat_map(|(id, ring)| ring.iter().map(move |&q| (q, id)))
        .collect();
    let grid = PointGrid::new(items.iter().map(|&(q, id)| (q, (id, q))).collect());
    for (id, ring) in std::iter::once(outer).chain(holes.iter_mut()).enumerate() {
        *ring = with_touching_vertices(ring, id, &grid);
    }
}

/// `ring` (number `id`) with each vertex of another ring that lies in the
/// interior of one of its edges inserted into that edge, in order along it.
fn with_touching_vertices(
    ring: &[Point2],
    id: usize,
    grid: &PointGrid<(usize, Point2)>,
) -> Vec<Point2> {
    let mut out = Vec::with_capacity(ring.len());
    for (i, &a) in ring.iter().enumerate() {
        let b = ring[(i + 1) % ring.len()];
        out.push(a);
        let edge_box = Aabb::new(
            Point2::new(a.x.min(b.x), a.y.min(b.y)),
            Point2::new(a.x.max(b.x), a.y.max(b.y)),
        );
        let mut on_edge: Vec<Point2> = grid
            .query(edge_box)
            .filter(|&&(other, q)| other != id && on_open_segment(a, b, q))
            .map(|&(_, q)| q)
            .collect();
        on_edge.sort_by(|p, q| {
            a.distance_sq(*p)
                .partial_cmp(&a.distance_sq(*q))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        on_edge.dedup();
        out.extend(on_edge);
    }
    out
}

/// Whether `m` lies strictly between the endpoints of segment `a`-`b`.
fn on_open_segment(a: Point2, b: Point2, m: Point2) -> bool {
    m.x >= a.x.min(b.x)
        && m.x <= a.x.max(b.x)
        && m.y >= a.y.min(b.y)
        && m.y <= a.y.max(b.y)
        && m != a
        && m != b
        && orient2d(a, b, m) == Sign::Zero
}

fn nearest_vertex(ring: &[Point2], m: Point2) -> usize {
    let d = |p: Point2| (p.x - m.x).powi(2) + (p.y - m.y).powi(2);
    (0..ring.len())
        .min_by(|&a, &b| {
            d(ring[a])
                .partial_cmp(&d(ring[b]))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(0)
}

/// The ring position of a vertex visible from `m`: the first ring edge (or
/// vertex) the +x ray from `m` hits, resolved to its larger-x endpoint, then
/// swapped for any reflex vertex inside the triangle `m`, hit point,
/// endpoint that makes a smaller angle with the ray. Candidates must also
/// open towards `m` (`in_cone`), which picks the right copy among the
/// coincident vertices earlier bridges leave behind.
fn visible_vertex(ring: &[Point2], m: Point2) -> Option<usize> {
    let n = ring.len();
    // (hit x, ring position of the endpoint to start from, hit is a vertex)
    let mut best: Option<(f64, usize, bool)> = None;
    let mut consider = |x: f64, pos: usize, is_vertex: bool| {
        let better = match best {
            None => true,
            Some((bx, _, bv)) => x < bx || (x == bx && is_vertex && !bv),
        };
        if x >= m.x && better {
            best = Some((x, pos, is_vertex));
        }
    };
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if (a.y > m.y && b.y > m.y) || (a.y < m.y && b.y < m.y) {
            continue;
        }
        if a.y == b.y {
            // Edge lying on the ray: its endpoints are vertex hits.
            consider(a.x, i, true);
            consider(b.x, (i + 1) % n, true);
            continue;
        }
        let t = (m.y - a.y) / (b.y - a.y);
        let x = a.x + t * (b.x - a.x);
        if a.y == m.y {
            consider(a.x, i, true);
        } else if b.y == m.y {
            consider(b.x, (i + 1) % n, true);
        } else {
            let pos = if a.x >= b.x { i } else { (i + 1) % n };
            consider(x, pos, false);
        }
    }
    let (hit_x, pos, is_vertex) = best?;

    // Prefer a copy of the endpoint's coordinates that opens towards `m`.
    let endpoint = ring[pos];
    let copy = (0..n)
        .filter(|&j| ring[j] == endpoint)
        .find(|&j| in_cone(ring, j, m))
        .unwrap_or(pos);
    if is_vertex {
        return Some(copy);
    }

    let hit = Point2::new(hit_x, m.y);
    // A ray passing a few ulp off the endpoint makes the triangle degenerate:
    // `in_triangle` then accepts every vertex on its line, including ones past
    // the hit, and the bridge could cross the hole between. Nothing can stand
    // between `m` and a hit that close to the endpoint, so bridge to it.
    if orient2d(m, hit, endpoint) == Sign::Zero {
        return Some(copy);
    }
    let x_limit = hit_x.max(endpoint.x);
    let mut chosen = copy;
    let mut best_key = f64::INFINITY;
    for j in 0..n {
        let q = ring[j];
        if q == endpoint || q.x <= m.x || q.x > x_limit || !in_triangle(q, m, hit, endpoint) {
            continue;
        }
        let prev = ring[(j + n - 1) % n];
        let next = ring[(j + 1) % n];
        if orient2d(prev, q, next) == Sign::Positive || !in_cone(ring, j, m) {
            continue;
        }
        let key = (q.y - m.y).abs() / (q.x - m.x);
        if key < best_key {
            best_key = key;
            chosen = j;
        }
    }
    Some(chosen)
}

/// Whether the segment from ring vertex `j` to `m` leaves `j` into the
/// ring's interior (the ring is CCW, so the interior is on the left).
fn in_cone(ring: &[Point2], j: usize, m: Point2) -> bool {
    let n = ring.len();
    let (prev, v, next) = (ring[(j + n - 1) % n], ring[j], ring[(j + 1) % n]);
    let left_of_in = orient2d(prev, v, m) != Sign::Negative;
    let left_of_out = orient2d(v, next, m) != Sign::Negative;
    if orient2d(prev, v, next) == Sign::Positive {
        left_of_in && left_of_out
    } else {
        left_of_in || left_of_out
    }
}

/// Inclusive point-in-triangle for a triangle of either winding.
fn in_triangle(p: Point2, a: Point2, b: Point2, c: Point2) -> bool {
    let s = [orient2d(a, b, p), orient2d(b, c, p), orient2d(c, a, p)];
    !(s.contains(&Sign::Positive) && s.contains(&Sign::Negative))
}

#[cfg(test)]
mod tests {
    use super::super::point_in_polygon;
    use super::super::predicates::signed_area;
    use super::super::triangulate::ear_clip_with_holes;
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point2> {
        vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)]
    }

    /// Triangulate and require an exact partition of outer minus holes.
    fn assert_partitions(outer: &[Point2], holes: &[Vec<Point2>]) {
        let (merged, tris) = ear_clip_with_holes(outer, holes);
        let expected =
            signed_area(outer).abs() - holes.iter().map(|h| signed_area(h).abs()).sum::<f64>();
        let total: f64 = tris
            .iter()
            .map(|&[a, b, c]| signed_area(&[merged[a], merged[b], merged[c]]).abs())
            .sum();
        assert!(
            !tris.is_empty() && (total - expected).abs() <= expected * 1e-9,
            "triangle area {total}, want {expected}"
        );
        for &[a, b, c] in &tris {
            let m = p(
                (merged[a].x + merged[b].x + merged[c].x) / 3.0,
                (merged[a].y + merged[b].y + merged[c].y) / 3.0,
            );
            assert!(point_in_polygon(m, outer), "centroid {m:?} outside outer");
            assert!(
                holes.iter().all(|h| !point_in_polygon(m, h)),
                "centroid {m:?} inside a hole"
            );
        }
    }

    #[test]
    fn stacked_holes_with_equal_max_x() {
        assert_partitions(
            &rect(0.0, 0.0, 10.0, 20.0),
            &[rect(4.0, 2.0, 6.0, 4.0), rect(4.0, 8.0, 6.0, 10.0)],
        );
    }

    #[test]
    fn grids_of_square_holes() {
        for side in [2_usize, 6] {
            let pitch = 10.0;
            let size = pitch * (side as f64 + 1.0);
            let holes: Vec<_> = (0..side)
                .flat_map(|r| (0..side).map(move |c| (r, c)))
                .map(|(r, c)| {
                    let (x, y) = (
                        pitch * (c as f64 + 0.5) + 2.0,
                        pitch * (r as f64 + 0.5) + 2.0,
                    );
                    rect(x, y, x + 5.0, y + 5.0)
                })
                .collect();
            assert_partitions(&rect(0.0, 0.0, size, size), &holes);
        }
    }

    #[test]
    fn hole_sharing_part_of_an_outer_edge() {
        assert_partitions(&rect(0.0, 0.0, 10.0, 10.0), &[rect(4.0, 4.0, 10.0, 6.0)]);
    }

    #[test]
    fn hole_touching_an_outer_corner() {
        assert_partitions(&rect(0.0, 0.0, 10.0, 10.0), &[rect(8.0, 8.0, 10.0, 10.0)]);
        assert_partitions(
            &rect(0.0, 0.0, 10.0, 10.0),
            &[vec![p(0.0, 0.0), p(3.0, 1.0), p(1.0, 3.0)]],
        );
    }

    #[test]
    fn bridge_does_not_cross_a_hole_that_is_not_bridged_yet() {
        let outer = vec![
            p(0.0, 0.0),
            p(60.0, 0.0),
            p(60.0, 20.0),
            p(0.0, 20.0),
            p(0.0, 5.0),
        ];
        let sliver = vec![p(7.0, 4.5), p(8.0, 5.0), p(7.0, 5.5)];
        assert_partitions(&outer, &[sliver, rect(2.0, 1.0, 6.0, 6.0)]);
    }
}
