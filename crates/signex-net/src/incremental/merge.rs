//! Level two of the derivation: clusters joined by name, then the nets.
//!
//! The rules are the netlist's (`merge_named_labels` and `merge_power_ports`):
//! same-text `Net` / `Global` / `Power` labels join, and `Global` / `Power`
//! labels join the power ports of the same value. The name of a net is the
//! best of its labels and port values; a net with no label takes the value of
//! the first port in sheet order.

use std::cell::OnceCell;
use std::collections::{BTreeSet, HashMap};

use signex_types::schematic::{LabelType, SchematicSheet};
use uuid::Uuid;

use super::level1::Cluster;
use crate::build::best_net_name_of;
use crate::signature::NetSignature;
use crate::uf::Key;

/// The nets of a set of clusters.
pub(super) struct Merged {
    /// Sorted, like `net_signatures`.
    pub(super) nets: Vec<NetSignature>,
    /// Per input cluster: the opaque id of its net and its position in `nets`,
    /// `None` when the net has nothing to identify it.
    pub(super) of_cluster: Vec<(Key, Option<usize>)>,
}

fn root_of(uf: &mut [usize], mut x: usize) -> usize {
    while uf[x] != x {
        uf[x] = uf[uf[x]];
        x = uf[x];
    }
    x
}

#[derive(Default)]
struct Acc<'a> {
    id: Option<Key>,
    terminals: BTreeSet<(Uuid, String)>,
    labels: BTreeSet<Uuid>,
    sheet_pins: BTreeSet<Uuid>,
    named: Vec<(LabelType, &'a str)>,
    ports: Vec<(Uuid, &'a str)>,
}

impl Acc<'_> {
    fn is_net(&self) -> bool {
        !(self.terminals.is_empty() && self.labels.is_empty() && self.sheet_pins.is_empty())
    }
}

/// The first of `ports` in sheet order. Only ports of different values need
/// the order, so it is read from the sheet when they do.
fn first_port<'a>(
    ports: &[(Uuid, &'a str)],
    sheet: &SchematicSheet,
    order: &OnceCell<HashMap<Uuid, usize>>,
) -> Option<&'a str> {
    let (_, head) = ports.first()?;
    if ports.iter().all(|(_, value)| value == head) {
        return Some(*head);
    }
    let order = order.get_or_init(|| {
        let mut at = HashMap::new();
        for (index, symbol) in sheet.symbols.iter().enumerate() {
            at.entry(symbol.uuid).or_insert(index);
        }
        at
    });
    ports
        .iter()
        .min_by_key(|(symbol, value)| (order.get(symbol).copied().unwrap_or(usize::MAX), *value))
        .map(|(_, value)| *value)
}

/// A name that joins clusters: `power` false for the label namespace (`Net`,
/// `Global` and `Power` labels of one text), true for the power namespace
/// (`Global` / `Power` labels and power ports of one value).
pub(super) type GroupKey = (bool, String);

/// Every group `cluster` belongs to; the one place the joining rules live.
pub(super) fn name_groups(cluster: &Cluster) -> Vec<GroupKey> {
    let mut groups = Vec::new();
    for label in &cluster.labels {
        if label.text.is_empty() {
            continue;
        }
        if matches!(
            label.kind,
            LabelType::Global | LabelType::Power | LabelType::Net
        ) {
            groups.push((false, label.text.clone()));
        }
        if matches!(label.kind, LabelType::Global | LabelType::Power) {
            groups.push((true, label.text.clone()));
        }
    }
    for (_, value) in &cluster.ports {
        if !value.is_empty() {
            groups.push((true, value.clone()));
        }
    }
    groups
}

/// The union-find over `clusters` after the name joins.
fn join_by_name(clusters: &[(Key, &Cluster)]) -> Vec<usize> {
    let mut uf: Vec<usize> = (0..clusters.len()).collect();
    let mut groups: HashMap<GroupKey, usize> = HashMap::new();
    for (at, (_, cluster)) in clusters.iter().enumerate() {
        for key in name_groups(cluster) {
            match groups.get(&key) {
                Some(&other) => {
                    let (a, b) = (root_of(&mut uf, at), root_of(&mut uf, other));
                    if a != b {
                        uf[a.max(b)] = a.min(b);
                    }
                }
                None => {
                    groups.insert(key, at);
                }
            }
        }
    }
    uf
}

/// What each joined class holds, and the class of every cluster.
fn accumulate<'a>(
    clusters: &[(Key, &'a Cluster)],
    uf: &mut [usize],
) -> (HashMap<usize, Acc<'a>>, Vec<usize>) {
    let mut classes: HashMap<usize, Acc<'a>> = HashMap::new();
    let mut class_of: Vec<usize> = Vec::with_capacity(clusters.len());
    for (at, (root, cluster)) in clusters.iter().enumerate() {
        let class = root_of(uf, at);
        class_of.push(class);
        let acc = classes.entry(class).or_default();
        acc.id = Some(acc.id.map_or(*root, |id| id.min(*root)));
        acc.terminals.extend(cluster.terminals.iter().cloned());
        acc.sheet_pins.extend(cluster.sheet_pins.iter().copied());
        for label in &cluster.labels {
            acc.labels.insert(label.uuid);
            acc.named.push((label.kind, label.text.as_str()));
        }
        acc.ports
            .extend(cluster.ports.iter().map(|(s, v)| (*s, v.as_str())));
    }
    (classes, class_of)
}

/// The signature of every class that is a net, with its class, sorted.
fn signatures(
    classes: HashMap<usize, Acc<'_>>,
    sheet: &SchematicSheet,
) -> Vec<(NetSignature, usize)> {
    let order: OnceCell<HashMap<Uuid, usize>> = OnceCell::new();
    let mut nets: Vec<(NetSignature, usize)> = Vec::new();
    for (class, acc) in classes {
        if !acc.is_net() {
            continue;
        }
        let name = if acc.named.is_empty() {
            first_port(&acc.ports, sheet, &order).map(str::to_string)
        } else {
            let values: Vec<&str> = acc.ports.iter().map(|(_, v)| *v).collect();
            best_net_name_of(acc.named.iter().copied(), &values).map(|(_, name)| name)
        };
        nets.push((
            NetSignature {
                terminals: acc.terminals,
                labels: acc.labels,
                sheet_pins: acc.sheet_pins,
                name: name.unwrap_or_default(),
            },
            class,
        ));
    }
    nets.sort_by(|a, b| a.0.cmp(&b.0));
    nets
}

/// Join `clusters` (root key, cluster) by name and build the nets. `sheet`
/// supplies the symbol order for the rare net that needs it.
pub(super) fn merge(clusters: &[(Key, &Cluster)], sheet: &SchematicSheet) -> Merged {
    let mut uf = join_by_name(clusters);
    let (classes, class_of) = accumulate(clusters, &mut uf);
    let ids: HashMap<usize, Key> = classes
        .iter()
        .map(|(class, acc)| (*class, acc.id.unwrap_or_default()))
        .collect();
    let nets = signatures(classes, sheet);
    let position: HashMap<usize, usize> = nets
        .iter()
        .enumerate()
        .map(|(at, (_, class))| (*class, at))
        .collect();
    let of_cluster = class_of
        .iter()
        .map(|class| (ids[class], position.get(class).copied()))
        .collect();
    Merged {
        nets: nets.into_iter().map(|(net, _)| net).collect(),
        of_cluster,
    }
}
