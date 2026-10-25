//! The ring that [`super::holes`] grows hole by hole.
//!
//! A doubly linked list of vertices, so splicing a hole in costs the hole's
//! size rather than the ring's, plus the indexes the bridge search needs:
//!
//! - an order label per vertex, increasing along the ring from its first
//!   vertex, so ties between equal candidates break by ring position exactly
//!   as a scan of a flat array would;
//! - the vertices at each coordinate, for the copies earlier bridges leave;
//! - a [`CellGrid`] holding the vertices by point and the edges in only the
//!   cells the segment passes through, so a
//!   +x ray or a triangle query reads only the cells it crosses.
//!
//! An edge is stored as the pair of vertices it joined when it was added, and
//! counts only while the first vertex still links to the second: splicing a
//! hole into an edge retires it without touching the cells.

use super::Point2;
use super::aabb_index::Aabb;
use super::point_grid::CellGrid;
use std::collections::HashMap;

/// Index of a vertex in a [`HoleRing`].
pub(super) type VertexId = u32;

/// The widest step between the labels of vertices added in one splice.
const LABEL_STEP: u64 = 1 << 20;

/// The smallest per-vertex label room a splice may start from.
const MIN_LABEL_GAP: u64 = 1 << 8;

/// The label spacing a relabelled stretch is spread to.
const RELABEL_SPACING: u64 = 1 << 24;

struct Vertex {
    point: Point2,
    prev: VertexId,
    next: VertexId,
    label: u64,
}

#[derive(Default, Clone)]
struct Cell {
    vertices: Vec<VertexId>,
    edges: Vec<(VertexId, VertexId)>,
}

pub(super) struct HoleRing {
    vertices: Vec<Vertex>,
    at_point: HashMap<[u64; 2], Vec<VertexId>>,
    grid: CellGrid,
    cells: Vec<Cell>,
}

/// A hashable key equal for points that compare equal (`-0.0 == 0.0`).
fn key(p: Point2) -> [u64; 2] {
    [(p.x + 0.0).to_bits(), (p.y + 0.0).to_bits()]
}

impl HoleRing {
    /// A ring of `outer` (non-empty), indexed for `capacity` vertices that all
    /// lie inside `bounds`.
    pub(super) fn new(outer: &[Point2], bounds: Aabb, capacity: usize) -> Self {
        let mut ring = Self {
            vertices: Vec::with_capacity(capacity),
            at_point: HashMap::with_capacity(capacity),
            grid: CellGrid::new(bounds, capacity),
            cells: Vec::new(),
        };
        ring.cells = vec![Cell::default(); ring.grid.cell_count()];
        let n = outer.len();
        let stride = u64::MAX / (n as u64 + 2);
        for (i, &point) in outer.iter().enumerate() {
            ring.push(Vertex {
                point,
                prev: ((i + n - 1) % n) as VertexId,
                next: ((i + 1) % n) as VertexId,
                label: stride * (i as u64 + 1),
            });
        }
        for i in 0..n as VertexId {
            ring.register_edge(i, (i + 1) % n as VertexId);
        }
        ring
    }

    fn push(&mut self, vertex: Vertex) -> VertexId {
        let id = self.vertices.len() as VertexId;
        let p = vertex.point;
        self.at_point.entry(key(p)).or_default().push(id);
        let cell = self.cell_of(p);
        self.cells[cell].vertices.push(id);
        self.vertices.push(vertex);
        id
    }

    fn cell_of(&self, p: Point2) -> usize {
        self.grid.index(self.grid.column(p.x), self.grid.row(p.y))
    }

