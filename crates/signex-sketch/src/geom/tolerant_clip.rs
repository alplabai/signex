//! Degeneracy-tolerant ear clipper for the ring [`super::holes`] builds.
//!
//! The bridged ring carries coincident vertices (every bridge visits its two
//! ends twice), collinear runs and zero-width spikes that the strict
//! [`super::ear_clip`] rejects. This clipper drops the degenerate vertices and
//! clips the rest.
//!
//! An ear is blocked when a vertex that is not one of its corners
//! - is non-convex at the time of the test and inside the ear triangle (edges
//!   included), or
//! - sits at a corner's coordinates, convex or not, and one of its ring edges
//!   runs strictly into the ear's angle there (a bridged ring touches itself
//!   there, so the ear would cover boundary).
//!
//! A convex vertex elsewhere never blocks: it can only touch the ear's
//! boundary, not enter it. The non-convex vertices sit in a [`PointGrid`] of
//! [`super::AabbIndex`] cells and each ear queries just the cells its triangle
//! overlaps instead of scanning the whole ring (a bounding box would reach
//! most of a long thin strip for every thin ear). Vertices that turn
//! non-convex after the build join a short side list. The vertices at the
//! corners are found by coordinates.
//!
//! The forced cut below only runs after no ear passed the test, i.e. on input
//! that is already not a clean partition (the result is documented as
//! inexact).

use super::Point2;
use super::point_grid::PointGrid;
use super::predicates::{Sign, orient2d};

/// Ear-clip a CCW ring that may contain bridge duplicates, collinear runs
/// and zero-width spikes, which are dropped without losing area. If no ear is
/// found the clipper cuts a convex corner anyway rather than give up, so the
/// result may then not partition the ring exactly; callers that need the area
/// exact should check it.
///
/// Ears are clipped smallest first (docs/TOLERANT_EAR_ORDER.md), so a long
/// thin strip never turns into a fan of long slivers that each query a whole
/// row of the grid.
pub(super) fn clip_ears_tolerant(pts: &[Point2]) -> Vec<[usize; 3]> {
    let triangles = clip_smallest_first(pts);
    if covers_exactly(pts, &triangles) {
        return triangles;
    }
    // Rings with antiparallel coincident edges (holes sharing a side) can
    // yield an ear that lies outside the region when it is clipped early;
    // clipping along the ring peels those pairs off first.
    clip_in_ring_order(pts)
}

fn clip_smallest_first(pts: &[Point2]) -> Vec<[usize; 3]> {
    let mut ring = LinkedRing::new(pts);
    let mut triangles = Vec::with_capacity(pts.len().saturating_sub(2));
    ring.settle((0..pts.len()).collect());
    ring.changed.clear();
    let mut heap = EarHeap::default();
    heap.push_all(&ring, 0..pts.len());
    // True while every convex vertex was queued afresh and nothing was
    // clipped since: if all of them are blocked then, no ear is left.
    let mut fresh = true;
    while ring.count >= 3 {
        let Some(b) = heap.pop_valid(&ring) else {
            if !fresh {
                // A blocker may have gone without a neighbour noticing.
                heap.parked.clear();
                heap.push_all(&ring, 0..pts.len());
                fresh = true;
                continue;
            }
            // Forced cut: no ear passed the test.
            let Some(start) = ring.first_alive() else {
                break;
            };
            let Some(b) = ring.walk(start).find(|&v| ring.is_convex(v)) else {
                break;
            };
            clip(&mut ring, &mut heap, &mut triangles, b);
            fresh = false;
            continue;
        };
        match ring.blocker(b) {
            Some(q) => heap.parked.entry(q).or_default().push(b),
            None => {
                clip(&mut ring, &mut heap, &mut triangles, b);
                fresh = false;
            }
        }
    }
    triangles
}

