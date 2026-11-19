//! Signal harness connectivity (#1280). A harness wire carries every entry of
//! its type at once; this module turns that into ordinary nets.
//!
//! Per sheet, a **bundle** is a union-find over harness wires (end to end and
//! T contacts, by [`pt_key`]), connector tips, harness-typed sheet-entry
//! points and Hierarchical / Global labels lying on a harness wire (such a
//! label is a harness *port*, named by its text). Within a bundle the nets
//! wired to the entry named `E` on every connector are one net, named
//! `<Type>.<E>` unless a net label names it. An entry whose point touches
//! another bundle (a harness wire from a second connector's tip) nests that
//! bundle: its entries join as `<Type>.<E>.<F>`. Across the hierarchy a
//! harness-typed sheet entry `N` joins the parent bundle it touches with the
//! child's bundle carrying the port `N`, entry by entry by path. A sheet with
//! no harness takes none of these steps. Rules and rationale:
//! `docs/SIGNAL_HARNESSES.md`.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use signex_types::schematic::{LabelType, Point, SchematicSheet, normalize_port_name};
use uuid::Uuid;

use crate::build::{SegmentIndex, pt_key};
use crate::uf::{Key, find, union};

/// A harness-typed sheet entry lying on a bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessSheetEntry {
    pub sheet_symbol: Uuid,
    pub pin: Uuid,
    pub name: String,
    pub harness_type: String,
}

/// A Hierarchical or Global label lying on a harness wire: the port a parent
/// sheet's harness-typed entry of the same name binds to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessPort {
    pub label: Uuid,
    pub name: String,
}

/// One connected harness of a sheet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HarnessBundle {
    /// Every harness type its connectors and typed sheet entries carry,
    /// sorted. Two or more is a type conflict.
    pub types: Vec<String>,
    /// Its connectors, in sheet order.
    pub connectors: Vec<Uuid>,
    pub sheet_entries: Vec<HarnessSheetEntry>,
    pub ports: Vec<HarnessPort>,
    pub wires: Vec<Uuid>,
    /// The entry names the bundle's connectors carry, in first-seen order.
    pub entry_names: Vec<String>,
    /// Whether the bundle hangs off an entry of another connector (nested).
    pub nested: bool,
}

impl HarnessBundle {
    /// The type the bundle's net names use: the first of [`Self::types`].
    pub fn net_type(&self) -> Option<&str> {
        self.types.first().map(String::as_str)
    }
}

/// The harness union-find of one sheet.
struct Topology<'a> {
    sheet: &'a SchematicSheet,
    parent: HashMap<Key, Key>,
    index: SegmentIndex,
}

impl<'a> Topology<'a> {
    fn new(sheet: &'a SchematicSheet) -> Self {
        let segments: Vec<(Point, Point)> = sheet
            .harness
            .wires
            .iter()
            .map(|w| (w.start, w.end))
            .collect();
        let index = SegmentIndex::new(&segments);
        let mut topo = Self {
            sheet,
            parent: HashMap::new(),
            index,
        };
        for (start, end) in &segments {
            union(&mut topo.parent, pt_key(start), pt_key(end));
        }
        // A harness wire ending on another one joins it (a T contact needs no
        // dot: a bundle is typed, never two nets crossing).
        for (start, end) in &segments {
            topo.attach(*start);
            topo.attach(*end);
        }
        for c in &sheet.harness.connectors {
            topo.attach(c.tip_point());
        }
        for (_, at) in typed_entries(sheet) {
            topo.attach(at);
        }
        for label in port_labels(sheet) {
            if topo.index.contains(pt_key(&label.position)) {
                topo.attach(label.position);
            }
        }
        topo
    }

    /// Make `p` a node and join it to every harness wire it lies on.
    fn attach(&mut self, p: Point) {
        let pk = pt_key(&p);
        find(&mut self.parent, pk);
        let on: Vec<Key> = self.index.containing(pk).map(|(a, _)| a).collect();
        for a in on {
            union(&mut self.parent, pk, a);
        }
    }

    /// The bundle `p` touches: a node of the union-find, or a point on a
    /// harness wire.
    fn class_at(&mut self, p: Point) -> Option<Key> {
        let pk = pt_key(&p);
        if self.parent.contains_key(&pk) {
            return Some(find(&mut self.parent, pk));
        }
        let (a, _) = self.index.containing(pk).next()?;
        Some(find(&mut self.parent, a))
    }

