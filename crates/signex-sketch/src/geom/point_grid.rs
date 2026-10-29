//! Points bucketed into a coarse grid of [`AabbIndex`] cells, about four
//! points per cell, so a box query touches only the cells under the box
//! instead of scanning every point. Built once, queried many times; used by
//! the hole bridging and the tolerant ear clipper, whose point sets are large
//! (every vertex of a pour with many cut-outs).
//!
//! The cell count is split across the two axes by the bounds' aspect ratio
//! ([`CellGrid`]), so a 1000:1 strip gets a long row of cells rather than a
//! square grid that puts every point in one row.

use super::Point2;
use super::aabb_index::{Aabb, AabbIndex};

/// The most cells a grid is given, however many items it indexes.
const MAX_CELLS: f64 = (1 << 22) as f64;

/// A `nx` x `ny` lattice of equal cells over some bounds, about one cell per
/// four expected items, with the cell count divided between the axes so cells
/// are close to square whatever the bounds' aspect ratio.
pub(super) struct CellGrid {
    min: Point2,
    cell_w: f64,
    cell_h: f64,
    nx: usize,
    ny: usize,
}

impl CellGrid {
    pub(super) fn empty() -> Self {
        Self {
            min: Point2::new(0.0, 0.0),
            cell_w: 1.0,
            cell_h: 1.0,
            nx: 0,
            ny: 0,
        }
    }

    /// A grid over `bounds` for about `items` points. Bounds that are not
    /// finite (a NaN or infinite coordinate) get a single cell: every point
    /// clamps into it, so queries stay correct, just unindexed. The cell
    /// count is capped, and each axis has at least one cell.
    pub(super) fn new(bounds: Aabb, items: usize) -> Self {
        let (w, h) = (bounds.max.x - bounds.min.x, bounds.max.y - bounds.min.y);
        let single = |min: Point2| Self {
            min,
            cell_w: 1.0,
            cell_h: 1.0,
            nx: 1,
            ny: 1,
        };
        if !(w.is_finite() && h.is_finite() && bounds.min.x.is_finite() && bounds.min.y.is_finite())
        {
            return single(Point2::new(0.0, 0.0));
        }
        let target = (items as f64 / 4.0).ceil().clamp(1.0, MAX_CELLS);
        let (nx, ny) = if w <= 0.0 && h <= 0.0 {
            (1.0, 1.0)
        } else if h <= 0.0 {
            (target, 1.0)
        } else if w <= 0.0 {
            (1.0, target)
        } else {
            let nx = (target * w / h).sqrt().round().clamp(1.0, target);
            (nx, (target / nx).ceil().max(1.0))
        };
        if !(nx >= 1.0 && ny >= 1.0 && nx * ny <= 2.0 * MAX_CELLS) {
            return single(bounds.min);
        }
        Self {
            min: bounds.min,
            cell_w: (w / nx).max(f64::MIN_POSITIVE),
            cell_h: (h / ny).max(f64::MIN_POSITIVE),
            nx: nx as usize,
            ny: ny as usize,
        }
    }

    pub(super) fn cell_count(&self) -> usize {
        self.nx * self.ny
    }

    pub(super) fn columns(&self) -> usize {
        self.nx
    }

    /// The column holding `x`, clamped onto the grid.
    pub(super) fn column(&self, x: f64) -> usize {
        (((x - self.min.x) / self.cell_w).floor().max(0.0) as usize).min(self.nx.saturating_sub(1))
    }

    /// The row holding `y`, clamped onto the grid.
    pub(super) fn row(&self, y: f64) -> usize {
        (((y - self.min.y) / self.cell_h).floor().max(0.0) as usize).min(self.ny.saturating_sub(1))
    }

    /// The y range row `row` covers; the first and last rows reach to infinity
    /// because points outside the bounds clamp onto them.
    pub(super) fn row_span(&self, row: usize) -> (f64, f64) {
        let lo = if row == 0 {
            f64::NEG_INFINITY
        } else {
            self.min.y + row as f64 * self.cell_h
        };
        let hi = if row + 1 >= self.ny {
            f64::INFINITY
        } else {
            self.min.y + (row + 1) as f64 * self.cell_h
        };
        (lo, hi)
    }

