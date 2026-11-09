//! Where things are, for the questions the pass asks of one place.
//!
//! Every question the rubber-band pass asks (what else is on this tip, does
//! this segment touch anything foreign, which wires end here) concerns one
//! point or one segment. A [`Grid`] files wire indices by the cells their
//! bounding box covers and point items by the cell they sit in, so a question
//! reads the few things near the place instead of every wire, label, dot and
//! pin of the sheet.
//!
//! The grid is a superset filter, never an answer: it hands back candidates,
//! and the caller runs the same exact test over them as it ran over everything
//! before. A thing that moves is filed under its new cell as well and is never
//! taken out of the old ones (a stale entry is one more candidate to test), so
//! the candidates always include every thing whose current box meets the
//! question's box. Candidates come back sorted by index, so a search that took
//! the first match in sheet order still does.
//!
//! In tests [`Grid::scan_all`] makes every question see every thing: the
//! reference the grid is held to.

use std::collections::HashMap;

use signex_net::uf::Key;

/// Cell edge in 1 um keys (2^14 = 16.384 mm).
const CELL_SHIFT: u32 = 14;
/// A box spanning more cells than this is filed in `long` instead.
const MAX_CELLS: i64 = 64;

type Cell = (i64, i64);

fn cell_of(k: Key) -> Cell {
    (k.0 >> CELL_SHIFT, k.1 >> CELL_SHIFT)
}

/// The cells a box covers, or `None` when it covers too many.
fn cells(lo: Key, hi: Key) -> Option<impl Iterator<Item = Cell>> {
    let (a, b) = (cell_of(lo), cell_of(hi));
    let span = (b.0 - a.0 + 1).saturating_mul(b.1 - a.1 + 1);
    (span <= MAX_CELLS).then(|| (a.0..=b.0).flat_map(move |x| (a.1..=b.1).map(move |y| (x, y))))
}

fn bounds(a: Key, b: Key) -> (Key, Key) {
    ((a.0.min(b.0), a.1.min(b.1)), (a.0.max(b.0), a.1.max(b.1)))
}

/// Indices filed by the cells of a box.
#[derive(Default)]
pub(super) struct Filed {
    cells: HashMap<Cell, Vec<usize>>,
    /// Too big for cells: a candidate of every question.
    long: Vec<usize>,
}

impl Filed {
    /// File `index` under the box `a`-`b`.
    pub(super) fn file(&mut self, index: usize, a: Key, b: Key) {
        let (lo, hi) = bounds(a, b);
        match cells(lo, hi) {
            Some(covered) => {
                for cell in covered {
                    let at = self.cells.entry(cell).or_default();
                    if at.last() != Some(&index) {
                        at.push(index);
                    }
                }
            }
            None => {
                if self.long.last() != Some(&index) {
                    self.long.push(index);
                }
            }
        }
    }

    /// Every index filed under a cell the box `a`-`b` meets, with repeats.
    fn near(&self, a: Key, b: Key, out: &mut Vec<usize>) {
        out.extend_from_slice(&self.long);
        let (lo, hi) = bounds(a, b);
        match cells(lo, hi) {
            Some(covered) => {
                for cell in covered {
                    if let Some(at) = self.cells.get(&cell) {
                        out.extend_from_slice(at);
                    }
                }
            }
            // A question this big is rare: every filed index is a candidate.
            None => {
                out.extend(self.cells.values().flatten());
            }
        }
    }
}

/// The wires, labels, dots, no-connects and pin tips of the working copy, by
/// place.
#[derive(Default)]
pub(super) struct Grid {
    wires: Filed,
    labels: Filed,
    junctions: Filed,
    no_connects: Filed,
    tips: Filed,
    #[cfg(test)]
    scan_all: bool,
}

/// What a question can see.
#[derive(Clone, Copy)]
pub(super) enum Kind {
    Wire,
    Label,
    Junction,
    NoConnect,
    Tip,
}

impl Grid {
    fn filed(&self, kind: Kind) -> &Filed {
        match kind {
            Kind::Wire => &self.wires,
            Kind::Label => &self.labels,
            Kind::Junction => &self.junctions,
            Kind::NoConnect => &self.no_connects,
            Kind::Tip => &self.tips,
        }
    }

    pub(super) fn file(&mut self, kind: Kind, index: usize, a: Key, b: Key) {
        let filed = match kind {
            Kind::Wire => &mut self.wires,
            Kind::Label => &mut self.labels,
            Kind::Junction => &mut self.junctions,
            Kind::NoConnect => &mut self.no_connects,
            Kind::Tip => &mut self.tips,
        };
        filed.file(index, a, b);
    }

    /// The indices that can meet the box `a`-`b`, sorted, once each, all
    /// below `len`. With `scan_all` (tests) all of `0..len`.
    pub(super) fn near(&self, kind: Kind, a: Key, b: Key, len: usize) -> Vec<usize> {
        #[cfg(test)]
        if self.scan_all {
            return (0..len).collect();
        }
        let mut found = Vec::new();
        self.filed(kind).near(a, b, &mut found);
        found.retain(|&i| i < len);
        found.sort_unstable();
        found.dedup();
        found
    }

    /// Every question sees everything from now on.
    #[cfg(test)]
    pub(super) fn scan_all(&mut self) {
        self.scan_all = true;
    }
}

#[cfg(test)]
thread_local! {
    static SCAN_ALL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether the pass being built on this thread must scan everything.
#[cfg(test)]
pub(super) fn scan_all_requested() -> bool {
    SCAN_ALL.with(std::cell::Cell::get)
}

/// Make every pass built on this thread scan everything until the guard drops.
#[cfg(test)]
pub(crate) fn scan_all_on(on: bool) -> ScanAll {
    ScanAll(SCAN_ALL.with(|flag| flag.replace(on)))
}

/// Restores the setting [`scan_all_on`] found.
#[cfg(test)]
pub(crate) struct ScanAll(bool);

#[cfg(test)]
impl Drop for ScanAll {
    fn drop(&mut self) {
        SCAN_ALL.with(|flag| flag.set(self.0));
    }
}
