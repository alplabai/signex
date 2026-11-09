//! Net signatures for a sheet that differs from a known one by a few elements.
//!
//! A drag step moves a handful of symbols and the wires on them, yet
//! [`net_signatures`](crate::net_signatures) derives every net of the sheet
//! again. [`NetBase`] derives the pre-move sheet once and keeps its physical
//! clusters (wires, dots, labels, pins, ports) keyed by union-find root.
//! [`NetBase::derive`] then diffs a later sheet against it by element id and
//! rebuilds only the clusters the change can reach; the other clusters are
//! copied from the base, and the nets are joined by name over all of them.
//!
//! The answer is the one `net_signatures` gives, for any sheet: a sheet the
//! base cannot judge (changed library symbols, repeated element ids) is
//! derived in full. What "reach" means, and why it is enough, is written up in
//! `docs/INCREMENTAL_NETS.md`.

mod delta;
mod derive;
mod level1;
mod merge;
mod records;

pub use delta::{NetDelta, point_is_occupied};

use std::collections::HashMap;

use signex_types::schematic::{LabelType, LibSymbolTable, Point, SchematicSheet};
use uuid::Uuid;

use crate::build::{SegmentIndex, pt_key};
use crate::uf::Key;
use level1::{Cluster, Level1, compress, derive_level1};
use records::{ItemGrid, Kind, Rec, SymGeom};

/// One element of the base, by kind and position in the base's list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Elem {
    Wire(usize),
    Junction(usize),
    Label(usize),
    NoConnect(usize),
    Symbol(usize),
    SheetPin(usize),
}

/// A sheet's derived connectivity, kept to derive near-identical sheets from.
pub struct NetBase {
    /// False when the base cannot be diffed (two elements share an id).
    usable: bool,
    lib_symbols: LibSymbolTable,
    wires: Kind<(Point, Point)>,
    junctions: Kind<Point>,
    labels: Kind<(Point, LabelType, String)>,
    no_connects: Kind<Point>,
    symbols: Kind<SymGeom>,
    sheet_pins: Kind<Point>,
    /// Every union-find node to its root.
    root: HashMap<Key, Key>,
    clusters: HashMap<Key, Cluster>,
    /// Elements by the root of each key they sit on (a symbol under each of
    /// its pin tips, connected or not).
    buckets: HashMap<Key, Vec<Elem>>,
    segments: SegmentIndex,
    items: ItemGrid,
    /// What the delta derive (`delta.rs`) reads of the base nets.
    names: delta::BaseNets,
}

impl std::fmt::Debug for NetBase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetBase")
            .field("usable", &self.usable)
            .field("clusters", &self.clusters.len())
            .finish_non_exhaustive()
    }
}

/// `(id, position)` of every child-sheet pin.
fn sheet_pin_list(sheet: &SchematicSheet) -> Vec<(Uuid, Point)> {
    sheet
        .child_sheets
        .iter()
        .flat_map(|c| c.pins.iter().map(|p| (p.uuid, p.position)))
        .collect()
}

/// The six kinds of element the derivation reads, as the base remembers them.
struct Kinds {
    wires: Kind<(Point, Point)>,
    junctions: Kind<Point>,
    labels: Kind<(Point, LabelType, String)>,
    no_connects: Kind<Point>,
    symbols: Kind<SymGeom>,
    sheet_pins: Kind<Point>,
}

fn rec<G>(uuid: Uuid, geom: G) -> Rec<G> {
    Rec { uuid, geom }
}

impl Kinds {
    fn of(sheet: &SchematicSheet, pins: &[(Uuid, Point)]) -> Self {
        Self {
            wires: Kind::new(
                sheet
                    .wires
                    .iter()
                    .map(|w| rec(w.uuid, (w.start, w.end)))
                    .collect(),
            ),
            junctions: Kind::new(
                sheet
                    .junctions
                    .iter()
                    .map(|j| rec(j.uuid, j.position))
                    .collect(),
            ),
            labels: Kind::new(
                sheet
                    .labels
                    .iter()
                    .map(|l| rec(l.uuid, (l.position, l.label_type, l.text.clone())))
                    .collect(),
            ),
            no_connects: Kind::new(
                sheet
                    .no_connects
                    .iter()
                    .map(|n| rec(n.uuid, n.position))
                    .collect(),
            ),
            symbols: Kind::new(
                sheet
                    .symbols
                    .iter()
                    .map(|s| rec(s.uuid, SymGeom::of(sheet, s)))
                    .collect(),
            ),
            sheet_pins: Kind::new(pins.iter().map(|(u, at)| rec(*u, *at)).collect()),
        }
    }

    /// Whether the diff can tell every element of every kind apart.
    fn unique(&self) -> bool {
        self.wires.unique
            && self.junctions.unique
            && self.labels.unique
            && self.no_connects.unique
            && self.symbols.unique
            && self.sheet_pins.unique
    }

    /// Every element under the root of each point it sits on.
    fn buckets(&self, rb: impl Fn(Key) -> Key) -> HashMap<Key, Vec<Elem>> {
        let mut buckets: HashMap<Key, Vec<Elem>> = HashMap::new();
        let mut put = |k: Key, elem: Elem| buckets.entry(rb(k)).or_default().push(elem);
        for (at, rec) in self.wires.recs.iter().enumerate() {
            put(pt_key(&rec.geom.0), Elem::Wire(at));
        }
        for (at, rec) in self.junctions.recs.iter().enumerate() {
            put(pt_key(&rec.geom), Elem::Junction(at));
        }
        for (at, rec) in self.labels.recs.iter().enumerate() {
            put(pt_key(&rec.geom.0), Elem::Label(at));
        }
        for (at, rec) in self.no_connects.recs.iter().enumerate() {
            put(pt_key(&rec.geom), Elem::NoConnect(at));
        }
        for (at, rec) in self.symbols.recs.iter().enumerate() {
            for tip in &rec.geom.tips {
                put(*tip, Elem::Symbol(at));
            }
        }
        for (at, rec) in self.sheet_pins.recs.iter().enumerate() {
            put(pt_key(&rec.geom), Elem::SheetPin(at));
        }
        buckets
    }

    /// The points a wire can pass through.
    fn items(&self) -> ItemGrid {
        ItemGrid::new(
            (self.labels.recs.iter().map(|r| pt_key(&r.geom.0)))
                .chain(self.junctions.recs.iter().map(|r| pt_key(&r.geom)))
                .chain(self.sheet_pins.recs.iter().map(|r| pt_key(&r.geom))),
        )
    }
}

impl NetBase {
    /// Derive `sheet` once.
    #[must_use]
    pub fn new(sheet: &SchematicSheet) -> Self {
        let pins = sheet_pin_list(sheet);
        let kinds = Kinds::of(sheet, &pins);
        let Level1 {
            mut parent,
            clusters,
            index,
        } = derive_level1(sheet, &pins, &|_| true);
        let root = compress(&mut parent);
        let buckets = kinds.buckets(|k| root.get(&k).copied().unwrap_or(k));
        let items = kinds.items();
        let names = delta::BaseNets::new(sheet, &clusters);
        Self {
            usable: kinds.unique(),
            lib_symbols: sheet.lib_symbols.clone(),
            wires: kinds.wires,
            junctions: kinds.junctions,
            labels: kinds.labels,
            no_connects: kinds.no_connects,
            symbols: kinds.symbols,
            sheet_pins: kinds.sheet_pins,
            root,
            clusters,
            buckets,
            segments: index,
            items,
            names,
        }
    }
}