    /// The columns, first to last, that the triangle `tri` can reach inside
    /// row `row`: the x extent of the triangle clipped to the row's slab,
    /// widened by a column and a hair of slab so rounding in the cell maths
    /// never drops a cell that holds a point of the triangle.
    pub(super) fn row_columns(&self, row: usize, tri: [Point2; 3]) -> (usize, usize) {
        let (lo, hi) = self.row_span(row);
        let magnitude = tri
            .iter()
            .map(|p| p.y.abs())
            .fold(self.min.y.abs(), f64::max);
        let pad = self.cell_h * 1e-9 + magnitude * 4.0 * f64::EPSILON;
        let (lo, hi) = (lo - pad, hi + pad);
        let (mut x0, mut x1) = (f64::INFINITY, f64::NEG_INFINITY);
        let mut take = |x: f64| {
            x0 = x0.min(x);
            x1 = x1.max(x);
        };
        for k in 0..3 {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            if a.y >= lo && a.y <= hi {
                take(a.x);
            }
            for edge_y in [lo, hi] {
                if (a.y < edge_y) != (b.y < edge_y) {
                    take(a.x + (b.x - a.x) * (edge_y - a.y) / (b.y - a.y));
                }
            }
        }
        if x0 > x1 {
            return (1, 0);
        }
        let first = self.column(x0).saturating_sub(1);
        let last = (self.column(x1) + 1).min(self.nx.saturating_sub(1));
        (first, last)
    }

    pub(super) fn cell_width(&self) -> f64 {
        self.cell_w
    }

    pub(super) fn cell_height(&self) -> f64 {
        self.cell_h
    }

    pub(super) fn index(&self, column: usize, row: usize) -> usize {
        row * self.nx + column
    }
}

pub(super) struct PointGrid<T> {
    grid: CellGrid,
    cells: Vec<AabbIndex<T>>,
}

impl<T: Clone> PointGrid<T> {
    pub(super) fn empty() -> Self {
        Self {
            grid: CellGrid::empty(),
            cells: Vec::new(),
        }
    }

    pub(super) fn new(items: Vec<(Point2, T)>) -> Self {
        let points: Vec<Point2> = items.iter().map(|(p, _)| *p).collect();
        let Some(bounds) = Aabb::from_points(&points) else {
            return Self::empty();
        };
        let grid = CellGrid::new(bounds, items.len());
        let mut cells = vec![AabbIndex::new(); grid.cell_count()];
        for (p, item) in items {
            let at = grid.index(grid.column(p.x), grid.row(p.y));
            cells[at].insert(item, Aabb::new(p, p));
        }
        Self { grid, cells }
    }