/// The earlier ring-order clipper: the first unblocked convex vertex from the
/// last clip. Quadratic on a long thin strip, which is why it is only the
/// fallback.
fn clip_in_ring_order(pts: &[Point2]) -> Vec<[usize; 3]> {
    let mut ring = LinkedRing::new(pts);
    let mut triangles = Vec::with_capacity(pts.len().saturating_sub(2));
    ring.settle((0..pts.len()).collect());
    ring.changed.clear();
    let mut cursor = ring.first_alive();
    while ring.count >= 3 {
        let Some(start) = cursor else { break };
        let pick = ring
            .walk(start)
            .find(|&v| ring.is_convex(v) && ring.blocker(v).is_none())
            .or_else(|| ring.walk(start).find(|&v| ring.is_convex(v)));
        let Some(b) = pick else { break };
        let (a, c) = (ring.prev[b], ring.next[b]);
        triangles.push([a, b, c]);
        ring.remove(b);
        ring.settle(vec![a, c]);
        let changed = std::mem::take(&mut ring.changed);
        ring.index_new_reflex(&changed);
        cursor = [a, c]
            .into_iter()
            .find(|&v| ring.alive[v])
            .or_else(|| ring.first_alive());
    }
    triangles
}

/// Whether the triangles' signed areas add up to the ring's area, which a
/// clip that covered the outside of the ring cannot do.
fn covers_exactly(pts: &[Point2], triangles: &[[usize; 3]]) -> bool {
    let cross =
        |a: Point2, b: Point2, c: Point2| (b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y);
    let mut ring = 0.0;
    let mut scale = 0.0;
    for (i, &p) in pts.iter().enumerate() {
        let q = pts[(i + 1) % pts.len()];
        ring += p.x * q.y - q.x * p.y;
        scale += (p.x * q.y).abs() + (q.x * p.y).abs();
    }
    let sum: f64 = triangles
        .iter()
        .map(|t| cross(pts[t[0]], pts[t[1]], pts[t[2]]))
        .sum();
    (sum - ring).abs() <= 1e-9 * scale.max(1.0)
}

/// Clip the ear at `b`, then queue the vertices whose ears changed and the
/// ears parked behind a vertex that changed.
fn clip(ring: &mut LinkedRing<'_>, heap: &mut EarHeap, triangles: &mut Vec<[usize; 3]>, b: usize) {
    let (a, c) = (ring.prev[b], ring.next[b]);
    triangles.push([a, b, c]);
    ring.remove(b);
    ring.settle(vec![a, c]);
    let mut wake = std::mem::take(&mut ring.changed);
    for i in 0..wake.len() {
        wake.extend(heap.parked.remove(&wake[i]).unwrap_or_default());
    }
    wake.sort_unstable();
    wake.dedup();
    ring.index_new_reflex(&wake);
    heap.push_all(ring, wake);
}

/// Convex vertices ordered by the size of their ear, smallest first.
#[derive(Default)]
struct EarHeap {
    queue: std::collections::BinaryHeap<std::cmp::Reverse<(u64, usize, u32)>>,
    /// Ears that a vertex blocked, by blocker; re-queued when it changes.
    parked: std::collections::HashMap<usize, Vec<usize>>,
}

impl EarHeap {
    fn push_all(&mut self, ring: &LinkedRing<'_>, vs: impl IntoIterator<Item = usize>) {
        for v in vs {
            if ring.alive[v] && ring.is_convex(v) {
                let key = ring.ear_size(v).to_bits();
                self.queue
                    .push(std::cmp::Reverse((key, v, ring.version[v])));
            }
        }
    }

    /// The smallest queued ear whose corner has not changed since queuing.
    fn pop_valid(&mut self, ring: &LinkedRing<'_>) -> Option<usize> {
        while let Some(std::cmp::Reverse((_, v, version))) = self.queue.pop() {
            if ring.alive[v] && ring.version[v] == version {
                return Some(v);
            }
        }
        None
    }
}

