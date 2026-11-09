//! The intent check of one headless wire run (#2199): what a wire from
//! `start` to `end` pulled into its net besides the two points it was drawn
//! between. Rules and refusals: `docs/CLI.md`, "Wire intent".
//!
//! Pure and built on the netlist derivation ([`merged_connectivity`],
//! [`collect_terminals_at`], [`collect_net_labels`], [`power_name_carriers`]):
//! there is no second connectivity model. The caller applies the run to a
//! copy of the sheet and passes both versions.

use std::collections::{HashMap, HashSet};

use signex_types::schematic::{InstancePath, Point, SchematicSheet, pin_tips};
use uuid::Uuid;

use super::{
    SegmentIndex, best_net_name, build_netlist, collect_net_labels, collect_terminals_at,
    merged_connectivity, point_on_segment, power_name_carriers, pt_key,
};
use crate::uf::Key;

/// One end of the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunEnd {
    Start,
    End,
}

/// What kind of item a run pulled in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExtraKind {
    Pin,
    Label,
    Junction,
    Wire,
    /// A child-sheet pin; the identity is `<pin> of <sheet name>`.
    SheetPin,
}

/// An item that was on another net before and is on the run's net now.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtraItem {
    pub kind: ExtraKind,
    /// `designator.pin`, the label text, or the wire's uuid.
    pub identity: String,
    pub position: Point,
}

/// The net the run ended up on, when a pin lands on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentNet {
    pub name: String,
    /// `designator.pin`, sorted as the netlist sorts them.
    pub pins: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WireIntent {
    /// Items pulled in that the two ends did not name.
    pub extra: Vec<ExtraItem>,
    /// Every name of the net at each end before the run (empty: unnamed).
    pub end_names: [Vec<String>; 2],
    /// The ends with nothing under them before the run.
    pub open: Vec<RunEnd>,
    /// Whether the run's two ends are on one net after it.
    pub joined: bool,
    pub net: Option<IntentNet>,
}

impl WireIntent {
    /// Whether the run joins two named nets that carry different names.
    pub fn short(&self) -> bool {
        names_short(&self.end_names)
    }

    /// Every name of the two ends, distinct, in order.
    pub fn merged_names(&self) -> Vec<String> {
        let mut all: Vec<String> = Vec::new();
        for name in self.end_names.iter().flatten() {
            if !all.contains(name) {
                all.push(name.clone());
            }
        }
        all
    }
}

/// What the caller named at one end of the run: a point, and the pin when it
/// was given as a pin reference.
#[derive(Debug, Clone, PartialEq)]
pub struct EndSpec {
    pub at: Point,
    /// The placed symbol and pin id (`Terminal::symbol`, `Terminal::pin`) a
    /// pin reference named. Only that pin is intended at the point.
    pub pin: Option<(Uuid, String)>,
}

impl EndSpec {
    /// An end given as a coordinate.
    pub fn at(at: Point) -> Self {
        Self { at, pin: None }
    }
}