    /// The items whose point lies in `region` (inclusive).
    pub(super) fn query(&self, region: Aabb) -> impl Iterator<Item = &T> + '_ {
        let grid = &self.grid;
        let (columns, rows) = if self.cells.is_empty() {
            (0..0, 0..0)
        } else {
            (
                grid.column(region.min.x)..grid.column(region.max.x) + 1,
                grid.row(region.min.y)..grid.row(region.max.y) + 1,
            )
        };
        rows.flat_map(move |row| {
            columns
                .clone()
                .flat_map(move |column| self.cells[grid.index(column, row)].query_region(region))
        })
    }

    /// The items whose point lies in a cell the triangle `tri` overlaps and
    /// inside its bounding box: a superset of the items inside the triangle.
    /// Only the cells the triangle crosses are walked, not every cell under
    /// its box, and in an order that reaches the corners of a long thin
    /// triangle first (that is where its blockers sit), so an `any` / `find`
    /// over a thin ear usually stops early. Non-finite corners fall back to
    /// the box.
    pub(super) fn query_triangle(&self, tri: [Point2; 3]) -> impl Iterator<Item = &T> + '_ {
        let grid = &self.grid;
        let region = Aabb::from_points(&tri).expect("three corners");
        let finite = tri.iter().all(|p| p.x.is_finite() && p.y.is_finite());
        // (row, first column, last column) per row the triangle crosses,
        // widest run first so the sweep below can stop at the first short one.
        let mut runs: Vec<(usize, usize, usize)> = if self.cells.is_empty() {
            Vec::new()
        } else {
            (grid.row(region.min.y)..=grid.row(region.max.y))
                .map(|row| {
                    let (first, last) = if finite {
                        grid.row_columns(row, tri)
                    } else {
                        (grid.column(region.min.x), grid.column(region.max.x))
                    };
                    (row, first, last)
                })
                .collect()
        };
        let width = |&(_, first, last): &(usize, usize, usize)| (last + 1).saturating_sub(first);
        runs.sort_by_key(|run| std::cmp::Reverse(width(run)));
        let longest = runs.first().map_or(0, width);
        let runs = std::rc::Rc::new(runs);
        // Step `i` takes the i-th cell from alternating ends of every run
        // that is long enough to have one.
        (0..longest).flat_map(move |i| {
            let runs = std::rc::Rc::clone(&runs);
            let count = runs.partition_point(|run| width(run) > i);
            (0..count).flat_map(move |k| {
                let (row, first, last) = runs[k];
                let column = if i % 2 == 0 {
                    first + i / 2
                } else {
                    last - i / 2
                };
                self.cells[grid.index(column, row)].query_region(region)
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(w: f64, h: f64) -> Aabb {
        Aabb::new(Point2::new(0.0, 0.0), Point2::new(w, h))
    }

    /// Cells follow the bounds' shape: a 1000:1 strip is a long row of cells,
    /// not a square lattice with the strip in a single row of it.
    #[test]
    fn cells_follow_the_aspect_ratio() {
        let strip = CellGrid::new(bounds(1000.0, 1.0), 4000);
        assert!(
            strip.nx >= 500 && strip.ny <= 2,
            "{}x{}",
            strip.nx,
            strip.ny
        );
        let tall = CellGrid::new(bounds(1.0, 1000.0), 4000);
        assert!(tall.ny >= 500 && tall.nx <= 2, "{}x{}", tall.nx, tall.ny);
        let square = CellGrid::new(bounds(10.0, 10.0), 4000);
        assert_eq!((square.nx, square.ny), (32, 32));
        let line = CellGrid::new(bounds(100.0, 0.0), 400);
        assert_eq!((line.nx, line.ny), (100, 1));
        let dot = CellGrid::new(bounds(0.0, 0.0), 400);
        assert_eq!((dot.nx, dot.ny), (1, 1));
    }

    /// Box queries on a strip return exactly the points inside the box.
    /// Non-finite or degenerate bounds still build a valid grid whose queries
    /// match a scan.
    #[test]
    fn odd_bounds_build_a_valid_grid() {
        let nan = f64::NAN;
        let inf = f64::INFINITY;
        let pt = |x, y| Point2::new(x, y);
        let cases = [
            Aabb::new(pt(0.0, 0.0), pt(nan, 5.0)),
            Aabb::new(pt(nan, nan), pt(nan, nan)),
            Aabb::new(pt(0.0, 0.0), pt(inf, 5.0)),
            Aabb::new(pt(-inf, 0.0), pt(inf, inf)),
            Aabb::new(pt(0.0, 0.0), pt(0.0, 5.0)),
            Aabb::new(pt(0.0, 0.0), pt(5.0, 0.0)),
            Aabb::new(pt(1.0, 1.0), pt(1.0, 1.0)),
            Aabb::new(pt(0.0, 0.0), pt(1e300, 1e-300)),
        ];
        for bounds in cases {
            let grid = CellGrid::new(bounds, 4000);
            assert!(grid.nx >= 1 && grid.ny >= 1, "{bounds:?}");
            assert!(grid.cell_count() <= 2 << 22, "{bounds:?}");
        }
        let points: Vec<(Point2, usize)> = [
            pt(0.0, 0.0),
            pt(1.0, 2.0),
            pt(nan, 1.0),
            pt(3.0, inf),
            pt(-inf, 0.5),
            pt(4.0, 4.0),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, p)| (p, i))
        .collect();
        let grid = PointGrid::new(points.clone());
        let region = Aabb::new(pt(-1.0, -1.0), pt(5.0, 5.0));
        let mut got: Vec<usize> = grid.query(region).copied().collect();
        let mut want: Vec<usize> = points
            .iter()
            .filter(|(p, _)| Aabb::new(*p, *p).overlaps(region))
            .map(|&(_, i)| i)
            .collect();
        got.sort_unstable();
        want.sort_unstable();
        assert_eq!(got, want);
        let strip = Aabb::new(pt(0.0, 0.0), pt(1000.0, 0.0));
        let strip_grid = CellGrid::new(strip, 4000);
        assert_eq!((strip_grid.nx, strip_grid.ny), (1000, 1));
    }

    #[test]
    fn strip_queries_match_a_scan() {
        let points: Vec<(Point2, usize)> = (0..3000)
            .map(|i| {
                let x = i as f64 * 0.37;
                (Point2::new(x, (i * 7 % 11) as f64 * 0.1), i)
            })
            .collect();
        let grid = PointGrid::new(points.clone());
        for (x0, x1, y0, y1) in [
            (0.0, 5.0, 0.0, 1.0),
            (100.0, 400.0, 0.2, 0.5),
            (-50.0, 2000.0, -1.0, 2.0),
            (500.0, 500.5, 0.3, 0.3),
            (1200.0, 1300.0, 5.0, 6.0),
        ] {
            let region = Aabb::new(Point2::new(x0, y0), Point2::new(x1, y1));
            let mut got: Vec<usize> = grid.query(region).copied().collect();
            let mut want: Vec<usize> = points
                .iter()
                .filter(|(p, _)| region.contains(*p))
                .map(|&(_, i)| i)
                .collect();
            got.sort_unstable();
            want.sort_unstable();
            assert_eq!(got, want, "{region:?}");
        }
    }

    /// Every item whose point is inside or on `tri` and inside its bounding
    /// box, by an exact scan; `query_triangle` must return all of them, and
    /// only items inside the box. Also checks that `row_columns` spans the
    /// column of each such item in its row.
    fn assert_covers(
        items: &[(Point2, usize)],
        grid: &PointGrid<usize>,
        tri: [Point2; 3],
        case: &str,
    ) {
        use super::super::predicates::{Sign, orient2d};
        let bbox = Aabb::from_points(&tri).expect("three corners");
        let got: Vec<usize> = grid.query_triangle(tri).copied().collect();
        for (p, i) in items {
            let signs = [0, 1, 2].map(|k| orient2d(tri[k], tri[(k + 1) % 3], *p));
            let inside = !(signs.contains(&Sign::Positive) && signs.contains(&Sign::Negative));
            if inside && bbox.contains(*p) {
                assert!(got.contains(i), "{case}: {tri:?} missed {p:?}");
                let (first, last) = grid.grid.row_columns(grid.grid.row(p.y), tri);
                let column = grid.grid.column(p.x);
                assert!(
                    first <= column && column <= last,
                    "{case}: {tri:?} row span {first}..={last} misses column {column} of {p:?}"
                );
            }
        }
        assert!(
            got.iter().all(|&i| bbox.contains(items[i].0)),
            "{case}: {tri:?} returned a point outside its box"
        );
    }

    fn rng(mut seed: u64) -> impl FnMut(u64) -> u64 {
        move |range| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % range
        }
    }

    /// `query_triangle` never misses a point inside or on the triangle,
    /// against a brute-force scan, over a table of the awkward triangles:
    /// each case below is named in the assertion message.
    #[test]
    fn triangle_queries_cover_the_points_inside() {
        let mut next = rng(0x2545_f491_4f6c_dd1d);
        // Lattice points and corners, so many land exactly on an edge or a
        // corner, at the origin and far from it.
        for offset in [0.0, 1.0e7] {
            let at = |u: u64, v: u64| Point2::new(u as f64 * 0.5, offset + v as f64 * 0.125);
            let items: Vec<(Point2, usize)> =
                (0..2000).map(|i| (at(next(60), next(200)), i)).collect();
            let grid = PointGrid::new(items.clone());
            for _ in 0..300 {
                let tri = [0; 3].map(|_| at(next(60), next(200)));
                assert_covers(&items, &grid, tri, "vertices on edges and corners");
                let (p, q) = (at(next(60), next(200)), at(next(60), next(200)));
                assert_covers(&items, &grid, [p, p, q], "coincident vertices (two)");
                assert_covers(&items, &grid, [p, p, p], "coincident vertices (all three)");
                let mid = Point2::new((p.x + q.x) / 2.0, (p.y + q.y) / 2.0);
                assert_covers(&items, &grid, [p, mid, q], "collinear, zero area");
                assert_covers(&items, &grid, [p, q, p], "degenerate spike");
                let far = Point2::new(-500.0 + next(40) as f64, offset - 90.0 + next(10) as f64);
                assert_covers(&items, &grid, [far, p, q], "corner outside the grid bounds");
                assert_covers(
                    &items,
                    &grid,
                    [
                        far,
                        Point2::new(far.x - 5.0, far.y - 5.0),
                        Point2::new(far.x - 9.0, far.y),
                    ],
                    "wholly outside the grid bounds",
                );
            }
        }
        // Slivers thinner than a cell, and near-horizontal ones that straddle
        // a row boundary, on a grid whose cells are tiny next to a 1e7 offset.
        for offset in [0.0, 1.0e7] {
            let items: Vec<(Point2, usize)> = (0..4000)
                .map(|i| {
                    let (u, v) = (next(400), next(100));
                    (
                        Point2::new(u as f64 * 2.5e-6, offset + v as f64 * 1.0e-5),
                        i,
                    )
                })
                .collect();
            let grid = PointGrid::new(items.clone());
            assert!(grid.grid.ny >= 4, "{} rows", grid.grid.ny);
            let cell_h = grid.grid.cell_height();
            for _ in 0..300 {
                let row = 1 + next((grid.grid.ny as u64).saturating_sub(2).max(1)) as usize;
                let (boundary, _) = grid.grid.row_span(row);
                let (x0, x1) = (
                    next(200) as f64 * 2.5e-6,
                    5.0e-4 + next(200) as f64 * 2.5e-6,
                );
                let skew = [
                    0.0,
                    cell_h * 1.0e-3,
                    cell_h * 1.0e-9,
                    f64::EPSILON * offset.max(1.0),
                ][next(4) as usize];
                assert_covers(
                    &items,
                    &grid,
                    [
                        Point2::new(x0, boundary),
                        Point2::new(x1, boundary + skew),
                        Point2::new(x1, boundary),
                    ],
                    "near-horizontal sliver on a row boundary",
                );
                assert_covers(
                    &items,
                    &grid,
                    [
                        Point2::new(x0, boundary - skew),
                        Point2::new(x1, boundary + skew),
                        Point2::new(x1, boundary - skew),
                    ],
                    "near-horizontal sliver straddling a row boundary",
                );
                let y = offset + next(100) as f64 * 1.0e-5;
                assert_covers(
                    &items,
                    &grid,
                    [
                        Point2::new(x0, y),
                        Point2::new(x1, y + cell_h * 0.01),
                        Point2::new(x1, y),
                    ],
                    "thinner than one cell",
                );
            }
        }
    }

    /// `row_columns` reports no columns for a row the triangle does not
    /// reach, and a run holding the triangle's x extent for one it crosses.
    #[test]
    fn row_columns_follow_the_triangle() {
        let grid = CellGrid::new(bounds(100.0, 100.0), 400);
        let tri = [
            Point2::new(10.0, 1.0),
            Point2::new(60.0, 1.0),
            Point2::new(60.0, 2.0),
        ];
        let (first, last) = grid.row_columns(grid.row(1.5), tri);
        assert!(first <= grid.column(10.0) && grid.column(60.0) <= last);
        assert!(last - first <= grid.column(60.0) - grid.column(10.0) + 2);
        let (first, last) = grid.row_columns(grid.ny - 1, tri);
        assert!(first > last, "{first}..={last}");
    }
}
