//! Hit-test internals — spatial-hash builder + per-primitive distance
//! / containment helpers used by the public
//! [`super::hit_test_point`] / [`super::hit_test_box`] entries.
//!
//! The spatial hash buckets every primitive's world-space AABB into
//! a grid keyed by `(i32, i32)` cells of [`CELL_SIZE_MM`]. A point
//! query touches only the buckets that overlap the cursor + tolerance
//! pad — O(k) where k is the bucket population near the cursor. A box
//! query inflates the touched-bucket set to every cell the query box
//! overlaps and applies the [`super::SelectionMode`] rule on each
//! candidate.
//!
//! Z-order: primitives are appended in render order, and queries
//! traverse them in reverse so the topmost item wins. Render order
//! comes from [`super::render`] and is documented in [`build_index`].

use std::collections::HashMap;

use signex_types::schematic::{
    Aabb, Point, SchDrawing, SelectedItem, SelectedKind, point_to_segment_dist,
};

use super::{SchematicSnapshot, SelectionMode};

/// Spatial-hash cell size in millimetres. Sized to comfortably enclose
/// the typical schematic primitive (≈ 1 cell per pin or junction);
/// large primitives (symbols, multi-segment polylines) span several
/// cells and are inserted into each overlapping cell.
pub const CELL_SIZE_MM: f64 = 5.08;

#[derive(Debug, Clone)]
struct HitEntry {
    item: SelectedItem,
    bbox: Aabb,
    z: usize,
}

/// Spatial-hash hit index. See module doc.
#[derive(Debug, Default, Clone)]
pub struct HitIndex {
    buckets: HashMap<(i32, i32), Vec<usize>>,
    entries: Vec<HitEntry>,
}

impl HitIndex {
    /// Build the index from a snapshot. Render order:
    /// `drawings → buses → wires → bus_entries → child_sheets →
    /// symbols → junctions → no_connects → labels → text_notes`.
    /// Hit-test queries traverse this list in reverse so labels and
    /// text notes win over wires and drawings underneath.
    pub fn build(snapshot: &SchematicSnapshot<'_>) -> Self {
        let mut index = HitIndex::default();
        let sheet = snapshot.sheet;

        for d in &sheet.drawings {
            index.insert(
                drawing_uuid(d),
                SelectedKind::Drawing,
                super::drawing::drawing_aabb(d),
            );
        }
        for b in &sheet.buses {
            index.insert(b.uuid, SelectedKind::Bus, super::bus::bus_aabb(b));
        }
        for w in &sheet.wires {
            index.insert(w.uuid, SelectedKind::Wire, super::wire::wire_aabb(w));
        }
        for e in &sheet.bus_entries {
            index.insert(
                e.uuid,
                SelectedKind::BusEntry,
                super::bus_entry::bus_entry_aabb(e),
            );
        }
        for s in &sheet.symbols {
            if let Some(lib) = snapshot.lib_symbol(&s.lib_id) {
                index.insert(
                    s.uuid,
                    SelectedKind::Symbol,
                    super::symbol::symbol_aabb(s, lib),
                );
            }
        }
        for j in &sheet.junctions {
            index.insert(
                j.uuid,
                SelectedKind::Junction,
                super::junction::junction_aabb(j),
            );
        }
        for nc in &sheet.no_connects {
            index.insert(
                nc.uuid,
                SelectedKind::NoConnect,
                super::no_connect::no_connect_aabb(nc),
            );
        }
        for l in &sheet.labels {
            index.insert(l.uuid, SelectedKind::Label, super::label::label_aabb(l));
        }
        for n in &sheet.text_notes {
            index.insert(
                n.uuid,
                SelectedKind::TextNote,
                super::text::text_note_aabb(n),
            );
        }
        index
    }

    /// World-space bounding box of an indexed `SelectedItem`. `None`
    /// when the item isn't in the index.
    pub fn aabb_of(&self, item: &SelectedItem) -> Option<Aabb> {
        self.entries
            .iter()
            .find(|e| e.item == *item)
            .map(|e| e.bbox)
    }

    fn insert(&mut self, uuid: uuid::Uuid, kind: SelectedKind, bbox: Aabb) {
        let z = self.entries.len();
        self.entries.push(HitEntry {
            item: SelectedItem::new(uuid, kind),
            bbox,
            z,
        });
        for (cx, cy) in cells_for_aabb(&bbox) {
            self.buckets.entry((cx, cy)).or_default().push(z);
        }
    }

    fn entries_in_aabb(&self, bbox: &Aabb) -> Vec<&HitEntry> {
        let mut seen: Vec<usize> = Vec::new();
        for cell in cells_for_aabb(bbox) {
            if let Some(ids) = self.buckets.get(&cell) {
                for id in ids {
                    if !seen.contains(id) {
                        seen.push(*id);
                    }
                }
            }
        }
        seen.iter().filter_map(|i| self.entries.get(*i)).collect()
    }
}

