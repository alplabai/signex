//! A point index over wire segments, built once per derivation pass so the
//! "which segment does this point sit on" queries stop scanning every wire
//! (issue #1366). Answers are identical to the linear scans they replace —
//! the index only narrows the candidate set; the exact test is still
//! [`point_on_segment`] in the 1 µm key space.

use std::collections::{HashMap, HashSet};

use signex_types::schematic::Point;

use super::{anchor_choice, point_on_segment, pt_key};
use crate::uf::Key;

/// Grid cell edge in 1 µm keys (2^14 = 16.384 mm).
const CELL_SHIFT: u32 = 14;
/// A segment whose bounding box spans more cells than this is kept in a
/// short "long" list scanned for every query instead of being rasterised.
const MAX_CELLS_PER_SEGMENT: i64 = 256;

type Cell = (i64, i64);

fn cell_of(k: Key) -> Cell {
    (k.0 >> CELL_SHIFT, k.1 >> CELL_SHIFT)
}

/// Endpoint set plus a uniform grid of segment bounding boxes.
pub struct SegmentIndex {
    endpoints: HashSet<Key>,
    cells: HashMap<Cell, Vec<(Key, Key)>>,
    long: Vec<(Key, Key)>,
}

impl SegmentIndex {
    /// Index `segments` (each `(start, end)`).
    #[must_use]
    pub fn new(segments: &[(Point, Point)]) -> Self {
        let mut endpoints = HashSet::with_capacity(segments.len() * 2);
        let mut cells: HashMap<Cell, Vec<(Key, Key)>> = HashMap::new();
        let mut long = Vec::new();
        for (a, b) in segments {
            let (ak, bk) = (pt_key(a), pt_key(b));
            endpoints.insert(ak);
            endpoints.insert(bk);
            let (lo, hi) = (
                cell_of((ak.0.min(bk.0), ak.1.min(bk.1))),
                cell_of((ak.0.max(bk.0), ak.1.max(bk.1))),
            );
            let span = (hi.0 - lo.0 + 1).saturating_mul(hi.1 - lo.1 + 1);
            if span > MAX_CELLS_PER_SEGMENT {
                long.push((ak, bk));
                continue;
            }
            for cx in lo.0..=hi.0 {
                for cy in lo.1..=hi.1 {
                    cells.entry((cx, cy)).or_default().push((ak, bk));
                }
            }
        }
        Self {
            endpoints,
            cells,
            long,
        }
    }

    /// True when `pk` is an endpoint of any indexed segment.
    #[must_use]
    pub fn is_endpoint(&self, pk: Key) -> bool {
        self.endpoints.contains(&pk)
    }

    /// Every indexed segment (as key pairs) that contains `pk`, endpoints included.
    pub fn containing(&self, pk: Key) -> impl Iterator<Item = (Key, Key)> + '_ {
        self.cells
            .get(&cell_of(pk))
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .chain(self.long.iter())
            .copied()
            .filter(move |&(a, b)| point_on_segment(pk, a, b))
    }

    /// True when `pk` lies on any indexed segment (endpoint or interior).
    #[must_use]
    pub fn contains(&self, pk: Key) -> bool {
        self.containing(pk).next().is_some()
    }

    /// The deterministic anchor segment for an interior tap (rule 2 of
    /// [`anchor_point`](super::anchor_point)); `None` when nothing contains `pk`.
    #[must_use]
    pub fn anchor_segment(&self, pk: Key) -> Option<(Key, Key)> {
        anchor_choice(self.containing(pk).map(|(a, b)| ((a, b), a, b)))
    }
}
