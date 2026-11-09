//! The wires near a few boxes, found without indexing the sheet's wires.
//!
//! Reconciling junction dots reads wires only near the wires it mints dots
//! for and near the dots it judges again. Handing the junction code just the
//! wires that can be near those places gives it the same answers as handing
//! it every wire: its tests are all "within the tolerance of this point".
//! The boxes go into a coarse grid; each wire of the sheet is then one
//! constant-time look-up of the few cells its own box covers.

use std::collections::HashMap;

use signex_types::schematic::{Point, Wire};

/// How far past a touched wire or a judged dot a reconcile test can read a
/// wire: each test reads wires within one tolerance of its query point, and
/// each query point is within one tolerance of a touched wire or is a judged
/// dot, so two tolerances reach every wire read. The third is headroom for
/// float rounding in the grown boxes. Proof: docs/INCREMENTAL_NETS.md,
/// "Reconcile margin".
pub(crate) const RECONCILE_MARGIN_MM: f64 =
    REACH_IN_TOLERANCES * crate::JUNCTION_TOLERANCE_MM + crate::JUNCTION_TOLERANCE_MM;
/// Hops from a touched wire to a wire a test reads, one tolerance each.
const REACH_IN_TOLERANCES: f64 = 2.0;

/// Cell edge in mm.
const CELL_MM: f64 = 4.0;
/// A box covering more cells than this is checked for every wire.
const MAX_CELLS: i64 = 1024;

/// A box `(min x, min y, max x, max y)` in mm.
type Area = (f64, f64, f64, f64);

type Cell = (i64, i64);

fn cell(v: f64) -> i64 {
    (v / CELL_MM).floor() as i64
}

fn cells_of((x0, y0, x1, y1): Area) -> Option<(i64, i64, i64, i64)> {
    let (cx0, cy0, cx1, cy1) = (cell(x0), cell(y0), cell(x1), cell(y1));
    let w = cx1.saturating_sub(cx0).saturating_add(1);
    let h = cy1.saturating_sub(cy0).saturating_add(1);
    w.checked_mul(h)
        .filter(|n| *n <= MAX_CELLS)
        .map(|_| (cx0, cy0, cx1, cy1))
}

fn finite(p: &Point) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

fn meets(a: Area, b: Area) -> bool {
    a.0 <= b.2 && b.0 <= a.2 && a.1 <= b.3 && b.1 <= a.3
}

/// A set of boxes, to ask which wires can touch one.
pub(crate) struct Regions {
    boxes: Vec<Area>,
    cells: HashMap<Cell, Vec<usize>>,
    /// Too big for cells: tried for every wire.
    wide: Vec<usize>,
}

impl Regions {
    pub(crate) fn new(boxes: Vec<Area>) -> Self {
        let mut cells: HashMap<Cell, Vec<usize>> = HashMap::new();
        let mut wide = Vec::new();
        for (i, area) in boxes.iter().enumerate() {
            match cells_of(*area) {
                Some((x0, y0, x1, y1)) => {
                    for cx in x0..=x1 {
                        for cy in y0..=y1 {
                            cells.entry((cx, cy)).or_default().push(i);
                        }
                    }
                }
                None => wide.push(i),
            }
        }
        Self { boxes, cells, wide }
    }

    /// Whether `wire` can be near a box: its own box meets one. A wire with a
    /// coordinate that is not a number has no box and is always kept.
    pub(crate) fn may_touch(&self, wire: &Wire) -> bool {
        if !finite(&wire.start) || !finite(&wire.end) {
            return true;
        }
        let area = (
            wire.start.x.min(wire.end.x),
            wire.start.y.min(wire.end.y),
            wire.start.x.max(wire.end.x),
            wire.start.y.max(wire.end.y),
        );
        if self.wide.iter().any(|&i| meets(self.boxes[i], area)) {
            return true;
        }
        let Some((x0, y0, x1, y1)) = cells_of(area) else {
            // A wire this long: every box is tried.
            return self.boxes.iter().any(|&b| meets(b, area));
        };
        (x0..=x1).any(|cx| {
            (y0..=y1).any(|cy| {
                self.cells
                    .get(&(cx, cy))
                    .is_some_and(|at| at.iter().any(|&i| meets(self.boxes[i], area)))
            })
        })
    }
}

/// `area` grown by `margin` on every side.
pub(crate) fn grown((x0, y0, x1, y1): Area, margin: f64) -> Area {
    (x0 - margin, y0 - margin, x1 + margin, y1 + margin)
}

/// The box of the segment `a`-`b`.
pub(crate) fn boxed(a: Point, b: Point) -> Area {
    (a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y))
}
