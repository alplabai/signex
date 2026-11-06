//! What the base remembers of the sheet, and the diff of a later sheet
//! against it.

use std::collections::HashMap;

use signex_types::schematic::{
    BodyStyle, Point, SchematicSheet, Symbol, SymbolTransform, instance_pins,
};
use uuid::Uuid;

use crate::build::{point_on_segment, pt_key};
use crate::uf::Key;

/// One element: its id and the geometry the derivation reads.
#[derive(Debug)]
pub(super) struct Rec<G> {
    pub(super) uuid: Uuid,
    pub(super) geom: G,
}

/// The elements of one kind, by position and by id. `unique` is false when
/// two share an id, which the diff cannot tell apart.
pub(super) struct Kind<G> {
    pub(super) recs: Vec<Rec<G>>,
    pub(super) ids: HashMap<Uuid, usize>,
    pub(super) unique: bool,
}

impl<G> Kind<G> {
    pub(super) fn new(recs: Vec<Rec<G>>) -> Self {
        let mut ids = HashMap::with_capacity(recs.len());
        for (at, rec) in recs.iter().enumerate() {
            ids.entry(rec.uuid).or_insert(at);
        }
        let unique = ids.len() == recs.len();
        Self { recs, ids, unique }
    }
}

/// What the derivation reads of a placed symbol.
#[derive(Debug)]
pub(super) struct SymGeom {
    lib_id: String,
    position: Point,
    rotation: f64,
    mirror_x: bool,
    mirror_y: bool,
    unit: u32,
    body_style: BodyStyle,
    is_power: bool,
    value: String,
    /// Where every pin of the placed unit ends, connected or not.
    pub(super) tips: Vec<Key>,
}

impl SymGeom {
    pub(super) fn of(sheet: &SchematicSheet, sym: &Symbol) -> Self {
        Self {
            lib_id: sym.lib_id.clone(),
            position: sym.position,
            rotation: sym.rotation,
            mirror_x: sym.mirror_x,
            mirror_y: sym.mirror_y,
            unit: sym.unit,
            body_style: sym.body_style,
            is_power: sym.is_power,
            value: sym.value.clone(),
            tips: symbol_tips(sheet, sym),
        }
    }

    pub(super) fn matches(&self, sym: &Symbol) -> bool {
        self.position == sym.position
            && self.rotation == sym.rotation
            && self.mirror_x == sym.mirror_x
            && self.mirror_y == sym.mirror_y
            && self.unit == sym.unit
            && self.body_style == sym.body_style
            && self.is_power == sym.is_power
            && self.lib_id == sym.lib_id
            && self.value == sym.value
    }
}

/// Where every pin of `sym` ends in `sheet`, in the 1 um key space.
pub(super) fn symbol_tips(sheet: &SchematicSheet, sym: &Symbol) -> Vec<Key> {
    let Some(lib) = sheet.resolve_lib_symbol(sym) else {
        return Vec::new();
    };
    let xform = SymbolTransform::from_symbol(sym);
    let tips: Vec<Key> = instance_pins(&lib, sym)
        .map(|lp| pt_key(&xform.apply(lp.pin.position)))
        .collect();
    tips
}

/// Marks a base element the later sheet no longer has unchanged.
pub(super) const GONE: usize = usize::MAX;

/// How a later sheet differs from the base, for one kind of element.
pub(super) struct Diff {
    /// Base positions that are gone or changed.
    pub(super) removed: Vec<usize>,
    /// Positions in the later sheet that are new or changed.
    pub(super) added: Vec<usize>,
    /// Per base position: where the unchanged element sits in the later
    /// sheet, [`GONE`] when it is not unchanged.
    pub(super) kept: Vec<usize>,
}

impl Diff {
    pub(super) fn kept_at(&self, base: usize) -> Option<usize> {
        Some(self.kept[base]).filter(|&at| at != GONE)
    }
}

/// Match `cand` against `base` by id; `same` decides whether a match is
/// unchanged. An id met twice counts as a new element the second time.
pub(super) fn diff<G, C>(
    base: &Kind<G>,
    cand: &[C],
    uuid_of: impl Fn(&C) -> Uuid,
    same: impl Fn(&G, &C) -> bool,
) -> Diff {
    let mut kept = vec![GONE; base.recs.len()];
    let mut seen = vec![false; base.recs.len()];
    let mut added = Vec::new();
    let mut removed = Vec::new();
    for (at, item) in cand.iter().enumerate() {
        let id = uuid_of(item);
        // Most edits keep the order, so the same slot is the likely match.
        let found = match base.recs.get(at) {
            Some(rec) if rec.uuid == id => Some(at),
            _ => base.ids.get(&id).copied(),
        };
        match found {
            Some(slot) if !seen[slot] => {
                seen[slot] = true;
                if same(&base.recs[slot].geom, item) {
                    kept[slot] = at;
                } else {
                    removed.push(slot);
                    added.push(at);
                }
            }
            _ => added.push(at),
        }
    }
    removed.extend((0..base.recs.len()).filter(|&slot| !seen[slot]));
    Diff {
        removed,
        added,
        kept,
    }
}

/// Grid cell edge in 1 um keys (2^14 = 16.384 mm), as in `SegmentIndex`.
const CELL_SHIFT: u32 = 14;
/// A segment spanning more cells than this scans every point instead.
const MAX_CELLS: i64 = 256;

type Cell = (i64, i64);

fn cell_of(k: Key) -> Cell {
    (k.0 >> CELL_SHIFT, k.1 >> CELL_SHIFT)
}

/// Points that can sit in the interior of a wire (labels, junction dots,
/// child-sheet pins), found by the segment they lie on.
pub(super) struct ItemGrid {
    cells: HashMap<Cell, Vec<Key>>,
    all: Vec<Key>,
}

impl ItemGrid {
    pub(super) fn new(keys: impl IntoIterator<Item = Key>) -> Self {
        let mut cells: HashMap<Cell, Vec<Key>> = HashMap::new();
        let mut all = Vec::new();
        for key in keys {
            cells.entry(cell_of(key)).or_default().push(key);
            all.push(key);
        }
        Self { cells, all }
    }

    /// Every point on segment `a`-`b`, endpoints included.
    pub(super) fn on_segment(&self, a: Key, b: Key) -> Vec<Key> {
        let lo = cell_of((a.0.min(b.0), a.1.min(b.1)));
        let hi = cell_of((a.0.max(b.0), a.1.max(b.1)));
        let span = (hi.0 - lo.0 + 1).saturating_mul(hi.1 - lo.1 + 1);
        if span > MAX_CELLS {
            return self
                .all
                .iter()
                .copied()
                .filter(|&k| point_on_segment(k, a, b))
                .collect();
        }
        let mut found = Vec::new();
        for cx in lo.0..=hi.0 {
            for cy in lo.1..=hi.1 {
                if let Some(keys) = self.cells.get(&(cx, cy)) {
                    found.extend(keys.iter().copied().filter(|&k| point_on_segment(k, a, b)));
                }
            }
        }
        found
    }
}
