//! Deriving a sheet against the base: what changed, which clusters that can
//! reach, and the nets of the sheet with those clusters rebuilt.

use std::collections::{HashMap, HashSet};

use signex_types::schematic::{Point, SchematicSheet};
use uuid::Uuid;

use super::level1::{Cluster, Level1, derive_level1};
use super::merge::{Merged, merge};
use super::records::{Diff, diff, symbol_tips};
use super::{Elem, NetBase, sheet_pin_list};
use crate::build::{SegmentIndex, pt_key};
use crate::signature::{NetSignature, net_signatures_at};
use crate::uf::{Key, find, union as uf_union};

/// A later sheet against the base, element kind by element kind.
struct Diffs {
    wires: Diff,
    junctions: Diff,
    labels: Diff,
    no_connects: Diff,
    symbols: Diff,
    pins: Diff,
    cand_pins: Vec<(Uuid, Point)>,
}

type Answer = (Vec<NetSignature>, Vec<(Key, Option<usize>)>);

impl NetBase {
    /// The nets of `sheet`, as [`net_signatures`](crate::net_signatures)
    /// gives them.
    #[must_use]
    pub fn derive(&self, sheet: &SchematicSheet) -> Vec<NetSignature> {
        self.derive_at(sheet, &[]).0
    }

    /// [`net_signatures_at`] for `sheet`.
    #[must_use]
    pub fn derive_at(&self, sheet: &SchematicSheet, at: &[(Key, bool)]) -> Answer {
        self.incremental(sheet, at)
            .unwrap_or_else(|| net_signatures_at(sheet, at))
    }

    /// Whether [`derive`](Self::derive) finds `sheet`'s nets from the base
    /// rather than in full (it does not when the base cannot judge the
    /// sheet: its library symbols changed, or the base has two elements with one id).
    #[must_use]
    pub fn derives_incrementally(&self, sheet: &SchematicSheet) -> bool {
        self.incremental(sheet, &[]).is_some()
    }

    fn root_of(&self, key: Key) -> Key {
        self.root.get(&key).copied().unwrap_or(key)
    }

    fn diffs(&self, cand: &SchematicSheet) -> Diffs {
        let cand_pins = sheet_pin_list(cand);
        Diffs {
            wires: diff(
                &self.wires,
                &cand.wires,
                |w| w.uuid,
                |g, w| g.0 == w.start && g.1 == w.end,
            ),
            junctions: diff(
                &self.junctions,
                &cand.junctions,
                |j| j.uuid,
                |g, j| *g == j.position,
            ),
            labels: diff(
                &self.labels,
                &cand.labels,
                |l| l.uuid,
                |g, l| g.0 == l.position && g.1 == l.label_type && g.2 == l.text,
            ),
            no_connects: diff(
                &self.no_connects,
                &cand.no_connects,
                |n| n.uuid,
                |g, n| *g == n.position,
            ),
            symbols: diff(
                &self.symbols,
                &cand.symbols,
                |s| s.uuid,
                |g, s| g.matches(s),
            ),
            pins: diff(&self.sheet_pins, &cand_pins, |p| p.0, |g, p| *g == p.1),
            cand_pins,
        }
    }

    /// The keys the change touches, and the wire segments it adds or drops
    /// (before and after). `at` keys are queried, so they count.
    fn dirty(
        &self,
        d: &Diffs,
        cand: &SchematicSheet,
        at: &[(Key, bool)],
    ) -> (Vec<Key>, Vec<(Key, Key)>) {
        let mut keys: Vec<Key> = at.iter().map(|(k, _)| *k).collect();
        let mut segments: Vec<(Key, Key)> = Vec::new();
        for &slot in &d.wires.removed {
            let (a, b) = self.wires.recs[slot].geom;
            segments.push((pt_key(&a), pt_key(&b)));
        }
        for &i in &d.wires.added {
            let w = &cand.wires[i];
            segments.push((pt_key(&w.start), pt_key(&w.end)));
        }
        for &(a, b) in &segments {
            keys.push(a);
            keys.push(b);
        }
        self.dirty_points(d, cand, &mut keys);
        self.dirty_symbols(d, cand, &mut keys);
        (keys, segments)
    }

