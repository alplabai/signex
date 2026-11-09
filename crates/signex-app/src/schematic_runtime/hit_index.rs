//! A bucket grid over the item hit extents of one snapshot, so a hover hit
//! test reads the few items near the cursor instead of bounding every item.
//! `hit_test_styled` stays the reference: [`HitIndex::hit`] returns the same
//! item for every point (the last item in sheet order whose extent holds it).

use super::*;

/// Edge of one grid cell in mm.
const CELL_MM: f64 = 10.0;
/// An extent covering more cells than this is scanned for every query.
const MAX_CELLS_PER_ENTRY: i64 = 256;

#[derive(Clone, Copy)]
enum Shape {
    /// Hit inside the extent.
    Box,
    /// Hit within `tolerance` of the segment.
    Segment { a: Point, b: Point, tolerance: f64 },
}

#[derive(Clone, Copy)]
struct Entry {
    item: SelectedItem,
    extent: Aabb,
    shape: Shape,
}

impl Entry {
    fn hits(&self, point: Point) -> bool {
        match self.shape {
            Shape::Box => self.extent.contains(point.x, point.y),
            Shape::Segment { a, b, tolerance } => {
                point_to_segment_distance(point, a, b) <= tolerance
            }
        }
    }
}

pub struct HitIndex {
    entries: Vec<Entry>,
    cells: HashMap<(i64, i64), Vec<u32>>,
    /// Entries too large (or not finite) to bucket, ascending.
    wide: Vec<u32>,
}

fn cell_of(v: f64) -> i64 {
    (v / CELL_MM).floor() as i64
}

impl HitIndex {
    pub fn build(
        snapshot: &SchematicRenderSnapshot,
        designators: &Designators,
        widths: WireWidths<'_>,
    ) -> Self {
        let mut wire_index = 0;
        let mut bus_index = 0;
        let entries: Vec<Entry> = collect_item_bounds(snapshot, designators, widths)
            .into_iter()
            .map(|item| match item.item.kind {
                SelectedKind::Wire => {
                    let wire = &snapshot.wires[wire_index];
                    wire_index += 1;
                    let tolerance = widths
                        .wire_mm(wire)
                        .max(signex_types::schematic::SCHEMATIC_HIT_WIRE_TOLERANCE_MM);
                    segment_entry(item.item, wire.start, wire.end, tolerance)
                }
                SelectedKind::Bus => {
                    let bus = &snapshot.buses[bus_index];
                    bus_index += 1;
                    let tolerance = widths
                        .bus_mm()
                        .max(signex_types::schematic::SCHEMATIC_HIT_BUS_TOLERANCE_MM);
                    segment_entry(item.item, bus.start, bus.end, tolerance)
                }
                _ => Entry {
                    item: item.item,
                    extent: item.bbox.expand(0.25),
                    shape: Shape::Box,
                },
            })
            .collect();

        let mut cells: HashMap<(i64, i64), Vec<u32>> = HashMap::new();
        let mut wide = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            let e = &entry.extent;
            let finite = e.min_x.is_finite()
                && e.min_y.is_finite()
                && e.max_x.is_finite()
                && e.max_y.is_finite();
            if !finite {
                wide.push(i as u32);
                continue;
            }
            let (x0, x1) = (cell_of(e.min_x), cell_of(e.max_x));
            let (y0, y1) = (cell_of(e.min_y), cell_of(e.max_y));
            let span = (x1 - x0 + 1).saturating_mul(y1 - y0 + 1);
            if span > MAX_CELLS_PER_ENTRY {
                wide.push(i as u32);
                continue;
            }
            for cx in x0..=x1 {
                for cy in y0..=y1 {
                    cells.entry((cx, cy)).or_default().push(i as u32);
                }
            }
        }
        Self {
            entries,
            cells,
            wide,
        }
    }

    /// The topmost item under `(x, y)`: the hit entry latest in sheet order.
    pub fn hit(&self, x: f64, y: f64) -> Option<SelectedItem> {
        let point = Point::new(x, y);
        let mut best: Option<u32> = None;
        let near = if x.is_finite() && y.is_finite() {
            self.cells.get(&(cell_of(x), cell_of(y)))
        } else {
            None
        };
        for &i in near.into_iter().flatten().chain(self.wide.iter()) {
            if best.is_some_and(|b| b >= i) {
                continue;
            }
            if self.entries[i as usize].hits(point) {
                best = Some(i);
            }
        }
        best.map(|i| self.entries[i as usize].item)
    }

    /// Every entry tested without the grid, for the equivalence test.
    #[cfg(test)]
    pub fn hit_scan(&self, x: f64, y: f64) -> Option<SelectedItem> {
        let point = Point::new(x, y);
        self.entries
            .iter()
            .rev()
            .find(|e| e.hits(point))
            .map(|e| e.item)
    }
}

fn segment_entry(item: SelectedItem, a: Point, b: Point, tolerance: f64) -> Entry {
    Entry {
        item,
        extent: Aabb::new(a.x, a.y, b.x, b.y).expand(tolerance),
        shape: Shape::Segment { a, b, tolerance },
    }
}