    fn tip_class(&mut self, connector: usize) -> Key {
        let tip = self.sheet.harness.connectors[connector].tip_point();
        find(&mut self.parent, pt_key(&tip))
    }

    /// The bundle nested at entry `entry` of connector `connector`, if its
    /// point touches a bundle other than the connector's own.
    fn nested_at(&mut self, connector: usize, entry: usize) -> Option<Key> {
        let own = self.tip_class(connector);
        let at = self.sheet.harness.connectors[connector].entry_point(entry);
        self.class_at(at).filter(|class| *class != own)
    }

    /// Every leaf signal of the bundle `class`: `(path, net point)`, where a
    /// nested entry contributes its sub-bundle's leaves under its own name.
    fn leaves(&mut self, class: Key, seen: &mut HashSet<Key>) -> Vec<(Vec<String>, Key)> {
        let mut out = Vec::new();
        if !seen.insert(class) {
            return out;
        }
        for ci in 0..self.sheet.harness.connectors.len() {
            if self.tip_class(ci) != class {
                continue;
            }
            for ei in 0..self.sheet.harness.connectors[ci].entries.len() {
                let name = self.sheet.harness.connectors[ci].entries[ei].name.clone();
                match self.nested_at(ci, ei) {
                    Some(sub) => {
                        for (mut path, key) in self.leaves(sub, seen) {
                            path.insert(0, name.clone());
                            out.push((path, key));
                        }
                    }
                    None => {
                        let at = self.sheet.harness.connectors[ci].entry_point(ei);
                        out.push((vec![name], pt_key(&at)));
                    }
                }
            }
        }
        out
    }

    /// Every bundle class some entry nests.
    fn nested_classes(&mut self) -> HashSet<Key> {
        let mut out = HashSet::new();
        for ci in 0..self.sheet.harness.connectors.len() {
            for ei in 0..self.sheet.harness.connectors[ci].entries.len() {
                if let Some(sub) = self.nested_at(ci, ei) {
                    out.insert(sub);
                }
            }
        }
        out
    }

    /// The type of the bundle `class`: its connectors' and typed entries'
    /// types, sorted.
    fn types_of(&mut self, class: Key) -> Vec<String> {
        let mut types = BTreeSet::new();
        for ci in 0..self.sheet.harness.connectors.len() {
            if self.tip_class(ci) == class {
                types.insert(self.sheet.harness.connectors[ci].harness_type.clone());
            }
        }
        for (ty, at) in typed_entries(self.sheet) {
            if self.class_at(at) == Some(class) {
                types.insert(ty.to_string());
            }
        }
        types.into_iter().filter(|t| !t.trim().is_empty()).collect()
    }

    /// Every top-level bundle class, in a stable order.
    fn top_classes(&mut self) -> Vec<Key> {
        let nested = self.nested_classes();
        let mut classes = BTreeSet::new();
        for ci in 0..self.sheet.harness.connectors.len() {
            classes.insert(self.tip_class(ci));
        }
        for (_, at) in typed_entries(self.sheet) {
            if let Some(class) = self.class_at(at) {
                classes.insert(class);
            }
        }
        classes
            .into_iter()
            .filter(|c| !nested.contains(c))
            .collect()
    }
}

/// Every harness-typed sheet entry of `sheet` as `(type, point)`.
fn typed_entries(sheet: &SchematicSheet) -> Vec<(&str, Point)> {
    sheet
        .child_sheets
        .iter()
        .flat_map(|cs| cs.pins.iter())
        .filter_map(|p| sheet.harness.entry_type(p.uuid).map(|t| (t, p.position)))
        .collect()
}

/// The labels that can be harness ports: Hierarchical and Global.
fn port_labels(sheet: &SchematicSheet) -> impl Iterator<Item = &signex_types::schematic::Label> {
    sheet
        .labels
        .iter()
        .filter(|l| matches!(l.label_type, LabelType::Hierarchical | LabelType::Global))
}

/// The leaves of a bundle grouped by path, paths joined with `.`.
fn by_path(leaves: Vec<(Vec<String>, Key)>) -> BTreeMap<String, Vec<Key>> {
    let mut out: BTreeMap<String, Vec<Key>> = BTreeMap::new();
    for (path, key) in leaves {
        out.entry(path.join(".")).or_default().push(key);
    }
    out
}

