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
    /// The distinct net names at the two ends before the run; two are a short.
    pub names: Vec<String>,
    /// The ends with nothing under them before the run.
    pub open: Vec<RunEnd>,
    /// Whether the run's two ends are on one net after it.
    pub joined: bool,
    pub net: Option<IntentNet>,
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
    let names = names_at(before, &mut b.parent, [start, end]);
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
    let ref_keys: HashSet<Key> = ends
        .iter()
        .filter(|e| e.pin.is_some())
        .map(|e| pt_key(&e.at))
        .collect();
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
            let intended = if named.contains(&id) {
                true
            } else if ref_keys.contains(&key) {
                // Another pin on a referenced tip is not the pin named.
                false
            } else if coord_keys.contains(&key) {
                true
            } else {
                before_terminal_root
                    .get(&id)
                    .is_some_and(|r| wanted.contains(r))
            };
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
        if !wanted.contains(r) && on_new(&mut a, p) {
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
        names,
        open,
        joined,
        net: net_of_run(after, start),
    }
}

/// The best name of the net at `p` on `sheet` (labels and power-port values,
/// by the netlist's naming rule), or `None` when the net is unnamed or nothing
/// is at `p`. Used to refuse a label or power port that would rename it.
pub fn net_name_at(sheet: &SchematicSheet, p: Point) -> Option<String> {
    let index = SegmentIndex::new(
        &sheet
            .wires
            .iter()
            .map(|w| (w.start, w.end))
            .collect::<Vec<_>>(),
    );
    let mut conn = merged_connectivity(sheet);
    conn.root_of_anchored_in(&p, &index);
    names_at(sheet, &mut conn.parent, [p, p]).into_iter().next()
}

/// The distinct best names of the nets under `points`, in order.
fn names_at(
    sheet: &SchematicSheet,
    parent: &mut HashMap<Key, Key>,
    points: [Point; 2],
) -> Vec<String> {
    let roots: Vec<Key> = points
        .iter()
        .map(|p| crate::uf::find(parent, pt_key(p)))
        .collect();
    let labels = collect_net_labels(sheet, parent);
    let carriers = power_name_carriers(sheet, parent);
    let mut names: Vec<String> = Vec::new();
    for root in roots {
        let on_net = labels.get(&root).map(Vec::as_slice).unwrap_or(&[]);
        let power: Vec<&str> = carriers
            .iter()
            .filter(|(r, ..)| *r == root)
            .map(|(_, value, ..)| value.as_str())
            .collect();
        if let Some((_, name)) = best_net_name(on_net, &power)
            && !names.contains(&name)
        {
            names.push(name);
        }
    }
    names
}

/// The 1 um keys of every pin tip on `sheet`.
fn pin_tip_keys(sheet: &SchematicSheet) -> HashSet<Key> {
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