    /// Add edge `a`-`b` to the cells the segment passes through, row by row,
    /// not to every cell of its bounding box: a long diagonal over a dense
    /// grid would otherwise land in a number of cells that grows with the
    /// whole grid.
    fn register_edge(&mut self, a: VertexId, b: VertexId) {
        let (pa, pb) = (self.point(a), self.point(b));
        let (r0, r1) = (self.grid.row(pa.y.min(pb.y)), self.grid.row(pa.y.max(pb.y)));
        for row in r0..=r1 {
            let (c0, c1) = self.columns_in_row(pa, pb, row);
            for column in c0..=c1 {
                let at = self.grid.index(column, row);
                self.cells[at].edges.push((a, b));
            }
        }
    }

    /// The columns segment `pa`-`pb` covers within row `row`, widened a hair
    /// on every side so a hit that rounds across a cell boundary is still in
    /// a registered cell. Falls back to the segment's whole column range when
    /// the arithmetic is not finite.
    fn columns_in_row(&self, pa: Point2, pb: Point2, row: usize) -> (usize, usize) {
        let (min_x, max_x) = (pa.x.min(pb.x), pa.x.max(pb.x));
        let whole = (self.grid.column(min_x), self.grid.column(max_x));
        let (lo, hi) = self.grid.row_span(row);
        let pad_y = self.grid.cell_height() * 1e-6;
        let y0 = (lo - pad_y).max(pa.y.min(pb.y));
        let y1 = (hi + pad_y).min(pa.y.max(pb.y));
        if y0 > y1 {
            return whole;
        }
        let x_at = |y: f64| {
            if pa.y == pb.y {
                pa.x
            } else {
                pa.x + (y - pa.y) * (pb.x - pa.x) / (pb.y - pa.y)
            }
        };
        let (x0, x1) = if pa.y == pb.y {
            (min_x, max_x)
        } else {
            (x_at(y0), x_at(y1))
        };
        let pad_x =
            self.grid.cell_width() * 1e-6 + 8.0 * f64::EPSILON * min_x.abs().max(max_x.abs());
        let (from, to) = (x0.min(x1) - pad_x, x0.max(x1) + pad_x);
        if !(from.is_finite() && to.is_finite()) {
            return whole;
        }
        let from = self.grid.column(from.max(min_x));
        let to = self.grid.column(to.min(max_x));
        (from.min(to), from.max(to))
    }

    /// How many (cell, edge) entries the cells hold, stale ones included.
    #[cfg(test)]
    fn edge_registrations(&self) -> usize {
        self.cells.iter().map(|c| c.edges.len()).sum()
    }

    pub(super) fn point(&self, v: VertexId) -> Point2 {
        self.vertices[v as usize].point
    }

    pub(super) fn prev(&self, v: VertexId) -> VertexId {
        self.vertices[v as usize].prev
    }

    pub(super) fn next(&self, v: VertexId) -> VertexId {
        self.vertices[v as usize].next
    }

    /// Where `v` sits along the ring, from its first vertex; compare two
    /// vertices' values to tell which comes first.
    pub(super) fn order(&self, v: VertexId) -> u64 {
        self.vertices[v as usize].label
    }

    /// The vertices at exactly `p`, first in ring order first.
    pub(super) fn copies(&self, p: Point2) -> Vec<VertexId> {
        let mut ids = self.at_point.get(&key(p)).cloned().unwrap_or_default();
        ids.sort_by_key(|&v| self.order(v));
        ids
    }

