//! The intent check of one headless point placement (#2199): a label, power
//! port, junction or no-connect. The caller applies the placement's real
//! engine command to a copy of the sheet and passes both; the two are
//! compared with the netlist's own connectivity and `analyse_buses`. Rules:
//! `docs/CLI.md`, "Labels, power ports, junctions and no-connects".
//!
//! There is one rule, as for a wire run:
//! * a name change: for nets, what `join_rule::needs_allow_merge` says (the
//!   project's rule, with hierarchical labels as aliases); for bus runs, two
//!   with different names end up as one, or a named run gains a name;
//! * for a pin reference only, extra: anything not on the pin's net before
//!   ends up on it (the end rules of [`wire_intent`]).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::Hash;

use signex_types::schematic::{Point, SchematicSheet, pin_tips};

use super::wire_intent::{
    EndSpec, ExtraItem, ExtraKind, names_of_root, net_names_of_root, same_names, wire_intent,
};
use super::{SegmentIndex, merged_connectivity, pt_key};
use crate::join_rule::{NetNames, needs_allow_merge};
use crate::uf::Key;

/// A net or bus run whose names change: the name sets of the named nets that
/// end up as one (`before`) and the names the result carries.
#[derive(Debug, Clone, PartialEq)]
pub struct NameChange {
    pub before: Vec<Vec<String>>,
    pub after: Vec<String>,
    /// A bus run, not a net.
    pub bus: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlacementIntent {
    /// Pin reference only: items pulled onto the pin's net.
    pub extra: Vec<ExtraItem>,
    pub changes: Vec<NameChange>,
}

/// Judge the placement that turned `before` into `after`; `pin` is the pin a
/// pin reference named, with its tip.
pub fn placement_intent(
    before: &SchematicSheet,
    after: &SchematicSheet,
    pin: Option<EndSpec>,
) -> PlacementIntent {
    let mut changes = net_changes(before, after);
    changes.extend(crate::bus_intent::bus_name_changes(before, after));
    let extra = pin.map_or_else(Vec::new, |spec| pin_extras(before, after, spec));
    PlacementIntent { extra, changes }
}

/// `sheet` with every name blanked, so no name joins nets: what is left is
/// geometry, which is what the extra check is about.
fn blanked(sheet: &SchematicSheet) -> SchematicSheet {
    let mut copy = sheet.clone();
    copy.labels.iter_mut().for_each(|l| l.text.clear());
    copy.symbols
        .iter_mut()
        .filter(|s| s.is_power)
        .for_each(|s| s.value.clear());
    copy
}

fn pin_extras(before: &SchematicSheet, after: &SchematicSheet, spec: EndSpec) -> Vec<ExtraItem> {
    // The placed item's own pin (a power port) is not something it pulls in.
    let mut after_geometry = blanked(after);
    let known: std::collections::HashSet<_> = before.symbols.iter().map(|s| s.uuid).collect();
    after_geometry.symbols.retain(|s| known.contains(&s.uuid));
    let mut extra = wire_intent(&blanked(before), &after_geometry, spec.clone(), spec).extra;
    // The blanked copy lost the label texts; the label at that point has one.
    for item in extra.iter_mut().filter(|i| i.kind == ExtraKind::Label) {
        if let Some(label) = before
            .labels
            .iter()
            .find(|l| pt_key(&l.position) == pt_key(&item.position))
        {
            item.identity = label.text.clone();
        }
    }
    extra
}

/// Every point of `sheet` that stands for a net: wire ends, junctions, labels,
/// sheet pins and pin tips.
fn points(sheet: &SchematicSheet) -> Vec<Point> {
    let mut points: Vec<Point> = sheet.wires.iter().flat_map(|w| [w.start, w.end]).collect();
    points.extend(sheet.junctions.iter().map(|j| j.position));
    points.extend(sheet.labels.iter().map(|l| l.position));
    points.extend(
        sheet
            .child_sheets
            .iter()
            .flat_map(|c| c.pins.iter().map(|p| p.position)),
    );
    for sym in &sheet.symbols {
        if let Some(lib) = sheet.resolve_lib_symbol(sym) {
            points.extend(pin_tips(&lib, sym).map(|(_, tip)| tip));
        }
    }
    points
}

/// Whether the net at `p` on `sheet` can carry a name from another sheet: it
/// reaches a sheet pin, a hierarchical or global label, or a power port. When
/// the rest of the project cannot be read, such a net's names are unknown.
pub fn net_reaches_other_sheets(sheet: &SchematicSheet, p: Point) -> bool {
    use signex_types::schematic::LabelType;
    let index = SegmentIndex::new(
        &sheet
            .wires
            .iter()
            .map(|w| (w.start, w.end))
            .collect::<Vec<_>>(),
    );
    let mut conn = merged_connectivity(sheet);
    conn.root_of_anchored_in(&p, &index);
    let pins: Vec<Point> = sheet
        .child_sheets
        .iter()
        .flat_map(|c| c.pins.iter().map(|pin| pin.position))
        .collect();
    for pin in &pins {
        conn.root_of_anchored_in(pin, &index);
    }
    let root = conn.root_of(&p);
    let on_pin = pins.iter().any(|pin| conn.root_of(pin) == root);
    let labels = super::collect_net_labels(sheet, &mut conn.parent);
    let carries_label = labels.get(&root).is_some_and(|ls| {
        ls.iter()
            .any(|l| matches!(l.label_type, LabelType::Global | LabelType::Hierarchical))
    });
    let carries_port = super::power_name_carriers(sheet, &mut conn.parent)
        .iter()
        .any(|(r, ..)| *r == root);
    on_pin || carries_label || carries_port
}

fn net_changes(before: &SchematicSheet, after: &SchematicSheet) -> Vec<NameChange> {
    let index = |s: &SchematicSheet| {
        SegmentIndex::new(&s.wires.iter().map(|w| (w.start, w.end)).collect::<Vec<_>>())
    };
    let (bi, ai) = (index(before), index(after));
    let (mut b, mut a) = (merged_connectivity(before), merged_connectivity(after));
    // A sheet pin joins the wire it sits on; anchor before reading any root.
    let sheet_pins: Vec<Point> = before
        .child_sheets
        .iter()
        .flat_map(|c| c.pins.iter().map(|p| p.position))
        .collect();
    for p in &sheet_pins {
        b.root_of_anchored_in(p, &bi);
        a.root_of_anchored_in(p, &ai);
    }
    let pairs: Vec<(Key, Key)> = points(before)
        .iter()
        .map(|p| (b.root_of(p), a.root_of(p)))
        .collect();
    // The rule the project check uses: hierarchical labels are aliases.
    let mut out = Vec::new();
    for (result, inputs) in joined(&pairs) {
        let nets: Vec<NetNames> = inputs
            .into_iter()
            .map(|input| net_names_of_root(before, &mut b.parent, input))
            .collect();
        let after_names = net_names_of_root(after, &mut a.parent, result);
        let refs: Vec<&NetNames> = nets.iter().collect();
        if !needs_allow_merge(&refs, &after_names.all()) {
            continue;
        }
        let mut sets: Vec<Vec<String>> = Vec::new();
        let all = |n: &NetNames| n.all().into_iter().collect::<Vec<_>>();
        for names in nets.iter().filter(|n| n.is_named()).map(all) {
            if !sets.contains(&names) {
                sets.push(names);
            }
        }
        out.push(NameChange {
            before: sets,
            after: names_of_root(after, &mut a.parent, result),
            bus: false,
        });
    }
    out
}

/// The before ids each after id absorbed.
fn joined<K: Copy + Ord>(pairs: &[(K, K)]) -> BTreeMap<K, BTreeSet<K>> {
    let mut joined: BTreeMap<K, BTreeSet<K>> = BTreeMap::new();
    for &(b, a) in pairs {
        joined.entry(a).or_default().insert(b);
    }
    joined
}

/// The name changes of `pairs` (a before id and the after id it became): per
/// result, the named inputs when two carry different name sets, or the one
/// named input when the result carries a name it did not. Bus runs only; a
/// net's names follow the join rule (`net_changes`).
pub(crate) fn name_changes<K: Copy + Eq + Hash + Ord>(
    pairs: &[(K, K)],
    before: &HashMap<K, Vec<String>>,
    after: &HashMap<K, Vec<String>>,
    bus: bool,
) -> Vec<NameChange> {
    let mut out = Vec::new();
    for (result, inputs) in joined(pairs) {
        let mut sets: Vec<Vec<String>> = Vec::new();
        for input in inputs {
            let names = before.get(&input).cloned().unwrap_or_default();
            if !names.is_empty() && !sets.iter().any(|s| same_names(s, &names)) {
                sets.push(names);
            }
        }
        let names = after.get(&result).cloned().unwrap_or_default();
        let changed = match sets.as_slice() {
            [] => false,
            [only] => names.iter().any(|n| !only.contains(n)),
            _ => true,
        };
        if changed {
            out.push(NameChange {
                before: sets,
                after: names,
                bus,
            });
        }
    }
    out
}