/// Judge the run from `start` to `end` that turned `before` into `after`.
///
/// What an end intends is what is connected there before the run by the
/// netlist's own gate (wire ends, junctions, labels, terminal pins, sheet
/// pins) and what the caller named: a pin reference names only that pin, a
/// coordinate names the pins at the point. A wire passing under an end is a
/// T the caller asked for only for a coordinate end with no pin tip there.
pub fn wire_intent(
    before: &SchematicSheet,
    after: &SchematicSheet,
    start: EndSpec,
    end: EndSpec,
) -> WireIntent {
    let (ends, start, end) = ([start.clone(), end.clone()], start.at, end.at);
    let root = InstancePath::root();
    let segments = |s: &SchematicSheet| -> Vec<(Point, Point)> {
        s.wires.iter().map(|w| (w.start, w.end)).collect()
    };
    let before_index = SegmentIndex::new(&segments(before));

    // Anchor every point before reading any root: an anchor can re-root a class.
    let mut b = merged_connectivity(before);
    // A child-sheet pin joins the wire it sits on, end or interior, with no
    // junction: the project netlist anchors it as `SheetPinContacts` says.
    let sheet_pins: Vec<(Point, String)> = before
        .child_sheets
        .iter()
        .flat_map(|cs| {
            cs.pins
                .iter()
                .map(move |sp| (sp.position, format!("{} of {}", sp.name, cs.name)))
        })
        .collect();
    let tip_keys = pin_tip_keys(before);
    let tapped_ends = ends
        .iter()
        .filter(|e| e.pin.is_none() && !tip_keys.contains(&pt_key(&e.at)))
        .map(|e| &e.at);
    for p in tapped_ends.chain(sheet_pins.iter().map(|(p, _)| p)) {
        b.root_of_anchored_in(p, &before_index);
    }
    let wanted: HashSet<Key> = ends.iter().map(|e| b.root_of(&e.at)).collect();
    let end_names = [
        names_at_point(before, &mut b.parent, &start),
        names_at_point(before, &mut b.parent, &end),
    ];
    let before_terminal_root: HashMap<(Uuid, String), Key> =
        collect_terminals_at(before, &root, &mut b.parent)
            .into_iter()
            .flat_map(|(r, found)| found.into_iter().map(move |(t, _)| ((t.symbol, t.pin), r)))
            .collect();
    let before_label_roots: Vec<Key> = before
        .labels
        .iter()
        .map(|l| b.root_of(&l.position))
        .collect();
    let before_junction_roots: Vec<Key> = before
        .junctions
        .iter()
        .map(|j| b.root_of(&j.position))
        .collect();
    let before_wire_roots: Vec<Key> = before.wires.iter().map(|w| b.root_of(&w.start)).collect();
    let before_sheet_pin_roots: Vec<Key> = sheet_pins.iter().map(|(p, _)| b.root_of(p)).collect();

    let after_index = SegmentIndex::new(&segments(after));
    let mut a = merged_connectivity(after);
    for p in std::iter::once(&start)
        .chain(std::iter::once(&end))
        .chain(before.wires.iter().map(|w| &w.start))
        .chain(sheet_pins.iter().map(|(p, _)| p))
    {
        a.root_of_anchored_in(p, &after_index);
    }
    let new_root = a.root_of(&start);
    let on_new = |a: &mut crate::SheetConnectivity, p: &Point| a.root_of(p) == new_root;

    let mut extra = Vec::new();
    let named: HashSet<(Uuid, String)> = ends.iter().filter_map(|e| e.pin.clone()).collect();
    let coord_keys: HashSet<Key> = ends
        .iter()
        .filter(|e| e.pin.is_none())
        .map(|e| pt_key(&e.at))
        .collect();
    for (found_root, found) in collect_terminals_at(after, &root, &mut a.parent) {
        if found_root != new_root {
            continue;
        }
        for (t, tip) in found {
            let id = (t.symbol, t.pin.clone());
            let key = pt_key(&tip);
            // A pin already on a wanted net is no surprise, wherever it sits.
            // Otherwise only the pin a reference named, or the pins at a
            // coordinate end, are intended: another pin on a referenced tip is
            // not the pin named.
            let on_wanted = before_terminal_root
                .get(&id)
                .is_some_and(|r| wanted.contains(r));
            let intended = named.contains(&id) || on_wanted || coord_keys.contains(&key);
            if !intended {
                extra.push(ExtraItem {
                    kind: ExtraKind::Pin,
                    identity: format!("{}.{}", t.reference, t.pin),
                    position: tip,
                });
            }
        }
    }
    for (l, r) in before.labels.iter().zip(&before_label_roots) {
        if !wanted.contains(r) && on_new(&mut a, &l.position) {
            extra.push(ExtraItem {
                kind: ExtraKind::Label,
                identity: l.text.clone(),
                position: l.position,
            });
        }
    }
    for (j, r) in before.junctions.iter().zip(&before_junction_roots) {
        if !wanted.contains(r) && on_new(&mut a, &j.position) {
            extra.push(ExtraItem {
                kind: ExtraKind::Junction,
                identity: j.uuid.to_string(),
                position: j.position,
            });
        }
    }
    for (w, r) in before.wires.iter().zip(&before_wire_roots) {
        if !wanted.contains(r) && on_new(&mut a, &w.start) {
            extra.push(ExtraItem {
                kind: ExtraKind::Wire,
                identity: w.uuid.to_string(),
                position: w.start,
            });
        }
    }
    for ((p, name), r) in sheet_pins.iter().zip(&before_sheet_pin_roots) {
        // A sheet pin on a referenced pin's tip is on the pin's net only if
        // something real (a wire end, junction or label) connects them there.
        let butted = ends
            .iter()
            .any(|e| e.pin.is_some() && pt_key(&e.at) == pt_key(p))
            && !has_connection_anchor(before, *p);
        if (!wanted.contains(r) || butted) && on_new(&mut a, p) {
            extra.push(ExtraItem {
                kind: ExtraKind::SheetPin,
                identity: name.clone(),
                position: *p,
            });
        }
    }
    extra.sort_by(|x, y| (x.kind, &x.identity).cmp(&(y.kind, &y.identity)));
    extra.dedup_by(|x, y| x.kind == y.kind && x.identity == y.identity);

    let open = [(RunEnd::Start, start), (RunEnd::End, end)]
        .into_iter()
        .filter(|(_, p)| !has_item_at(before, &before_index, &tip_keys, p))
        .map(|(end, _)| end)
        .collect();
    let joined = on_new(&mut a, &end);
    WireIntent {
        extra,
        end_names,
        open,
        joined,
        net: net_of_run(after, start),
    }
}