    /// Positions of the dots, labels, no-connects and child-sheet pins that
    /// went or came.
    fn dirty_points(&self, d: &Diffs, cand: &SchematicSheet, keys: &mut Vec<Key>) {
        let pk = pt_key;
        keys.extend(
            d.junctions
                .removed
                .iter()
                .map(|&s| pk(&self.junctions.recs[s].geom)),
        );
        keys.extend(
            d.junctions
                .added
                .iter()
                .map(|&i| pk(&cand.junctions[i].position)),
        );
        keys.extend(
            d.labels
                .removed
                .iter()
                .map(|&s| pk(&self.labels.recs[s].geom.0)),
        );
        keys.extend(d.labels.added.iter().map(|&i| pk(&cand.labels[i].position)));
        keys.extend(
            d.no_connects
                .removed
                .iter()
                .map(|&s| pk(&self.no_connects.recs[s].geom)),
        );
        keys.extend(
            d.no_connects
                .added
                .iter()
                .map(|&i| pk(&cand.no_connects[i].position)),
        );
        keys.extend(
            d.pins
                .removed
                .iter()
                .map(|&s| pk(&self.sheet_pins.recs[s].geom)),
        );
        keys.extend(d.pins.added.iter().map(|&i| pk(&d.cand_pins[i].1)));
    }

    /// Every pin tip of the symbols that went or came, connected or not.
    fn dirty_symbols(&self, d: &Diffs, cand: &SchematicSheet, keys: &mut Vec<Key>) {
        for &slot in &d.symbols.removed {
            keys.extend(self.symbols.recs[slot].geom.tips.iter().copied());
        }
        for &i in &d.symbols.added {
            keys.extend(symbol_tips(cand, &cand.symbols[i]));
        }
    }

    /// The roots of the base clusters the change can reach. A key reaches the
    /// cluster it sits on and every cluster of a wire passing through it (a
    /// dot or label there is anchored to one of them, and an endpoint there
    /// takes the anchor away). A label, dot or child-sheet pin lying on an
    /// added or dropped wire is reached through its own key the same way.
    fn reach(&self, keys: Vec<Key>, segments: &[(Key, Key)]) -> HashSet<Key> {
        let mut work = keys;
        for &(a, b) in segments {
            work.extend(self.items.on_segment(a, b));
        }
        let mut region: HashSet<Key> = HashSet::new();
        let mut expanded: HashSet<Key> = HashSet::new();
        while let Some(key) = work.pop() {
            if !expanded.insert(key) {
                continue;
            }
            region.insert(self.root_of(key));
            for (start, _) in self.segments.containing(key) {
                region.insert(self.root_of(start));
            }
        }
        region
    }

    /// The part of `cand` inside `region`, as a sheet of its own plus its
    /// child-sheet pins: the unchanged elements the region's buckets hold,
    /// and everything added or changed.
    fn region_sheet(
        &self,
        region: &HashSet<Key>,
        d: &Diffs,
        cand: &SchematicSheet,
    ) -> (SchematicSheet, Vec<(Uuid, Point)>) {
        let mut sub = SchematicSheet {
            lib_symbols: cand.lib_symbols.clone(),
            ..SchematicSheet::default()
        };
        let mut pins: Vec<(Uuid, Point)> = Vec::new();
        let mut taken: HashSet<Elem> = HashSet::new();
        for root in region {
            for &elem in self.buckets.get(root).map(Vec::as_slice).unwrap_or(&[]) {
                if taken.insert(elem) {
                    Self::keep_unchanged(elem, d, cand, &mut sub, &mut pins);
                }
            }
        }
        sub.wires
            .extend(d.wires.added.iter().map(|&i| cand.wires[i].clone()));
        sub.junctions
            .extend(d.junctions.added.iter().map(|&i| cand.junctions[i].clone()));
        sub.labels
            .extend(d.labels.added.iter().map(|&i| cand.labels[i].clone()));
        sub.no_connects.extend(
            d.no_connects
                .added
                .iter()
                .map(|&i| cand.no_connects[i].clone()),
        );
        sub.symbols
            .extend(d.symbols.added.iter().map(|&i| cand.symbols[i].clone()));
        pins.extend(d.pins.added.iter().map(|&i| d.cand_pins[i]));
        (sub, pins)
    }

