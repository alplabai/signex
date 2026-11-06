//! Level one of the derivation: the physical clusters.
//!
//! Wires, junction dots, labels and child-sheet pins anchored to the wire they
//! sit on, with the component pins, labels and power ports that land on each
//! cluster. Joining clusters by name is level two (`merge`). The pieces are the
//! ones `net_signatures` is made of, so a cluster here is exactly a piece of
//! the net it ends up in.

use std::collections::HashMap;

use signex_types::schematic::{InstancePath, LabelType, Point, SchematicSheet};
use uuid::Uuid;

use crate::build::{
    SegmentIndex, SheetConnectivity, anchor_point, collect_net_labels, collect_terminals_at,
    power_name_carriers, pt_key,
};
use crate::uf::{Key, find};

/// A label as the name rules read it.
#[derive(Debug, Clone)]
pub(super) struct LabelRec {
    pub(super) uuid: Uuid,
    pub(super) kind: LabelType,
    pub(super) text: String,
}

/// What lands on one physical cluster.
#[derive(Debug, Clone, Default)]
pub(super) struct Cluster {
    pub(super) terminals: Vec<(Uuid, String)>,
    pub(super) labels: Vec<LabelRec>,
    pub(super) sheet_pins: Vec<Uuid>,
    /// Power-port pins that name and join a net by value: `(symbol, value)`.
    pub(super) ports: Vec<(Uuid, String)>,
}

/// The physical union-find of a (part of a) sheet and its clusters by root.
pub(super) struct Level1 {
    pub(super) parent: HashMap<Key, Key>,
    pub(super) clusters: HashMap<Key, Cluster>,
    pub(super) index: SegmentIndex,
}

/// Derive the clusters of `sheet`. `keep` says whether a pin or port tip
/// belongs to the part being derived: a symbol that only partly lies in it
/// reports just the pins that do.
pub(super) fn derive_level1(
    sheet: &SchematicSheet,
    sheet_pins: &[(Uuid, Point)],
    keep: &dyn Fn(Key) -> bool,
) -> Level1 {
    let wires: Vec<(Point, Point)> = sheet.wires.iter().map(|w| (w.start, w.end)).collect();
    let junctions: Vec<Point> = sheet.junctions.iter().map(|j| j.position).collect();
    let mut parent = SheetConnectivity::from_segments(&wires, &junctions).into_parent();
    let index = SegmentIndex::new(&wires);
    for label in &sheet.labels {
        anchor_point(&mut parent, pt_key(&label.position), &index);
    }
    for (_, at) in sheet_pins {
        anchor_point(&mut parent, pt_key(at), &index);
    }
    let terminals = collect_terminals_at(sheet, &InstancePath::root(), &mut parent);
    let labels = collect_net_labels(sheet, &mut parent);
    let ports = power_name_carriers(sheet, &mut parent);

    let mut clusters: HashMap<Key, Cluster> = HashMap::new();
    for (root, found) in terminals {
        for (terminal, at) in found {
            if keep(pt_key(&at)) {
                clusters
                    .entry(root)
                    .or_default()
                    .terminals
                    .push((terminal.symbol, terminal.pin));
            }
        }
    }
    for (uuid, at) in sheet_pins {
        let root = find(&mut parent, pt_key(at));
        clusters.entry(root).or_default().sheet_pins.push(*uuid);
    }
    for (root, list) in labels {
        let cluster = clusters.entry(root).or_default();
        cluster.labels.extend(list.into_iter().map(|l| LabelRec {
            uuid: l.uuid,
            kind: l.label_type,
            text: l.text.clone(),
        }));
    }
    for (root, value, tip, symbol) in ports {
        if keep(pt_key(&tip)) {
            clusters
                .entry(root)
                .or_default()
                .ports
                .push((symbol, value));
        }
    }
    Level1 {
        parent,
        clusters,
        index,
    }
}

/// Every key of `parent` mapped to its root, so later reads need no `&mut`.
pub(super) fn compress(parent: &mut HashMap<Key, Key>) -> HashMap<Key, Key> {
    let keys: Vec<Key> = parent.keys().copied().collect();
    keys.into_iter().map(|k| (k, find(parent, k))).collect()
}