/// Join, in the net union-find `parent`, the nets wired to same-path entries
/// of every bundle of `sheet`. A sheet without harness objects is left as it
/// is. The one call [`crate::build`]'s per-sheet derivation makes.
pub(crate) fn join_entry_nets(sheet: &SchematicSheet, parent: &mut HashMap<Key, Key>) {
    if sheet.harness.connectors.is_empty() {
        return;
    }
    let mut topo = Topology::new(sheet);
    for class in topo.top_classes() {
        let leaves = topo.leaves(class, &mut HashSet::new());
        for keys in by_path(leaves).into_values() {
            let Some((&first, rest)) = keys.split_first() else {
                continue;
            };
            find(parent, first);
            for &k in rest {
                union(parent, first, k);
            }
        }
    }
}

/// The leaves of one bundle as `(path, net root)`, sampled from `parent`.
pub(crate) type Leaves = Vec<(String, Key)>;

/// What the netlist reads from a sheet's harnesses, sampled after every
/// level-1 union.
#[derive(Debug, Default)]
pub(crate) struct SheetHarnessNets {
    /// Net root -> its compiled name `<Type>.<path>`, the name a net takes
    /// when no label names it.
    pub(crate) names: HashMap<Key, String>,
    /// Per `child_sheets` entry: each harness-typed sheet entry's name with
    /// the leaves of the bundle it touches.
    pub(crate) pins: Vec<Vec<(String, Leaves)>>,
    /// Each harness port's name with the leaves of its bundle.
    pub(crate) ports: Vec<(String, Leaves)>,
}

/// Sample `sheet`'s harness nets from the fully merged net union-find
/// `parent`. Empty for a sheet without harness objects.
pub(crate) fn sample(sheet: &SchematicSheet, parent: &mut HashMap<Key, Key>) -> SheetHarnessNets {
    let mut out = SheetHarnessNets {
        pins: vec![Vec::new(); sheet.child_sheets.len()],
        ..Default::default()
    };
    if sheet.harness.connectors.is_empty() && sheet.harness.wires.is_empty() {
        return out;
    }
    let mut topo = Topology::new(sheet);
    for class in topo.top_classes() {
        let Some(ty) = topo.types_of(class).into_iter().next() else {
            continue;
        };
        for (path, root) in root_leaves(&mut topo, class, parent) {
            let name = format!("{ty}.{path}");
            let slot = out.names.entry(root).or_insert_with(|| name.clone());
            if name < *slot {
                *slot = name;
            }
        }
    }
    for (index, cs) in sheet.child_sheets.iter().enumerate() {
        for pin in &cs.pins {
            if sheet.harness.entry_type(pin.uuid).is_none() {
                continue;
            }
            let (Some(name), Some(class)) =
                (normalize_port_name(&pin.name), topo.class_at(pin.position))
            else {
                continue;
            };
            let leaves = root_leaves(&mut topo, class, parent);
            out.pins[index].push((name.to_string(), leaves));
        }
    }
    for label in port_labels(sheet) {
        if !topo.index.contains(pt_key(&label.position)) {
            continue;
        }
        let (Some(name), Some(class)) = (label.port_name(), topo.class_at(label.position)) else {
            continue;
        };
        let leaves = root_leaves(&mut topo, class, parent);
        out.ports.push((name.to_string(), leaves));
    }
    out
}

/// The leaves of the bundle `class` as `(path, net root)`, sorted.
fn root_leaves(topo: &mut Topology<'_>, class: Key, parent: &mut HashMap<Key, Key>) -> Leaves {
    let mut leaves: Leaves = Vec::new();
    for (path, keys) in by_path(topo.leaves(class, &mut HashSet::new())) {
        for k in keys {
            leaves.push((path.clone(), find(parent, k)));
        }
    }
    leaves.sort();
    leaves.dedup();
    leaves
}

/// Join, in the project's level-2 union-find, each harness-typed sheet entry
/// with the same-named harness port of its child, leaf by leaf of the same
/// path. `edges` are the stitcher's `(parent occurrence, sheet symbol index,
/// child occurrence)`; `harness_of` reads an occurrence's sampled harness.
pub(crate) fn join_across_sheets<'a>(
    l2: &mut HashMap<(usize, Key), (usize, Key)>,
    edges: &[(usize, usize, usize)],
    harness_of: impl Fn(usize) -> &'a SheetHarnessNets,
) {
    for &(parent_occ, cs_index, child_occ) in edges {
        let Some(pins) = harness_of(parent_occ).pins.get(cs_index) else {
            continue;
        };
        let ports = &harness_of(child_occ).ports;
        for (pin_name, pin_leaves) in pins {
            for (_, port_leaves) in ports.iter().filter(|(name, _)| name == pin_name) {
                for (path, pr) in pin_leaves {
                    for (_, cr) in port_leaves.iter().filter(|(p, _)| p == path) {
                        union(l2, (parent_occ, *pr), (child_occ, *cr));
                    }
                }
            }
        }
    }
}

