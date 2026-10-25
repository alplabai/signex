//! Degeneracy-tolerant ear clipper for the ring [`super::holes`] builds.
//!
//! The bridged ring carries coincident vertices (every bridge visits its two
//! ends twice), collinear runs and zero-width spikes that the strict
//! [`super::ear_clip`] rejects. This clipper drops the degenerate vertices and
//! clips the rest.
//!
//! An ear is blocked when a vertex that is not one of its corners, and that
//! is non-convex at the time of the test,
//! - is inside the ear triangle (edges included), or
//! - sits at a corner's coordinates and one of its ring edges runs strictly
//!   into the ear's angle there (a bridged ring touches itself there, so the
//!   ear would cover boundary).
//!
//! A vertex that is currently convex never blocks, coincident or not: it can
//! only touch the ear's boundary, not enter it. Only vertices that were
//! non-convex when the ring was built are treated as blockers. Clipping an ear
//! does not turn a convex neighbour non-convex; removing a degenerate spike
//! tip in `settle` changes a neighbour's direction and is not shown to keep
//! that, so the index is a working assumption there rather than a proven
//! invariant. The indexed vertices sit in a [`PointGrid`] of
//! [`super::AabbIndex`] cells and each ear queries just the cells its triangle
//! overlaps instead of scanning the whole ring (a bounding box would reach
//! most of a long thin strip for every thin ear).
//!
//! The one exception is the forced cut below, which only runs after no ear
//! passed the test, i.e. on input that is already not a clean partition (the
//! result is documented as inexact). A vertex that a forced cut turns
//! non-convex is not added to the index.

use super::Point2;
use super::point_grid::PointGrid;
use super::predicates::{Sign, orient2d};

/// Ear-clip a CCW ring that may contain bridge duplicates, collinear runs
/// and zero-width spikes, which are dropped without losing area. If no ear is
/// found the clipper cuts a convex corner anyway rather than give up, so the
/// result may then not partition the ring exactly; callers that need the area
/// exact should check it.
pub(super) fn clip_ears_tolerant(pts: &[Point2]) -> Vec<[usize; 3]> {
    let triangles = clip_ring(LinkedRing::new(pts));
    // Every clip in the tests also runs with the original bounding-box query
    // and must give the same triangles.
    #[cfg(test)]
    {
        let mut reference = LinkedRing::new(pts);
        reference.box_reference = true;
        assert_eq!(
            triangles,
            clip_ring(reference),
            "the triangle query changed the triangulation"
        );
    }
    triangles
}

fn clip_ring(mut ring: LinkedRing<'_>) -> Vec<[usize; 3]> {
    let pts = ring.pts;
    let mut triangles = Vec::with_capacity(pts.len().saturating_sub(2));
    ring.settle((0..pts.len()).collect());
    let mut cursor = ring.first_alive();
    while ring.count >= 3 {
        let Some(start) = cursor else { break };
        let pick = ring
            .walk(start)
            .find(|&v| ring.is_convex(v) && !ring.is_blocked(v))
            .or_else(|| ring.walk(start).find(|&v| ring.is_convex(v)));
        let Some(b) = pick else { break };
        let (a, c) = (ring.prev[b], ring.next[b]);
        triangles.push([a, b, c]);
        ring.remove(b);
        ring.settle(vec![a, c]);
        cursor = [a, c]
            .into_iter()
            .find(|&v| ring.alive[v])
            .or_else(|| ring.first_alive());
    }
    triangles
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
    /// The vertex that blocked the most recent blocked ear.
    last_blocker: std::cell::Cell<Option<usize>>,
    /// Test-only: answer `is_blocked` with the original box query.
    #[cfg(test)]
    box_reference: bool,
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
            last_blocker: std::cell::Cell::new(None),
            #[cfg(test)]
            box_reference: false,
        };
        let reflex: Vec<usize> = (0..n).filter(|&v| !ring.is_convex(v)).collect();
        ring.reflex = PointGrid::new(reflex.into_iter().map(|v| (pts[v], v)).collect());
        ring
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

    /// Whether the ear at `b` would cover ring boundary.
    fn is_blocked(&self, b: usize) -> bool {
        let (a, c) = (self.prev[b], self.next[b]);
        let tri = [self.pts[a], self.pts[b], self.pts[c]];
        #[cfg(test)]
        if self.box_reference {
            let region = super::aabb_index::Aabb::from_points(&tri).expect("three corners");
            return self
                .reflex
                .query(region)
                .any(|&q| self.blocks(q, [a, b, c], tri));
        }
        // Neighbouring ears are mostly blocked by the same vertex, so try the
        // last one first; a long thin ear otherwise scans a long run of cells
        // before it meets its blocker.
        if let Some(q) = self.last_blocker.get()
            && self.blocks(q, [a, b, c], tri)
        {
            return true;
        }
        let blocker = self
            .reflex
            .query_triangle(tri)
            .find(|&&q| self.blocks(q, [a, b, c], tri));
        self.last_blocker
            .set(blocker.copied().or(self.last_blocker.get()));
        blocker.is_some()
    }

    /// Whether vertex `q` stops the ear `tri` (corners `[a, b, c]`).
    fn blocks(&self, q: usize, [a, b, c]: [usize; 3], tri: [Point2; 3]) -> bool {
        #[cfg(test)]
        if !self.box_reference {
            CANDIDATES_TESTED.with(|n| n.set(n.get() + 1));
        }
        if !self.alive[q] || [a, b, c].contains(&q) {
            return false;
        }
        // The geometric test goes first: it rejects nearly every candidate of
        // a long thin ear and is cheaper than the convexity test.
        let touches = match tri.iter().position(|&corner| corner == self.pts[q]) {
            Some(k) => self.runs_into_ear(q, tri, k),
            None => in_triangle(self.pts[q], tri),
        };
        touches && !self.is_convex(q)
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
