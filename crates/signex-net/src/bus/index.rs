//! A uniform grid over segments, so "which segments contain this point" costs
//! the segments near the point instead of every segment on the sheet.

use std::collections::HashMap;

use signex_types::schematic::Point;

use crate::uf::Key;
use crate::{point_on_segment, pt_key};

/// Cell edge in 1 µm key units (10 mm).
const CELL: i64 = 10_000;
/// A segment spanning more cells than this is kept in a short linear list
/// instead of being registered in every cell it crosses.
const MAX_CELLS_PER_SEGMENT: i64 = 1024;

pub(super) struct SegmentIndex {
    keyed: Vec<(Key, Key)>,
    cells: HashMap<(i64, i64), Vec<usize>>,
    long: Vec<usize>,
}

fn cell_of(k: Key) -> (i64, i64) {
    (k.0.div_euclid(CELL), k.1.div_euclid(CELL))
}

impl SegmentIndex {
    pub(super) fn new(segments: &[(Point, Point)]) -> Self {
        let keyed: Vec<(Key, Key)> = segments
            .iter()
            .map(|(a, b)| (pt_key(a), pt_key(b)))
            .collect();
        let mut cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        let mut long = Vec::new();
        for (i, &(a, b)) in keyed.iter().enumerate() {
            let lo = cell_of((a.0.min(b.0), a.1.min(b.1)));
            let hi = cell_of((a.0.max(b.0), a.1.max(b.1)));
            let span = (hi.0 - lo.0 + 1).saturating_mul(hi.1 - lo.1 + 1);
            if span > MAX_CELLS_PER_SEGMENT {
                long.push(i);
                continue;
            }
            for cx in lo.0..=hi.0 {
                for cy in lo.1..=hi.1 {
                    cells.entry((cx, cy)).or_default().push(i);
                }
            }
        }
        Self { keyed, cells, long }
    }

    /// Indices of the segments that contain `p` (endpoints included).
    pub(super) fn containing(&self, p: Key) -> impl Iterator<Item = usize> + '_ {
        self.cells
            .get(&cell_of(p))
            .into_iter()
            .flatten()
            .chain(self.long.iter())
            .copied()
            .filter(move |&i| {
                let (a, b) = self.keyed[i];
                point_on_segment(p, a, b)
            })
    }

    /// True when `p` is exactly an endpoint of segment `i`.
    pub(super) fn is_endpoint(&self, i: usize, p: Key) -> bool {
        let (a, b) = self.keyed[i];
        p == a || p == b
    }
}

impl SegmentIndex {
    /// The endpoint keys of segment `i`.
    pub(super) fn keys(&self, i: usize) -> (Key, Key) {
        self.keyed[i]
    }

    /// How many segments a lookup of `p` has to test: the one cell's list
    /// plus the long segments. This is the work a lookup costs.
    #[cfg(test)]
    pub(super) fn candidates(&self, p: Key) -> usize {
        self.cells.get(&cell_of(p)).map_or(0, Vec::len) + self.long.len()
    }
}