/// Every bundle of `sheet`, top-level and nested, for ERC and the editor.
/// Empty for a sheet without harness objects.
pub fn harness_bundles(sheet: &SchematicSheet) -> Vec<HarnessBundle> {
    let h = &sheet.harness;
    if h.connectors.is_empty() && h.wires.is_empty() {
        return Vec::new();
    }
    let mut topo = Topology::new(sheet);
    let nested = topo.nested_classes();
    let mut bundles: BTreeMap<Key, HarnessBundle> = BTreeMap::new();
    for (ci, c) in h.connectors.iter().enumerate() {
        let class = topo.tip_class(ci);
        let b = bundles.entry(class).or_default();
        b.connectors.push(c.uuid);
        for e in &c.entries {
            if !b.entry_names.contains(&e.name) {
                b.entry_names.push(e.name.clone());
            }
        }
    }
    for w in &h.wires {
        let class = find(&mut topo.parent, pt_key(&w.start));
        bundles.entry(class).or_default().wires.push(w.uuid);
    }
    for cs in &sheet.child_sheets {
        for pin in &cs.pins {
            let Some(ty) = h.entry_type(pin.uuid) else {
                continue;
            };
            let Some(class) = topo.class_at(pin.position) else {
                continue;
            };
            bundles
                .entry(class)
                .or_default()
                .sheet_entries
                .push(HarnessSheetEntry {
                    sheet_symbol: cs.uuid,
                    pin: pin.uuid,
                    name: pin.name.trim().to_string(),
                    harness_type: ty.to_string(),
                });
        }
    }
    for label in port_labels(sheet) {
        if !topo.index.contains(pt_key(&label.position)) {
            continue;
        }
        let (Some(name), Some(class)) = (label.port_name(), topo.class_at(label.position)) else {
            continue;
        };
        bundles.entry(class).or_default().ports.push(HarnessPort {
            label: label.uuid,
            name: name.to_string(),
        });
    }
    let classes: Vec<Key> = bundles.keys().copied().collect();
    for class in classes {
        let types = topo.types_of(class);
        if let Some(b) = bundles.get_mut(&class) {
            b.types = types;
            b.nested = nested.contains(&class);
        }
    }
    bundles.into_values().collect()
}

/// Whether `p` lies on a harness wire of `sheet` (end or interior), so a
/// sheet entry or label there is joined by the harness, not left floating.
pub fn on_harness(sheet: &SchematicSheet, p: &Point) -> bool {
    harness_segments_touch(sheet.harness.wires.iter().map(|w| (w.start, w.end)), p)
}

/// [`on_harness`] over plain segments, for a caller holding its own copy.
pub fn harness_segments_touch(
    segments: impl IntoIterator<Item = (Point, Point)>,
    p: &Point,
) -> bool {
    let pk = pt_key(p);
    segments
        .into_iter()
        .any(|(a, b)| crate::build::point_on_segment(pk, pt_key(&a), pt_key(&b)))
}

/// One entry the far side of a harness expects and this side lacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessMismatch {
    /// The connector on this side.
    pub connector: Uuid,
    /// The entry of this side that stands where the expected one should
    /// (an entry the far side lacks), when there is one.
    pub entry: Option<Uuid>,
    /// That entry's name.
    pub has: Option<String>,
    /// The entry the far side carries and this side lacks.
    pub expects: String,
}

/// The entries `far` carries that the connector `entries` (`(uuid, name)`)
/// lacks, each paired, in order, with an entry of this side that `far`
/// lacks. Empty when both carry the same names.
pub fn harness_mismatches(
    connector: Uuid,
    entries: &[(Uuid, String)],
    far: &[String],
) -> Vec<HarnessMismatch> {
    let mut extra = entries.iter().filter(|(_, n)| !far.contains(n));
    far.iter()
        .filter(|name| !entries.iter().any(|(_, n)| n == *name))
        .map(|expects| {
            let has = extra.next();
            HarnessMismatch {
                connector,
                entry: has.map(|(id, _)| *id),
                has: has.map(|(_, n)| n.clone()),
                expects: expects.clone(),
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "harness_tests.rs"]
mod tests;