fn cells_for_aabb(bbox: &Aabb) -> Vec<(i32, i32)> {
    let cell = |v: f64| (v / CELL_SIZE_MM).floor() as i32;
    let x0 = cell(bbox.min_x);
    let x1 = cell(bbox.max_x);
    let y0 = cell(bbox.min_y);
    let y1 = cell(bbox.max_y);
    let mut cells = Vec::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            cells.push((x, y));
        }
    }
    cells
}

#[inline]
fn drawing_uuid(d: &SchDrawing) -> uuid::Uuid {
    match d {
        SchDrawing::Line { uuid, .. }
        | SchDrawing::Rect { uuid, .. }
        | SchDrawing::Circle { uuid, .. }
        | SchDrawing::Arc { uuid, .. }
        | SchDrawing::Polyline { uuid, .. } => *uuid,
    }
}

/// Public hit-test entry — see [`super::hit_test_point`].
pub fn point(
    index: &HitIndex,
    snapshot: &SchematicSnapshot<'_>,
    point_world: Point,
    tolerance_world: f64,
) -> Option<SelectedItem> {
    let pad = tolerance_world.max(0.0);
    let query = Aabb::new(
        point_world.x - pad,
        point_world.y - pad,
        point_world.x + pad,
        point_world.y + pad,
    );
    let mut candidates = index.entries_in_aabb(&query);
    // Topmost first.
    candidates.sort_by(|a, b| b.z.cmp(&a.z));
    candidates
        .into_iter()
        .find(|entry| primitive_hit_point(snapshot, entry.item, point_world, tolerance_world))
        .map(|entry| entry.item)
}

/// Public box hit-test entry — see [`super::hit_test_box`].
pub fn box_query(
    index: &HitIndex,
    snapshot: &SchematicSnapshot<'_>,
    box_world: Aabb,
    mode: SelectionMode,
) -> Vec<SelectedItem> {
    let mut hits: Vec<&HitEntry> = index.entries_in_aabb(&box_world);
    hits.sort_by_key(|e| e.z);
    hits.into_iter()
        .filter(|entry| primitive_hit_box(snapshot, entry.item, &entry.bbox, &box_world, mode))
        .map(|entry| entry.item)
        .collect()
}

fn primitive_hit_point(
    snapshot: &SchematicSnapshot<'_>,
    item: SelectedItem,
    p: Point,
    tol: f64,
) -> bool {
    match item.kind {
        SelectedKind::Wire => snapshot
            .sheet
            .wires
            .iter()
            .find(|w| w.uuid == item.uuid)
            .map(|w| point_to_segment_dist(p.x, p.y, w.start.x, w.start.y, w.end.x, w.end.y) <= tol)
            .unwrap_or(false),
        SelectedKind::Bus => snapshot
            .sheet
            .buses
            .iter()
            .find(|b| b.uuid == item.uuid)
            .map(|b| point_to_segment_dist(p.x, p.y, b.start.x, b.start.y, b.end.x, b.end.y) <= tol)
            .unwrap_or(false),
        SelectedKind::BusEntry => snapshot
            .sheet
            .bus_entries
            .iter()
            .find(|e| e.uuid == item.uuid)
            .map(|e| {
                let end = Point::new(e.position.x + e.size.0, e.position.y + e.size.1);
                point_to_segment_dist(p.x, p.y, e.position.x, e.position.y, end.x, end.y) <= tol
            })
            .unwrap_or(false),
        SelectedKind::Junction => snapshot
            .sheet
            .junctions
            .iter()
            .find(|j| j.uuid == item.uuid)
            .map(|j| {
                let r = super::junction::effective_diameter_mm(j) * 0.5 + tol;
                ((j.position.x - p.x).powi(2) + (j.position.y - p.y).powi(2)).sqrt() <= r
            })
            .unwrap_or(false),
        SelectedKind::NoConnect => snapshot
            .sheet
            .no_connects
            .iter()
            .find(|n| n.uuid == item.uuid)
            .map(|n| {
                let h = super::no_connect::NO_CONNECT_HALF_SIZE_MM + tol;
                (p.x - n.position.x).abs() <= h && (p.y - n.position.y).abs() <= h
            })
            .unwrap_or(false),
        SelectedKind::Symbol
        | SelectedKind::ChildSheet
        | SelectedKind::Label
        | SelectedKind::TextNote
        | SelectedKind::Drawing
        | SelectedKind::SheetPin
        | SelectedKind::SymbolRefField
        | SelectedKind::SymbolValField => {
            // For body-shaped primitives we use AABB containment as a
            // first-pass hit. Wave 4 keeps this simple; future tuning
            // can replace each arm with a tighter shape test.
            let bbox = match item.kind {
                SelectedKind::Symbol => snapshot
                    .sheet
                    .symbols
                    .iter()
                    .find(|s| s.uuid == item.uuid)
                    .and_then(|s| {
                        snapshot
                            .lib_symbol(&s.lib_id)
                            .map(|lib| super::symbol::symbol_aabb(s, lib))
                    }),
                SelectedKind::Label => snapshot
                    .sheet
                    .labels
                    .iter()
                    .find(|l| l.uuid == item.uuid)
                    .map(super::label::label_aabb),
                SelectedKind::TextNote => snapshot
                    .sheet
                    .text_notes
                    .iter()
                    .find(|n| n.uuid == item.uuid)
                    .map(super::text::text_note_aabb),
                SelectedKind::Drawing => snapshot
                    .sheet
                    .drawings
                    .iter()
                    .find(|d| drawing_uuid(d) == item.uuid)
                    .map(super::drawing::drawing_aabb),
                _ => None,
            };
            bbox.map(|b| b.expand(tol).contains(p.x, p.y))
                .unwrap_or(false)
        }
    }
}

