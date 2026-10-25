//! Reference check for [`super::holes::merge_holes`]: the bridge search as a
//! plain scan of the whole flat ring (one pass per hole, no index), compared
//! element for element with the indexed ring on seeded random layouts, so the
//! bridge choice under ties stays pinned.

use super::Point2;
use super::predicates::{Sign, orient2d};

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y)
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point2> {
    vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)]
}

/// A clockwise regular polygon.
fn cw_polygon(cx: f64, cy: f64, r: f64, sides: usize, rotation: f64) -> Vec<Point2> {
    let mut ring: Vec<Point2> = (0..sides)
        .map(|i| {
            let a = rotation + std::f64::consts::TAU * i as f64 / sides as f64;
            p(cx + r * a.cos(), cy + r * a.sin())
        })
        .collect();
    ring.reverse();
    ring
}

/// `merge_holes` as it was before the ring was indexed: fold the holes in one
/// at a time over a flat vector, scanning all of it for each bridge.
fn reference_merge(mut outer: Vec<Point2>, mut holes: Vec<Vec<Point2>>) -> Vec<Point2> {
    super::holes::split_touched_edges(&mut outer, &mut holes);
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

/// xorshift64, enough for seeded layouts.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn assert_same(name: &str, outer: Vec<Point2>, holes: Vec<Vec<Point2>>) {
    let want = reference_merge(outer.clone(), holes.clone());
    let got = super::holes::merge_holes(outer, holes);
    assert_eq!(got.len(), want.len(), "{name}");
    if let Some(i) = (0..got.len()).find(|&i| got[i] != want[i]) {
        panic!("{name}: ring differs at {i}: {:?} vs {:?}", got[i], want[i]);
    }
}

#[test]
fn grids_bridge_like_the_full_scan() {
    for side in [1_usize, 2, 5, 9] {
        for (pitch, r) in [(10.0, 3.0), (1.27, 0.4)] {
            for sides in [4_usize, 8, 16] {
                for rot in [0.0, std::f64::consts::PI / 8.0, 0.3] {
                    let size = pitch * (side as f64 + 1.0);
                    let holes = (0..side * side)
                        .map(|k| {
                            let (cx, cy) = (
                                pitch * ((k % side) as f64 + 1.0),
                                pitch * ((k / side) as f64 + 1.0),
                            );
                            cw_polygon(cx, cy, r, sides, rot)
                        })
                        .collect();
                    let name = format!("grid {side} {pitch} {sides} {rot}");
                    assert_same(&name, rect(0.0, 0.0, size, size), holes);
                }
            }
        }
    }
}

#[test]
fn touching_squares_bridge_like_the_full_scan() {
    for side in 1..=6_usize {
        for inset in [0.0, 0.5] {
            let s = 2.0;
            let holes = (0..side * side)
                .map(|k| {
                    let (c, r) = ((k % side) as f64 * s, (k / side) as f64 * s);
                    let mut h = rect(c + inset, r + inset, c + s - inset, r + s - inset);
                    h.reverse();
                    h
                })
                .collect();
            let size = side as f64 * s;
            assert_same(
                &format!("squares {side} {inset}"),
                rect(0.0, 0.0, size, size),
                holes,
            );
        }
    }
}

#[test]
fn random_layouts_bridge_like_the_full_scan() {
    let mut rng = Rng(0x1234_5678_9abc);
    for case in 0..300 {
        let n = 2 + (rng.next() * 6.0) as usize;
        let pitch = 10.0;
        let mut holes = Vec::new();
        for row in 0..n {
            for col in 0..n {
                if rng.next() < 0.6 {
                    let sides = 3 + (rng.next() * 6.0) as usize;
                    let radius = 1.0 + rng.next() * 3.5;
                    let cx = pitch * (col as f64 + 1.0) + (rng.next() - 0.5);
                    let cy = pitch * (row as f64 + 1.0) + (rng.next() - 0.5);
                    holes.push(cw_polygon(cx, cy, radius, sides, rng.next() * 6.0));
                }
            }
        }
        let size = pitch * (n as f64 + 1.0);
        let outer = match case % 4 {
            0 => rect(0.0, 0.0, size, size),
            1 => vec![
                p(0.0, 0.0),
                p(size, 0.0),
                p(size, size),
                p(size / 2.0, size * 0.7),
                p(0.0, size),
            ],
            2 => vec![
                p(0.0, 0.0),
                p(size, size * 0.1),
                p(size * 0.9, size),
                p(size * 0.05, size * 0.8),
            ],
            _ => rect(0.0, 0.0, size * 3.0, size),
        };
        assert_same(&format!("random {case}"), outer, holes);
    }
}

#[test]
fn strips_and_columns_bridge_like_the_full_scan() {
    for count in [3_usize, 40, 200] {
        let strip = (0..count)
            .map(|k| cw_polygon(10.0 * (k as f64 + 1.0), 5.0, 3.0, 16, 0.0))
            .collect();
        assert_same(
            "strip",
            rect(0.0, 0.0, 10.0 * (count as f64 + 1.0), 10.0),
            strip,
        );
        let column = (0..count)
            .map(|k| cw_polygon(5.0, 10.0 * (k as f64 + 1.0), 3.0, 8, 0.0))
            .collect();
        assert_same(
            "column",
            rect(0.0, 0.0, 10.0, 10.0 * (count as f64 + 1.0)),
            column,
        );
    }
}
