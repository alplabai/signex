//! Hole bridging and a degeneracy-tolerant ear clipper for polygons with
//! cut-outs.
//!
//! Rows and grids of rectangular holes, holes sharing an outer edge and
//! equal-x holes are ordinary pour geometry, and they leave collinear and
//! coincident vertices in the bridged ring that the strict [`super::ear_clip`]
//! rejects. This module bridges holes with the visible-vertex method and
//! clips the result with a tolerant ear test.

use super::Point2;
use super::predicates::{Sign, orient2d};

/// Bridge every hole into `outer` (CCW) and return the merged ring.
///
/// Holes (CW) are bridged in descending rightmost-x order. A hole is joined
/// at its rightmost vertex `M` to a vertex visible from it, found by casting
/// a ray from `M` towards +x: any hole not yet bridged lies entirely left of
/// `M.x`, so it cannot stand between `M` and the ray's first hit, and the
/// bridge never crosses an unbridged hole.
pub(super) fn merge_holes(outer: Vec<Point2>, mut holes: Vec<Vec<Point2>>) -> Vec<Point2> {
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
fn bridge_hole(ring: &[Point2], hole: &[Point2]) -> Vec<Point2> {
    let mut hole_idx = 0;
    for (i, p) in hole.iter().enumerate().skip(1) {
        let best = hole[hole_idx];
        if p.x > best.x || (p.x == best.x && p.y < best.y) {
            hole_idx = i;
        }
    }
    let m = hole[hole_idx];
    let ring_idx = visible_vertex(ring, m).unwrap_or_else(|| nearest_vertex(ring, m));

    let mut merged = Vec::with_capacity(ring.len() + hole.len() + 2);
    merged.extend_from_slice(&ring[..=ring_idx]);
    merged.extend_from_slice(&hole[hole_idx..]);
    merged.extend_from_slice(&hole[..=hole_idx]);
    merged.extend_from_slice(&ring[ring_idx..]);
    merged
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
    let mut chosen = copy;
    let mut best_key = f64::INFINITY;
    for j in 0..n {
        let q = ring[j];
        if q == endpoint || q.x <= m.x || !in_triangle(q, m, hit, endpoint) {
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

/// Ear-clip a CCW ring that may contain bridge duplicates, collinear runs
/// and zero-width spikes, which are dropped without losing area. A vertex
/// blocks an ear only when it is non-convex and inside the ear triangle
/// (edges included) at coordinates distinct from the ear's corners. If no
/// ear is found the clipper cuts a convex corner anyway rather than give up,
/// so the result may then not partition the ring exactly; callers that need
/// the area exact should check it.
pub(super) fn clip_ears_tolerant(pts: &[Point2]) -> Vec<[usize; 3]> {
    let mut ring: Vec<usize> = (0..pts.len()).collect();
    let mut triangles = Vec::with_capacity(pts.len().saturating_sub(2));
    let mut cursor = 0;
    loop {
        drop_degenerate(pts, &mut ring);
        let n = ring.len();
        if n < 3 {
            break;
        }
        let at = |i: usize| (ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]);
        let pick = (0..n)
            .map(|k| (cursor + k) % n)
            .find(|&i| {
                let (a, b, c) = at(i);
                orient2d(pts[a], pts[b], pts[c]) == Sign::Positive && !is_blocked(pts, &ring, i)
            })
            .or_else(|| {
                (0..n).find(|&i| {
                    let (a, b, c) = at(i);
                    orient2d(pts[a], pts[b], pts[c]) == Sign::Positive
                })
            });
        let Some(i) = pick else { break };
        let (a, b, c) = at(i);
        triangles.push([a, b, c]);
        ring.remove(i);
        cursor = i.saturating_sub(1);
    }
    triangles
}

/// Remove vertices that coincide with their successor or are collinear with
/// their neighbours (this includes zero-width spikes).
fn drop_degenerate(pts: &[Point2], ring: &mut Vec<usize>) {
    loop {
        let n = ring.len();
        if n < 3 {
            return;
        }
        let bad = (0..n).find(|&i| {
            let (a, b, c) = (
                pts[ring[(i + n - 1) % n]],
                pts[ring[i]],
                pts[ring[(i + 1) % n]],
            );
            b == c || orient2d(a, b, c) == Sign::Zero
        });
        match bad {
            Some(i) => {
                ring.remove(i);
            }
            None => return,
        }
    }
}

fn is_blocked(pts: &[Point2], ring: &[usize], i: usize) -> bool {
    let n = ring.len();
    let (pa, pb, pc) = (ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]);
    let (a, b, c) = (pts[pa], pts[pb], pts[pc]);
    (0..n).any(|j| {
        let (prev, cur, next) = (ring[(j + n - 1) % n], ring[j], ring[(j + 1) % n]);
        if cur == pa || cur == pb || cur == pc {
            return false;
        }
        let q = pts[cur];
        if q == a || q == b || q == c {
            return false;
        }
        orient2d(pts[prev], q, pts[next]) != Sign::Positive && in_triangle(q, a, b, c)
    })
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