fn primitive_hit_box(
    _snapshot: &SchematicSnapshot<'_>,
    _item: SelectedItem,
    item_bbox: &Aabb,
    box_world: &Aabb,
    mode: SelectionMode,
) -> bool {
    match mode {
        SelectionMode::Single => box_world.contains(
            (item_bbox.min_x + item_bbox.max_x) * 0.5,
            (item_bbox.min_y + item_bbox.max_y) * 0.5,
        ),
        SelectionMode::Enclosing => {
            // Item fully inside the query box.
            box_world.min_x <= item_bbox.min_x
                && box_world.max_x >= item_bbox.max_x
                && box_world.min_y <= item_bbox.min_y
                && box_world.max_y >= item_bbox.max_y
        }
        SelectionMode::Crossing => super::util::aabbs_overlap(item_bbox, box_world),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{SchematicSheet, Wire};
    use uuid::Uuid;

    fn snap_with_wires(wires: Vec<Wire>) -> SchematicSheet {
        SchematicSheet {
            uuid: Uuid::new_v4(),
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: String::new(),
            root_sheet_page: "1".to_string(),
            symbols: Vec::new(),
            wires,
            junctions: Vec::new(),
            labels: Vec::new(),
            child_sheets: Vec::new(),
            no_connects: Vec::new(),
            text_notes: Vec::new(),
            buses: Vec::new(),
            bus_entries: Vec::new(),
            drawings: Vec::new(),
            no_erc_directives: Vec::new(),
            title_block: Default::default(),
            lib_symbols: Default::default(),
        }
    }

    fn make_wire(start: (f64, f64), end: (f64, f64)) -> Wire {
        Wire {
            uuid: Uuid::new_v4(),
            start: Point::new(start.0, start.1),
            end: Point::new(end.0, end.1),
            stroke_width: 0.0,
        }
    }

    #[test]
    fn point_query_finds_wire_under_cursor() {
        let w = make_wire((0.0, 0.0), (10.0, 0.0));
        let target = w.uuid;
        let sheet = snap_with_wires(vec![w]);
        let theme = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::Signex);
        let snap = SchematicSnapshot::new(&sheet, &theme);
        let index = HitIndex::build(&snap);
        let hit = point(&index, &snap, Point::new(5.0, 0.05), 0.5);
        assert_eq!(hit.map(|i| i.uuid), Some(target));
    }

    #[test]
    fn point_query_misses_when_outside_tolerance() {
        let w = make_wire((0.0, 0.0), (10.0, 0.0));
        let sheet = snap_with_wires(vec![w]);
        let theme = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::Signex);
        let snap = SchematicSnapshot::new(&sheet, &theme);
        let index = HitIndex::build(&snap);
        let hit = point(&index, &snap, Point::new(5.0, 5.0), 0.1);
        assert!(hit.is_none());
    }

    #[test]
    fn box_query_enclosing_excludes_partially_overlapping_items() {
        let inside = make_wire((1.0, 1.0), (2.0, 2.0));
        let crossing = make_wire((0.5, 0.5), (5.0, 0.5));
        let inside_uuid = inside.uuid;
        let sheet = snap_with_wires(vec![inside, crossing]);
        let theme = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::Signex);
        let snap = SchematicSnapshot::new(&sheet, &theme);
        let index = HitIndex::build(&snap);
        let q = Aabb::new(0.0, 0.0, 3.0, 3.0);
        let hits_enclose = box_query(&index, &snap, q, SelectionMode::Enclosing);
        let hits_cross = box_query(&index, &snap, q, SelectionMode::Crossing);
        assert_eq!(hits_enclose.len(), 1);
        assert_eq!(hits_enclose[0].uuid, inside_uuid);
        assert_eq!(hits_cross.len(), 2);
    }

    #[test]
    fn cells_for_aabb_returns_at_least_one_cell() {
        let cells = cells_for_aabb(&Aabb::new(0.0, 0.0, 0.0, 0.0));
        assert!(!cells.is_empty());
    }
}