    /// Copy base element `elem` into `sub` from `cand` when it is unchanged.
    fn keep_unchanged(
        elem: Elem,
        d: &Diffs,
        cand: &SchematicSheet,
        sub: &mut SchematicSheet,
        pins: &mut Vec<(Uuid, Point)>,
    ) {
        match elem {
            Elem::Wire(s) => sub
                .wires
                .extend(d.wires.kept_at(s).map(|i| cand.wires[i].clone())),
            Elem::Junction(s) => sub
                .junctions
                .extend(d.junctions.kept_at(s).map(|i| cand.junctions[i].clone())),
            Elem::Label(s) => sub
                .labels
                .extend(d.labels.kept_at(s).map(|i| cand.labels[i].clone())),
            Elem::NoConnect(s) => sub.no_connects.extend(
                d.no_connects
                    .kept_at(s)
                    .map(|i| cand.no_connects[i].clone()),
            ),
            Elem::Symbol(s) => sub
                .symbols
                .extend(d.symbols.kept_at(s).map(|i| cand.symbols[i].clone())),
            Elem::SheetPin(s) => pins.extend(d.pins.kept_at(s).map(|i| d.cand_pins[i])),
        }
    }

    /// Where each queried point sits, as the full derive reports it: a point
    /// flagged as on a wire's interior is joined to that wire in the union-find
    /// that already holds the name joins, one point after another, and its net
    /// is then looked up by the root the nets were numbered with before those
    /// joins. A point that is already a node of another net (a pin tip held by
    /// a no-connect, say) therefore merges the two classes for the queries
    /// after it, and a net found by a stale root is found or missed exactly as
    /// there.
    fn locate(
        at: &[(Key, bool)],
        parent: &mut HashMap<Key, Key>,
        index: &SegmentIndex,
        rebuilt_at: &HashMap<Key, usize>,
        merged: &Merged,
    ) -> Vec<(Key, Option<usize>)> {
        let by_id: HashMap<Key, Option<usize>> = merged.of_cluster.iter().copied().collect();
        // The root a point's class had before any query joined it.
        let class_of = |parent: &mut HashMap<Key, Key>, key: Key| {
            let root = find(parent, key);
            rebuilt_at
                .get(&root)
                .map_or(root, |&i| merged.of_cluster[i].0)
        };
        let mut joined: HashMap<Key, Key> = HashMap::new();
        for &(key, on_interior) in at {
            if !on_interior || index.is_endpoint(key) {
                continue;
            }
            if let Some((a, b)) = index.anchor_segment(key) {
                let (point, wire) = (class_of(parent, key), class_of(parent, a.min(b)));
                uf_union(&mut joined, point, wire);
            }
        }
        at.iter()
            .map(|&(key, _)| {
                let root = find(&mut joined, class_of(parent, key));
                (root, by_id.get(&root).copied().flatten())
            })
            .collect()
    }

    fn incremental(&self, cand: &SchematicSheet, at: &[(Key, bool)]) -> Option<Answer> {
        if !self.usable || !cand.lib_symbols.shares_storage_with(&self.lib_symbols) {
            return None;
        }
        let d = self.diffs(cand);
        let (keys, segments) = self.dirty(&d, cand, at);
        let region = self.reach(keys, &segments);
        let (sub, sub_pins) = self.region_sheet(&region, &d, cand);
        let inside = |key: Key| region.contains(&self.root_of(key));
        let Level1 {
            mut parent,
            clusters,
            index,
        } = derive_level1(&sub, &sub_pins, &inside);
        let mut all: Vec<(Key, &Cluster)> = self
            .clusters
            .iter()
            .filter(|(root, _)| !region.contains(*root))
            .map(|(root, cluster)| (*root, cluster))
            .collect();
        let rebuilt = all.len();
        all.extend(clusters.iter().map(|(root, cluster)| (*root, cluster)));
        let merged = merge(&all, cand);

        let rebuilt_at: HashMap<Key, usize> = all[rebuilt..]
            .iter()
            .enumerate()
            .map(|(i, (root, _))| (*root, rebuilt + i))
            .collect();
        let found = Self::locate(at, &mut parent, &index, &rebuilt_at, &merged);
        Some((merged.nets, found))
    }
}
