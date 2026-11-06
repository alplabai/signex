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

mod derive;
mod level1;
mod merge;
mod records;

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

impl NetBase {
    /// Derive `sheet` once.
    #[must_use]
    pub fn new(sheet: &SchematicSheet) -> Self {
        let wires = Kind::new(
            sheet
                .wires
                .iter()
                .map(|w| Rec {
                    uuid: w.uuid,
                    geom: (w.start, w.end),
                })
                .collect(),
        );
        let junctions = Kind::new(
            sheet
                .junctions
                .iter()
                .map(|j| Rec {
                    uuid: j.uuid,
                    geom: j.position,
                })
                .collect(),
        );
        let labels = Kind::new(
            sheet
                .labels
                .iter()
                .map(|l| Rec {
                    uuid: l.uuid,
                    geom: (l.position, l.label_type, l.text.clone()),
                })
                .collect(),
        );
        let no_connects = Kind::new(
            sheet
                .no_connects
                .iter()
                .map(|n| Rec {
                    uuid: n.uuid,
                    geom: n.position,
                })
                .collect(),
        );
        let symbols = Kind::new(
            sheet
                .symbols
                .iter()
                .map(|s| Rec {
                    uuid: s.uuid,
                    geom: SymGeom::of(sheet, s),
                })
                .collect(),
        );
        let pins = sheet_pin_list(sheet);
        let sheet_pins = Kind::new(
            pins.iter()
                .map(|(uuid, at)| Rec {
                    uuid: *uuid,
                    geom: *at,
                })
                .collect(),
        );
        let usable = wires.unique
            && junctions.unique
            && labels.unique
            && no_connects.unique
            && symbols.unique
            && sheet_pins.unique;

        let Level1 {
            mut parent,
            clusters,
            index,
        } = derive_level1(sheet, &pins, &|_| true);
        let root = compress(&mut parent);
        let rb = |k: Key| root.get(&k).copied().unwrap_or(k);
        let mut buckets: HashMap<Key, Vec<Elem>> = HashMap::new();
        let mut put = |k: Key, elem: Elem| buckets.entry(rb(k)).or_default().push(elem);
        for (at, rec) in wires.recs.iter().enumerate() {
            put(pt_key(&rec.geom.0), Elem::Wire(at));
        }
        for (at, rec) in junctions.recs.iter().enumerate() {
            put(pt_key(&rec.geom), Elem::Junction(at));
        }
        for (at, rec) in labels.recs.iter().enumerate() {
            put(pt_key(&rec.geom.0), Elem::Label(at));
        }
        for (at, rec) in no_connects.recs.iter().enumerate() {
            put(pt_key(&rec.geom), Elem::NoConnect(at));
        }
        for (at, rec) in symbols.recs.iter().enumerate() {
            for tip in &rec.geom.tips {
                put(*tip, Elem::Symbol(at));
            }
        }
        for (at, rec) in sheet_pins.recs.iter().enumerate() {
            put(pt_key(&rec.geom), Elem::SheetPin(at));
        }
        let items = ItemGrid::new(
            labels
                .recs
                .iter()
                .map(|r| pt_key(&r.geom.0))
                .chain(junctions.recs.iter().map(|r| pt_key(&r.geom)))
                .chain(sheet_pins.recs.iter().map(|r| pt_key(&r.geom))),
        );
        Self {
            usable,
            lib_symbols: sheet.lib_symbols.clone(),
            wires,
            junctions,
            labels,
            no_connects,
            symbols,
            sheet_pins,
            root,
            clusters,
            buckets,
            segments: index,
            items,
        }
    }
}