/// The ring as a doubly linked list over the point indices, so removing an
/// ear is O(1).
struct LinkedRing<'a> {
    pts: &'a [Point2],
    prev: Vec<usize>,
    next: Vec<usize>,
    alive: Vec<bool>,
    count: usize,
    /// Vertices that were non-convex when the ring was built.
    reflex: PointGrid<usize>,
    /// Whether the vertex is in `reflex` or `late`.
    indexed: Vec<bool>,
    /// Vertices that turned non-convex after the build (clipping in another
    /// order can do that), too few to be worth a grid.
    late: Vec<usize>,
    /// Every vertex by coordinates: a vertex at an ear corner blocks by its
    /// edges, convex or not.
    at: std::collections::HashMap<(u64, u64), Vec<usize>>,
    /// The vertex that blocked the most recent blocked ear.
    last_blocker: std::cell::Cell<Option<usize>>,
    /// Bumped when a vertex's neighbours change, which stales its queued ear.
    version: Vec<u32>,
    /// Vertices whose neighbours changed since the last drain.
    changed: Vec<usize>,
}

impl<'a> LinkedRing<'a> {
    fn new(pts: &'a [Point2]) -> Self {
        let n = pts.len();
        let mut ring = Self {
            pts,
            prev: (0..n).map(|i| (i + n - 1) % n.max(1)).collect(),
            next: (0..n).map(|i| (i + 1) % n.max(1)).collect(),
            alive: vec![true; n],
            count: n,
            reflex: PointGrid::empty(),
            indexed: vec![false; n],
            late: Vec::new(),
            at: std::collections::HashMap::new(),
            last_blocker: std::cell::Cell::new(None),
            version: vec![0; n],
            changed: Vec::new(),
        };
        let reflex: Vec<usize> = (0..n).filter(|&v| !ring.is_convex(v)).collect();
        for &v in &reflex {
            ring.indexed[v] = true;
        }
        for (v, &p) in pts.iter().enumerate() {
            ring.at.entry(coord_key(p)).or_default().push(v);
        }
        ring.reflex = PointGrid::new(reflex.into_iter().map(|v| (pts[v], v)).collect());
        ring
    }

    /// Start treating vertices among `vs` that are now non-convex as blockers.
    fn index_new_reflex(&mut self, vs: &[usize]) {
        for &v in vs {
            if self.alive[v] && !self.indexed[v] && !self.is_convex(v) {
                self.indexed[v] = true;
                self.late.push(v);
            }
        }
    }

