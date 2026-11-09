//! The nets of a sheet that differs from the base, as a delta against the
//! base nets: which base nets are gone and which nets stand in their place.
//!
//! [`NetBase::derive`] returns every net of the sheet, which costs the number
//! of nets however little changed. [`NetBase::derive_delta`] returns only the
//! nets that differ from the base's, so a caller that compares or reshapes
//! the nets near a change pays for the change.
//!
//! Why `nets(sheet) = (base nets - removed) + added`, exactly.
//!
//! * The clusters (level one) of `sheet` are the base clusters outside the
//!   region plus the rebuilt ones (`derive.rs`).
//! * A net is a class of clusters joined by name ([`name_groups`]). Take the
//!   *local* clusters: the rebuilt ones, then every base cluster outside the
//!   region that shares a group with a local cluster or with a base cluster
//!   inside the region (the clusters the change replaced), again and again
//!   until no outside cluster shares a group with a local one.
//! * The classes of the local clusters are computed by [`merge`] over the
//!   local set alone. That is the whole class: any cluster of `sheet` joined
//!   to a local one shares a group with it, so it is local too (rebuilt
//!   clusters are all local, outside clusters carry their base groups).
//! * Every other class is made of outside clusters none of which shares a
//!   group with a local or replaced cluster, so it is a base class,
//!   unchanged, and its signature is the base net's.
//! * `removed` is every base net that holds a replaced or local cluster: the
//!   nets of the region's clusters and of the local outside clusters. Any
//!   other base net is such a class, and stays.
//!
//! The delta is only offered when the base's pins identify one net each (no
//! `(symbol, pin)` in two base nets), so a caller can find the base net a pin
//! sits on by that pin alone.

use std::collections::{HashMap, HashSet};

use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

use super::NetBase;
use super::level1::{Cluster, Level1, derive_level1};
use super::merge::{GroupKey, merge, name_groups};
use crate::build::{point_on_segment, pt_key};
use crate::signature::NetSignature;
use crate::uf::Key;

/// The nets of the base sheet, indexed for the delta derive.
pub(super) struct BaseNets {
    nets: Vec<NetSignature>,
    /// Per base cluster root: the position in `nets` of the net it is in;
    /// absent when its class has nothing to identify it.
    net_of_root: HashMap<Key, usize>,
    /// Base clusters by the group that joins them.
    groups: HashMap<GroupKey, Vec<Key>>,
    /// The base net a pin sits on.
    terminal_net: HashMap<(Uuid, String), usize>,
    /// No pin is in two base nets.
    unique_pins: bool,
}

impl BaseNets {
    pub(super) fn new(sheet: &SchematicSheet, clusters: &HashMap<Key, Cluster>) -> Self {
        let all: Vec<(Key, &Cluster)> = clusters.iter().map(|(r, c)| (*r, c)).collect();
        let merged = merge(&all, sheet);
        let mut net_of_root = HashMap::new();
        let mut groups: HashMap<GroupKey, Vec<Key>> = HashMap::new();
        for ((root, cluster), (_, at)) in all.iter().zip(&merged.of_cluster) {
            if let Some(at) = at {
                net_of_root.insert(*root, *at);
            }
            for key in name_groups(cluster) {
                groups.entry(key).or_default().push(*root);
            }
        }
        let mut terminal_net = HashMap::new();
        let mut unique_pins = true;
        for (at, net) in merged.nets.iter().enumerate() {
            for terminal in &net.terminals {
                unique_pins &= terminal_net.insert(terminal.clone(), at).is_none();
            }
        }
        Self {
            nets: merged.nets,
            net_of_root,
            groups,
            terminal_net,
            unique_pins,
        }
    }
}

/// How a sheet's nets differ from the base's.
#[derive(Debug, Clone, Default)]
pub struct NetDelta {
    /// Positions in [`NetBase::nets`] of the base nets that are gone (or
    /// changed): sorted, once each.
    pub removed: Vec<usize>,
    /// The nets that take their place, sorted: every net of the sheet that
    /// holds a cluster the change reached.
    pub added: Vec<NetSignature>,
    /// Per queried point: the opaque id of the net it sits on, and that net's
    /// position in `added` (`None` when nothing identifies the net).
    pub found: Vec<(Key, Option<usize>)>,
    /// Per queried point: whether a wire (end or interior), label, junction
    /// dot or no-connect of the sheet lies on it.
    pub occupied: Vec<bool>,
}

