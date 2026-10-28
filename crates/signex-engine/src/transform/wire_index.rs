//! Per-command spatial index over a sheet's wires, so junction reconciliation
//! asks "which wires are near this point" instead of scanning every wire for
//! every junction (issue #1364).
//!
//! The index only narrows the candidate set; every predicate is still the
//! original float-tolerance / key-space test applied to the candidates, so
//! answers are identical to a full scan. It is built once per command and
//! borrows the wires, so it must be rebuilt if wire geometry changes.

use signex_types::schematic::{Point, Wire};
use std::collections::HashMap;

/// Grid cell edge in mm.
const CELL_MM: f64 = 1.0;
/// A wire whose expanded bounding box spans more cells than this (a long
/// diagonal) is not gridded; it is a candidate for every segment query.
// ponytail: linear scan of these few wires, a quadtree if diagonals dominate.
const MAX_WIRE_CELLS: i64 = 4096;

type Cell = (i64, i64);

fn cell_of(v: f64) -> i64 {
    (v / CELL_MM).floor() as i64
}

/// True when the cell box spans more than [`MAX_WIRE_CELLS`] cells. Saturating
/// math: a huge or non-finite coordinate (a crafted file) saturates the cell
/// index and must read as oversized, never overflow.
fn too_many_cells((x0, y0, x1, y1): (i64, i64, i64, i64)) -> bool {
    let w = x1.saturating_sub(x0).saturating_add(1);
    let h = y1.saturating_sub(y0).saturating_add(1);
    w.checked_mul(h).is_none_or(|n| n > MAX_WIRE_CELLS)
}

pub(crate) struct WireIndex<'a> {
    wires: &'a [Wire],
    tolerance: f64,
    /// Wires with an endpoint in the cell.
    endpoints: HashMap<Cell, Vec<usize>>,
    /// Wires whose bounding box (expanded by `tolerance`) overlaps the cell.
    segments: HashMap<Cell, Vec<usize>>,
    oversized: Vec<usize>,
}

impl<'a> WireIndex<'a> {
    pub(crate) fn new(wires: &'a [Wire], tolerance: f64) -> Self {
        let mut index = WireIndex {
            wires,
            tolerance,
            endpoints: HashMap::new(),
            segments: HashMap::new(),
            oversized: Vec::new(),
        };
        for (i, wire) in wires.iter().enumerate() {
            for p in [wire.start, wire.end] {
                let slot = index
                    .endpoints
                    .entry((cell_of(p.x), cell_of(p.y)))
                    .or_default();
                if slot.last() != Some(&i) {
                    slot.push(i);
                }
            }
            let (x0, y0, x1, y1) = index.wire_cell_box(wire);
            if too_many_cells((x0, y0, x1, y1)) {
                index.oversized.push(i);
                continue;
            }
            for cx in x0..=x1 {
                for cy in y0..=y1 {
                    index.segments.entry((cx, cy)).or_default().push(i);
                }
            }
        }
        index
    }

    pub(crate) fn tolerance(&self) -> f64 {
        self.tolerance
    }

    pub(crate) fn wire(&self, i: usize) -> &Wire {
        &self.wires[i]
    }

    fn wire_cell_box(&self, wire: &Wire) -> (i64, i64, i64, i64) {
        self.cell_box(
            wire.start.x.min(wire.end.x),
            wire.start.y.min(wire.end.y),
            wire.start.x.max(wire.end.x),
            wire.start.y.max(wire.end.y),
        )
    }

    fn cell_box(&self, x0: f64, y0: f64, x1: f64, y1: f64) -> (i64, i64, i64, i64) {
        let t = self.tolerance;
        (
            cell_of(x0 - t),
            cell_of(y0 - t),
            cell_of(x1 + t),
            cell_of(y1 + t),
        )
    }

    /// Indices (ascending, unique) of wires with an endpoint possibly within
    /// `tolerance` of `point` on both axes.
    pub(crate) fn endpoint_candidates(&self, point: Point) -> Vec<usize> {
        let b = self.cell_box(point.x, point.y, point.x, point.y);
        self.collect(&self.endpoints, b, &[])
    }

    /// Indices (ascending, unique) of wires whose segment can pass within
    /// `tolerance` of `point` (also a superset of the wires containing it in
    /// the netlist's 1 um key space).
    pub(crate) fn segment_candidates(&self, point: Point) -> Vec<usize> {
        let b = self.cell_box(point.x, point.y, point.x, point.y);
        self.collect(&self.segments, b, &self.oversized)
    }

    /// Indices (ascending, unique) of wires with an endpoint possibly inside
    /// `wire`'s tolerance-expanded bounding box.
    pub(crate) fn endpoint_candidates_near_wire(&self, wire: &Wire) -> Vec<usize> {
        let b = self.wire_cell_box(wire);
        if too_many_cells(b) {
            return (0..self.wires.len()).collect();
        }
        self.collect(&self.endpoints, b, &[])
    }

    fn collect(
        &self,
        grid: &HashMap<Cell, Vec<usize>>,
        (x0, y0, x1, y1): (i64, i64, i64, i64),
        always: &[usize],
    ) -> Vec<usize> {
        let mut out: Vec<usize> = always.to_vec();
        for cx in x0..=x1 {
            for cy in y0..=y1 {
                if let Some(slot) = grid.get(&(cx, cy)) {
                    out.extend_from_slice(slot);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}