    /// Vertices at the three corners of `tri`.
    fn at_corners(&self, tri: [Point2; 3]) -> impl Iterator<Item = &usize> + '_ {
        tri.into_iter()
            .filter_map(|p| self.at.get(&coord_key(p)))
            .flatten()
    }

    fn first_alive(&self) -> Option<usize> {
        self.alive.iter().position(|&a| a)
    }

    /// The live vertices once around the ring, starting at `start`.
    fn walk(&self, start: usize) -> impl Iterator<Item = usize> + '_ {
        let mut at = Some(start);
        std::iter::from_fn(move || {
            let v = at?;
            let following = self.next[v];
            at = (following != start).then_some(following);
            Some(v)
        })
    }

    fn remove(&mut self, v: usize) {
        let (p, n) = (self.prev[v], self.next[v]);
        self.next[p] = n;
        self.prev[n] = p;
        self.alive[v] = false;
        self.count -= 1;
        for u in [p, n, v] {
            self.version[u] += 1;
            self.changed.push(u);
        }
    }

    /// Squared length of the longest side of the ear at `v`; the clip order.
    fn ear_size(&self, v: usize) -> f64 {
        let (a, b, c) = (self.pts[self.prev[v]], self.pts[v], self.pts[self.next[v]]);
        let d = |p: Point2, q: Point2| (p.x - q.x).powi(2) + (p.y - q.y).powi(2);
        let size = d(a, b).max(d(b, c)).max(d(c, a));
        if size.is_finite() { size } else { f64::MAX }
    }

    /// Drop every vertex among `seeds` (and, transitively, their neighbours)
    /// that coincides with its successor or is collinear with its neighbours,
    /// which includes zero-width spikes.
    fn settle(&mut self, mut seeds: Vec<usize>) {
        while let Some(v) = seeds.pop() {
            if self.count < 3 {
                return;
            }
            if !self.alive[v] {
                continue;
            }
            let (p, n) = (self.prev[v], self.next[v]);
            let (a, b, c) = (self.pts[p], self.pts[v], self.pts[n]);
            if b == c || orient2d(a, b, c) == Sign::Zero {
                self.remove(v);
                seeds.extend([p, n]);
            }
        }
    }

    fn is_convex(&self, v: usize) -> bool {
        let (a, b, c) = (self.pts[self.prev[v]], self.pts[v], self.pts[self.next[v]]);
        orient2d(a, b, c) == Sign::Positive
    }

    /// A vertex that stops the ear at `b` from being clipped, if any.
    fn blocker(&self, b: usize) -> Option<usize> {
        let (a, c) = (self.prev[b], self.next[b]);
        let tri = [self.pts[a], self.pts[b], self.pts[c]];
        // Neighbouring ears are mostly blocked by the same vertex, so try the
        // last one first.
        let found = match self.last_blocker.get() {
            Some(q) if self.blocks(q, [a, b, c], tri) => Some(q),
            _ => self
                .at_corners(tri)
                .chain(self.reflex.query_triangle(tri))
                .chain(&self.late)
                .find(|&&q| self.blocks(q, [a, b, c], tri))
                .copied(),
        };
        // Every query in the tests is checked against the plain box query.
        #[cfg(test)]
        {
            let region = super::aabb_index::Aabb::from_points(&tri).expect("three corners");
            let reference = self
                .reflex
                .query(region)
                .chain(self.at_corners(tri))
                .chain(&self.late)
                .any(|&q| self.blocks_geometry(q, [a, b, c], tri));
            assert_eq!(
                found.is_some(),
                reference,
                "triangle query disagrees with the box query"
            );
        }
        if found.is_some() {
            self.last_blocker.set(found);
        }
        found
    }

    /// Whether vertex `q` stops the ear `tri` (corners `[a, b, c]`).
    fn blocks(&self, q: usize, [a, b, c]: [usize; 3], tri: [Point2; 3]) -> bool {
        #[cfg(test)]
        CANDIDATES_TESTED.with(|n| n.set(n.get() + 1));
        self.blocks_geometry(q, [a, b, c], tri)
    }

    fn blocks_geometry(&self, q: usize, [a, b, c]: [usize; 3], tri: [Point2; 3]) -> bool {
        if !self.alive[q] || [a, b, c].contains(&q) {
            return false;
        }
        // The geometric test goes first: it rejects nearly every candidate of
        // a long thin ear and is cheaper than the convexity test.
        match tri.iter().position(|&corner| corner == self.pts[q]) {
            // Convex or not, an edge running into the ear covers boundary.
            Some(k) => self.runs_into_ear(q, tri, k),
            None => in_triangle(self.pts[q], tri) && !self.is_convex(q),
        }
    }

    /// Whether a ring edge leaving `q`, which sits at corner `k` of the ear
    /// `tri`, points strictly into the ear's interior angle there.
    fn runs_into_ear(&self, q: usize, tri: [Point2; 3], k: usize) -> bool {
        let (before, corner, after) = (tri[(k + 2) % 3], tri[k], tri[(k + 1) % 3]);
        [self.prev[q], self.next[q]].into_iter().any(|e| {
            let to = self.pts[e];
            orient2d(before, corner, to) == Sign::Positive
                && orient2d(corner, after, to) == Sign::Positive
        })
    }
}

/// Hash key of a point; `+ 0.0` folds -0.0 into 0.0 so equal points agree.
fn coord_key(p: Point2) -> (u64, u64) {
    ((p.x + 0.0).to_bits(), (p.y + 0.0).to_bits())
}

/// Inclusive point-in-triangle for a triangle of either winding.
fn in_triangle(p: Point2, [a, b, c]: [Point2; 3]) -> bool {
    let s = [orient2d(a, b, p), orient2d(b, c, p), orient2d(c, a, p)];
    !(s.contains(&Sign::Positive) && s.contains(&Sign::Negative))
}

#[cfg(test)]
thread_local! {
    /// Reflex vertices the blocking test looked at, for the cost bound in
    /// the strip test.
    pub(super) static CANDIDATES_TESTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