    /// Every vertex in ring order.
    pub(super) fn ids(&self) -> impl Iterator<Item = VertexId> + '_ {
        let mut at = 0;
        (0..self.vertices.len()).map(move |_| {
            let v = at;
            at = self.next(at);
            v
        })
    }

    pub(super) fn points(&self) -> Vec<Point2> {
        self.ids().map(|v| self.point(v)).collect()
    }

    /// The vertices whose point lies in `region` (inclusive), unordered.
    pub(super) fn vertices_in(&self, region: Aabb) -> impl Iterator<Item = VertexId> + '_ {
        let (c0, c1) = (
            self.grid.column(region.min.x),
            self.grid.column(region.max.x),
        );
        let (r0, r1) = (self.grid.row(region.min.y), self.grid.row(region.max.y));
        (r0..=r1).flat_map(move |row| {
            (c0..=c1).flat_map(move |column| {
                self.cells[self.grid.index(column, row)]
                    .vertices
                    .iter()
                    .copied()
                    .filter(move |&v| region.contains(self.point(v)))
            })
        })
    }

    /// Show `visit` every current edge `(a, b)` registered in a cell in the
    /// row of `m.y` at or right of `m.x` (the cells the segment passes
    /// through), nearest cells first.
    /// `visit` returns the smallest hit x found so far (infinity if none) and
    /// the scan ends once the cells that could hold anything nearer are done.
    /// An edge may be shown more than once.
    pub(super) fn scan_ray(&self, m: Point2, mut visit: impl FnMut(VertexId, VertexId) -> f64) {
        let row = self.grid.row(m.y);
        // One column either side of the exact range absorbs the ulp a hit
        // computed along a slanted edge can sit outside its bounding box.
        let first = self.grid.column(m.x).saturating_sub(1);
        let mut best = f64::INFINITY;
        for column in first..self.grid.columns() {
            for &(a, b) in &self.cells[self.grid.index(column, row)].edges {
                if self.next(a) == b {
                    best = visit(a, b);
                }
            }
            if best.is_finite() && column > self.grid.column(best) {
                return;
            }
        }
    }

    /// Splice `hole` into the ring just after `at`: the hole from `start`
    /// round to `start` again, then a second copy of `at`, so the ring runs
    /// `at`, hole, `at` again, and on. Positions follow the
    /// flat-array form `ring[..=at] + hole[start..] + hole[..=start] +
    /// ring[at..]`.
    pub(super) fn splice_hole(&mut self, at: VertexId, hole: &[Point2], start: usize) {
        let ring_next = self.next(at);
        let points: Vec<Point2> = hole[start..]
            .iter()
            .chain(&hole[..=start])
            .copied()
            .chain(std::iter::once(self.point(at)))
            .collect();
        let labels = self.labels_after(at, points.len());
        let first = self.vertices.len() as VertexId;
        let mut prev = at;
        for (k, (&point, label)) in points.iter().zip(labels).enumerate() {
            let id = first + k as VertexId;
            self.push(Vertex {
                point,
                prev,
                next: ring_next,
                label,
            });
            self.vertices[prev as usize].next = id;
            prev = id;
        }
        self.vertices[ring_next as usize].prev = prev;
        self.register_edge(at, first);
        for id in first..prev {
            self.register_edge(id, id + 1);
        }
        self.register_edge(prev, ring_next);
    }

    /// `count` increasing labels between `at` and its successor, relabelling
    /// a stretch of the ring around `at` first when they have no room.
    fn labels_after(&mut self, at: VertexId, count: usize) -> Vec<u64> {
        let room = |ring: &Self| {
            let next = ring.next(at);
            let end = if next == 0 {
                u64::MAX
            } else {
                ring.order(next)
            };
            (ring.order(at), end)
        };
        let (mut lo, mut hi) = room(self);
        if (hi - lo) / (count as u64 + 1) < MIN_LABEL_GAP {
            self.relabel_around(at, count);
            (lo, hi) = room(self);
        }
        // Packed against the successor, so the room left after `at` stays
        // large for the next splice at the same vertex.
        let step = ((hi - lo) / (count as u64 + 1)).min(LABEL_STEP);
        (0..count as u64)
            .map(|k| hi - step * (count as u64 - k))
            .collect()
    }

    /// Respace the labels of a stretch of the ring around `at`, leaving room
    /// for `spare` vertices after `at`. The stretch doubles until its labels
    /// span enough to give every vertex [`RELABEL_SPACING`], so repeated
    /// splices at one place cost a few vertices each, not the whole ring.
    fn relabel_around(&mut self, at: VertexId, spare: usize) {
        let (mut left, mut right, mut len) = (at, at, 1_usize);
        let (lo, hi, spacing) = loop {
            let lo = if left == 0 {
                0
            } else {
                self.order(self.prev(left))
            };
            let end = self.next(right) == 0;
            let hi = if end {
                u64::MAX
            } else {
                self.order(self.next(right))
            };
            let spacing = (hi - lo) / (len + spare + 1) as u64;
            if spacing >= RELABEL_SPACING || (left == 0 && end) {
                break (lo, hi, spacing);
            }
            let grow_by = len;
            for _ in 0..grow_by {
                if left != 0 {
                    left = self.prev(left);
                    len += 1;
                }
                if self.next(right) != 0 {
                    right = self.next(right);
                    len += 1;
                }
            }
        };
        debug_assert!(hi > lo);
        let (mut v, mut slot) = (left, 1_u64);
        loop {
            self.vertices[v as usize].label = lo + spacing * slot;
            slot += if v == at { spare as u64 + 1 } else { 1 };
            if v == right {
                break;
            }
            v = self.next(v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    /// Long diagonal edges over a dense grid register in the cells they cross,
    /// a count that follows their length, not in every cell of their bounding
    /// box.
    #[test]
    fn long_diagonals_register_only_where_they_pass() {
        // 20 edges, each running corner to corner of the box, then a
        // serpentine of 20 000 nearby points.
        let mut outer = Vec::new();
        for k in 0..10 {
            outer.push(p(0.0, k as f64));
            outer.push(p(1000.0, 100.0 - k as f64));
        }
        let field = 20_000;
        for k in 0..field {
            let (row, col) = (k / 200, k % 200);
            let col = if row % 2 == 0 { col } else { 199 - col };
            outer.push(p(3.0 + col as f64 * 4.9, 1.0 + row as f64 * 0.9));
        }
        let bounds = Aabb::from_points(&outer).unwrap();
        let ring = HoleRing::new(&outer, bounds, outer.len());
        let (cells, edges) = (ring.grid.cell_count(), outer.len());
        let registered = ring.edge_registrations();
        // Bounding boxes would put thousands of entries per diagonal.
        let allowed = 3 * edges + 20 * 600;
        assert!(20 * cells > allowed, "the test must be able to tell");
        assert!(
            registered < allowed,
            "{registered} registrations for {edges} edges over {cells} cells"
        );
    }

    /// Splicing holes at the same few vertices over and over, which nests the
    /// new labels into ever smaller gaps, keeps the ring equal to the flat
    /// vector splice and the labels strictly increasing along it.
    #[test]
    fn repeated_splices_keep_ring_order_and_labels() {
        let outer = vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 100.0), p(0.0, 100.0)];
        let bounds = Aabb::new(p(0.0, 0.0), p(100.0, 100.0));
        let hole = |k: usize| {
            let x = 10.0 + (k % 50) as f64;
            vec![p(x, 20.0), p(x + 0.5, 20.0), p(x, 21.0)]
        };
        let splices = 400;
        let mut ring = HoleRing::new(&outer, bounds, 4 + splices * 5);
        let mut flat = outer.clone();
        let mut ids: Vec<VertexId> = (0..4).collect();
        for k in 0..splices {
            // Alternate between the first vertex, the newest copy and the middle.
            let at = match k % 3 {
                0 => 1,
                1 => ids.iter().rposition(|_| true).unwrap(),
                _ => ids.len() / 2,
            };
            let h = hole(k);
            ring.splice_hole(ids[at], &h, 1);
            let mut merged = flat[..=at].to_vec();
            merged.extend_from_slice(&h[1..]);
            merged.extend_from_slice(&h[..=1]);
            merged.extend_from_slice(&flat[at..]);
            flat = merged;
            ids = ring.ids().collect();
            let labels: Vec<u64> = ids.iter().map(|&v| ring.order(v)).collect();
            assert!(labels.windows(2).all(|w| w[0] < w[1]), "labels at {k}");
        }
        assert_eq!(ring.points(), flat);
    }
}
