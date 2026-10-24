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

    pub(super) fn new(bounds: Aabb, items: usize) -> Self {
        let (w, h) = (bounds.max.x - bounds.min.x, bounds.max.y - bounds.min.y);
        let target = (items as f64 / 4.0).ceil().max(1.0);
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
}
