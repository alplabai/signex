//! Points bucketed into a coarse square grid of [`AabbIndex`] cells, about
//! four points per cell, so a box query touches only the cells under the box
//! instead of scanning every point. Built once, queried many times; used by
//! the hole bridging and the tolerant ear clipper, whose point sets are large
//! (every vertex of a pour with many cut-outs).

use super::Point2;
use super::aabb_index::{Aabb, AabbIndex};

pub(super) struct PointGrid<T> {
    min: Point2,
    cell: f64,
    side: usize,
    cells: Vec<AabbIndex<T>>,
}

impl<T: Clone> PointGrid<T> {
    pub(super) fn empty() -> Self {
        Self {
            min: Point2::new(0.0, 0.0),
            cell: 1.0,
            side: 0,
            cells: Vec::new(),
        }
    }

    pub(super) fn new(items: Vec<(Point2, T)>) -> Self {
        let points: Vec<Point2> = items.iter().map(|(p, _)| *p).collect();
        let Some(bounds) = Aabb::from_points(&points) else {
            return Self::empty();
        };
        let side = ((items.len() as f64 / 4.0).sqrt().ceil() as usize).max(1);
        let extent = (bounds.max.x - bounds.min.x).max(bounds.max.y - bounds.min.y);
        let mut grid = Self {
            min: bounds.min,
            cell: (extent / side as f64).max(f64::MIN_POSITIVE),
            side,
            cells: vec![AabbIndex::new(); side * side],
        };
        for (p, item) in items {
            let (cx, cy) = grid.locate(p);
            grid.cells[cy * side + cx].insert(item, Aabb::new(p, p));
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

    /// The items whose point lies in `region` (inclusive).
    pub(super) fn query(&self, region: Aabb) -> impl Iterator<Item = &T> + '_ {
        let (x0, y0) = self.locate(region.min);
        let (x1, y1) = self.locate(region.max);
        let (rows, cols) = if self.side == 0 {
            (0..0, 0..0)
        } else {
            (y0..y1 + 1, x0..x1 + 1)
        };
        rows.flat_map(move |cy| {
            cols.clone()
                .flat_map(move |cx| self.cells[cy * self.side + cx].query_region(region))
        })
    }
}