impl NetBase {
    /// The nets of the base sheet, sorted.
    #[must_use]
    pub fn nets(&self) -> &[NetSignature] {
        &self.names.nets
    }

    /// The position in [`Self::nets`] of the base net holding the pin
    /// `(symbol, pin)`.
    #[must_use]
    pub fn net_of_pin(&self, symbol: Uuid, pin: &str) -> Option<usize> {
        self.names
            .terminal_net
            .get(&(symbol, pin.to_string()))
            .copied()
    }

    /// Whether [`Self::derive_delta`] can answer for a sheet at all: the base
    /// can diff a later sheet and its pins identify one net each.
    #[must_use]
    pub fn delta_ready(&self) -> bool {
        self.usable && self.names.unique_pins
    }

    /// The nets of `sheet` as a [`NetDelta`], with the nets at the points `at`
    /// (`(point key, anchor to a wire's interior)`, as `net_signatures_at`).
    /// `None` when the base cannot judge `sheet` (see [`Self::derive_at`]) or
    /// is not [`Self::delta_ready`].
    #[must_use]
    pub fn derive_delta(&self, cand: &SchematicSheet, at: &[(Key, bool)]) -> Option<NetDelta> {
        if !self.delta_ready() || !cand.lib_symbols.shares_storage_with(&self.lib_symbols) {
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

        // The rebuilt clusters first, then the base clusters they are joined to.
        let mut local: Vec<(Key, &Cluster)> = clusters.iter().map(|(root, c)| (*root, c)).collect();
        let rebuilt = local.len();
        let mut pending: Vec<GroupKey> = local.iter().flat_map(|(_, c)| name_groups(c)).collect();
        let mut replaced: Vec<Key> = Vec::new();
        for root in &region {
            replaced.push(*root);
            if let Some(cluster) = self.clusters.get(root) {
                pending.extend(name_groups(cluster));
            }
        }
        let mut seen: HashSet<GroupKey> = HashSet::new();
        let mut taken: HashSet<Key> = HashSet::new();
        while let Some(group) = pending.pop() {
            if seen.contains(&group) {
                continue;
            }
            for root in self.names.groups.get(&group).map_or(&[][..], Vec::as_slice) {
                if region.contains(root) || !taken.insert(*root) {
                    continue;
                }
                let cluster = &self.clusters[root];
                pending.extend(name_groups(cluster));
                local.push((*root, cluster));
                replaced.push(*root);
            }
            seen.insert(group);
        }
        let merged = merge(&local, cand);

        let mut removed: Vec<usize> = replaced
            .iter()
            .filter_map(|root| self.names.net_of_root.get(root).copied())
            .collect();
        removed.sort_unstable();
        removed.dedup();

        let rebuilt_at: HashMap<Key, usize> = local[..rebuilt]
            .iter()
            .enumerate()
            .map(|(i, (root, _))| (*root, i))
            .collect();
        let found = Self::locate(at, &mut parent, &index, &rebuilt_at, &merged);
        let occupied = at.iter().map(|(key, _)| occupied(&sub, *key)).collect();
        Some(NetDelta {
            removed,
            added: merged.nets,
            found,
            occupied,
        })
    }

    /// All the nets of the sheet a [`NetDelta`] describes, sorted: what
    /// [`Self::derive`] gives for it.
    #[must_use]
    pub fn nets_with(&self, delta: &NetDelta) -> Vec<NetSignature> {
        let gone: HashSet<usize> = delta.removed.iter().copied().collect();
        let mut all: Vec<NetSignature> = (self.names.nets.iter().enumerate())
            .filter(|(at, _)| !gone.contains(at))
            .map(|(_, net)| net.clone())
            .collect();
        all.extend(delta.added.iter().cloned());
        all.sort();
        all
    }
}

/// Whether a wire, label, junction dot or no-connect of `sheet` lies on `key`.
fn occupied(sheet: &SchematicSheet, key: Key) -> bool {
    sheet
        .wires
        .iter()
        .any(|w| point_on_segment(key, pt_key(&w.start), pt_key(&w.end)))
        || sheet.labels.iter().any(|l| pt_key(&l.position) == key)
        || sheet.junctions.iter().any(|j| pt_key(&j.position) == key)
        || sheet.no_connects.iter().any(|n| pt_key(&n.position) == key)
}
