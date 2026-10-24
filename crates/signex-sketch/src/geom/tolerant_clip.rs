//! Degeneracy-tolerant ear clipper for the ring [`super::holes`] builds.
//!
//! The bridged ring carries coincident vertices (every bridge visits its two
//! ends twice), collinear runs and zero-width spikes that the strict
//! [`super::ear_clip`] rejects. This clipper drops the degenerate vertices and
//! clips the rest.
//!
//! An ear is blocked when a ring vertex that is not one of its corners
//! - is non-convex and inside the ear triangle (edges included), or
//! - sits at a corner's coordinates and its ring edges run into the ear (a
//!   bridged ring touches itself there, so the ear would cover boundary).
//!
//! Only non-convex vertices can block an ear, and clipping never makes a
//! convex vertex non-convex, so they are indexed once up front in an
//! [`AabbIndex`] cells of a coarse grid and each ear queries just the cells
//! under its bounding box instead of scanning the whole ring.

use super::Point2;
use super::aabb_index::{Aabb, AabbIndex};
use super::predicates::{Sign, orient2d};

/// Ear-clip a CCW ring that may contain bridge duplicates, collinear runs
/// and zero-width spikes, which are dropped without losing area. If no ear is
/// found the clipper cuts a convex corner anyway rather than give up, so the
/// result may then not partition the ring exactly; callers that need the area
/// exact should check it.
pub(super) fn clip_ears_tolerant(pts: &[Point2]) -> Vec<[usize; 3]> {
    let mut ring = LinkedRing::new(pts);
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
    reflex: ReflexGrid,
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
            reflex: ReflexGrid::empty(),
        };
        let reflex: Vec<usize> = (0..n).filter(|&v| !ring.is_convex(v)).collect();
        ring.reflex = ReflexGrid::new(pts, &reflex);
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
        let region = Aabb::from_points(&tri).expect("three corners");
        self.reflex.query(region).any(|&q| {
            if !self.alive[q] || [a, b, c].contains(&q) || self.is_convex(q) {
                return false;
            }
            match tri.iter().position(|&corner| corner == self.pts[q]) {
                Some(k) => self.runs_into_ear(q, tri, k),
                None => in_triangle(self.pts[q], tri),
            }
        })
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

/// Non-convex vertices bucketed into a square grid of [`AabbIndex`] cells,
/// about four vertices per cell, so a query touches only the cells under its
/// box.
struct ReflexGrid {
    min: Point2,
    cell: f64,
    side: usize,
    cells: Vec<AabbIndex<usize>>,
}

impl ReflexGrid {
    fn empty() -> Self {
        Self {
            min: Point2::new(0.0, 0.0),
            cell: 1.0,
            side: 0,
            cells: Vec::new(),
        }
    }

    fn new(pts: &[Point2], members: &[usize]) -> Self {
        let points: Vec<Point2> = members.iter().map(|&v| pts[v]).collect();
        let Some(bounds) = Aabb::from_points(&points) else {
            return Self::empty();
        };
        let side = ((members.len() as f64 / 4.0).sqrt().ceil() as usize).max(1);
        let extent = (bounds.max.x - bounds.min.x).max(bounds.max.y - bounds.min.y);
        let mut grid = Self {
            min: bounds.min,
            cell: (extent / side as f64).max(f64::MIN_POSITIVE),
            side,
            cells: vec![AabbIndex::new(); side * side],
        };
        for &v in members {
            let (cx, cy) = grid.locate(pts[v]);
            grid.cells[cy * side + cx].insert(v, Aabb::new(pts[v], pts[v]));
        }
        grid
    }

    /// The cell column and row holding `p`, clamped onto the grid.
    fn locate(&self, p: Point2) -> (usize, usize) {
        let at = |v: f64, lo: f64| {
            (((v - lo) / self.cell).floor().max(0.0) as usize).min(self.side.saturating_sub(1))
        };
        (at(p.x, self.min.x), at(p.y, self.min.y))
    }

    /// The vertices whose position lies in `region` (inclusive).
    fn query(&self, region: Aabb) -> impl Iterator<Item = &usize> + '_ {
        let (x0, y0) = self.locate(region.min);
        let (x1, y1) = self.locate(region.max);
        let rows = if self.side == 0 { 0..0 } else { y0..y1 + 1 };
        rows.flat_map(move |cy| {
            let cols = if self.side == 0 { 0..0 } else { x0..x1 + 1 };
            cols.flat_map(move |cx| self.cells[cy * self.side + cx].query_region(region))
        })
    }
}