/// Every name of the net the key of `p` is in, best first.
fn names_at_point(
    sheet: &SchematicSheet,
    parent: &mut HashMap<Key, Key>,
    p: &Point,
) -> Vec<String> {
    let root = crate::uf::find(parent, pt_key(p));
    names_of_root(sheet, parent, root)
}

/// Every name of the net `root` (label texts and power-port values), the best
/// name by the netlist's rule first.
pub(super) fn names_of_root(
    sheet: &SchematicSheet,
    parent: &mut HashMap<Key, Key>,
    root: Key,
) -> Vec<String> {
    let labels = collect_net_labels(sheet, parent);
    let carriers = power_name_carriers(sheet, parent);
    let on_net: Vec<&signex_types::schematic::Label> =
        labels.get(&root).cloned().unwrap_or_default();
    let power: Vec<&str> = carriers
        .iter()
        .filter(|(r, ..)| *r == root)
        .map(|(_, value, ..)| value.as_str())
        .collect();
    let mut names: Vec<String> = best_net_name(&on_net, &power)
        .map(|(_, name)| name)
        .into_iter()
        .collect();
    let all = on_net
        .iter()
        .map(|l| l.text.clone())
        .chain(power.iter().map(|v| (*v).to_string()));
    for name in all {
        if !name.is_empty() && !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// Whether `a` and `b` carry the same set of names.
pub(super) fn same_names(a: &[String], b: &[String]) -> bool {
    a.iter().all(|n| b.contains(n)) && b.iter().all(|n| a.contains(n))
}

/// Whether joining nets with these name sets shorts named nets: two that are
/// both named and carry different sets of names.
fn names_short(sets: &[Vec<String>]) -> bool {
    let named: Vec<&Vec<String>> = sets.iter().filter(|s| !s.is_empty()).collect();
    named
        .iter()
        .any(|s| named.iter().any(|t| !same_names(s, t)))
}

/// Whether a wire end, a junction or a label sits at `p`: what makes a pin tip
/// there a connection (a no-connect flag does not).
pub fn has_connection_anchor(sheet: &SchematicSheet, p: Point) -> bool {
    let key = pt_key(&p);
    sheet
        .wires
        .iter()
        .any(|w| pt_key(&w.start) == key || pt_key(&w.end) == key)
        || sheet.junctions.iter().any(|j| pt_key(&j.position) == key)
        || sheet.labels.iter().any(|l| pt_key(&l.position) == key)
}

/// The 1 um keys of every pin tip on `sheet`.
pub(super) fn pin_tip_keys(sheet: &SchematicSheet) -> HashSet<Key> {
    sheet
        .symbols
        .iter()
        .filter_map(|sym| Some((sym, sheet.resolve_lib_symbol(sym)?)))
        .flat_map(|(sym, lib)| {
            pin_tips(&lib, sym)
                .map(|(_, tip)| pt_key(&tip))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Whether a pin tip, a wire (end or interior), a junction, a label, a sheet
/// pin or either end of a bus entry sits at `p`.
fn has_item_at(
    sheet: &SchematicSheet,
    index: &SegmentIndex,
    tips: &HashSet<Key>,
    p: &Point,
) -> bool {
    let key = pt_key(p);
    index.contains(key)
        || tips.contains(&key)
        || sheet.junctions.iter().any(|j| pt_key(&j.position) == key)
        || sheet.labels.iter().any(|l| pt_key(&l.position) == key)
        || sheet
            .child_sheets
            .iter()
            .any(|cs| cs.pins.iter().any(|sp| pt_key(&sp.position) == key))
        || crate::bus_ends::entry_end_at(sheet, key)
}

/// The netlist net the wire at `start` is on, if a pin lands on it.
fn net_of_run(after: &SchematicSheet, start: Point) -> Option<IntentNet> {
    let key = pt_key(&start);
    let wire = after
        .wires
        .iter()
        .find(|w| point_on_segment(key, pt_key(&w.start), pt_key(&w.end)))?;
    let netlist = build_netlist(after);
    let net = netlist.nets.iter().find(|n| n.wires.contains(&wire.uuid))?;
    Some(IntentNet {
        name: net.name.clone(),
        pins: net
            .terminals
            .iter()
            .map(|t| format!("{}.{}", t.reference, t.pin))
            .collect(),
    })
}
