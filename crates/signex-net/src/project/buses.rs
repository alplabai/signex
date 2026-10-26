//! Buses across sheets (#1049): which member nets a bus-typed sheet pin or
//! Port carries on its own sheet, so the stitcher can join them member by
//! member with the bus on the other side.
//!
//! A bus never mints a net (D5.4). What crosses a sheet boundary is each
//! *member's* net: the net of the wire a bus entry feeds, named by its member
//! label. The scope rules are the ones plain names follow — a sheet pin joins
//! the Hierarchical or Global bus of the same [`join_key`] on its child; a
//! Global bus joins every same-keyed Global bus project-wide; a local Net
//! label never leaves its sheet.
//!
//! [`join_key`]: signex_types::schematic::BusName::join_key

use std::collections::{HashMap, HashSet};

use signex_types::schematic::{
    LabelType, Point, SchematicSheet, normalize_port_name, parse_bus_name,
};

use crate::build::pt_key;
use crate::bus::{BusGeometry, analyse_buses};
use crate::uf::{Key, find as uf_find};

/// One bus as seen from the sheet that names it.
pub(super) struct BusSide {
    /// [`BusName::join_key`](signex_types::schematic::BusName::join_key).
    pub key: String,
    /// The members the name carries.
    pub names: HashSet<String>,
    /// Member name → net roots of the entry wires that carry it on this sheet.
    pub roots: HashMap<String, Vec<Key>>,
}

/// Every bus-typed sheet pin and Port on one sheet.
#[derive(Default)]
pub(super) struct SheetBuses {
    /// Per `child_sheets` entry: its bus-typed pins.
    pub pins: Vec<Vec<BusSide>>,
    /// Hierarchical and Global labels that name a bus, with their kind.
    pub ports: Vec<(LabelType, BusSide)>,
}

/// Find the buses `sheet` names through its sheet pins and Ports. `parent` is
/// the fully merged union-find of the sheet, so a member label's root is the
/// root the stitcher uses for that net.
pub(super) fn sheet_buses(sheet: &SchematicSheet, parent: &mut HashMap<Key, Key>) -> SheetBuses {
    if sheet.buses.is_empty() {
        return SheetBuses {
            pins: sheet.child_sheets.iter().map(|_| Vec::new()).collect(),
            ports: Vec::new(),
        };
    }

    // A bus-typed sheet pin names the bus it sits on exactly as a label does,
    // so it joins the analysis as one more label, after the real ones.
    let mut texts: Vec<(Point, &str)> = sheet
        .labels
        .iter()
        .map(|l| (l.position, l.text.as_str()))
        .collect();
    let mut pin_label: HashMap<(usize, usize), usize> = HashMap::new();
    for (ci, cs) in sheet.child_sheets.iter().enumerate() {
        for (pi, sp) in cs.pins.iter().enumerate() {
            let Some(name) = normalize_port_name(&sp.name) else {
                continue;
            };
            if parse_bus_name(name).is_ok() {
                pin_label.insert((ci, pi), texts.len());
                texts.push((sp.position, name));
            }
        }
    }

    let buses: Vec<(Point, Point)> = sheet.buses.iter().map(|b| (b.start, b.end)).collect();
    let entries: Vec<(Point, (f64, f64))> = sheet
        .bus_entries
        .iter()
        .map(|e| (e.position, e.size))
        .collect();
    let wires: Vec<(Point, Point)> = sheet.wires.iter().map(|w| (w.start, w.end)).collect();
    let junctions: Vec<Point> = sheet.junctions.iter().map(|j| j.position).collect();
    let analysis = analyse_buses(&BusGeometry {
        buses: &buses,
        entries: &entries,
        wires: &wires,
        junctions: &junctions,
        labels: &texts,
    });

    let mut side_of = |li: usize| -> Option<BusSide> {
        let name = parse_bus_name(texts[li].1).ok()?;
        let (gi, group) = analysis
            .groups
            .iter()
            .enumerate()
            .find(|(_, g)| g.names.iter().any(|n| n.label == li))?;
        let mut roots: HashMap<String, Vec<Key>> = HashMap::new();
        for att in analysis.entries.iter().filter(|e| e.group == gi) {
            for &ml in &att.member_labels {
                let Some(label) = sheet.labels.get(ml) else {
                    continue;
                };
                if !group.carries(&label.text) {
                    continue;
                }
                let root = uf_find(parent, pt_key(&label.position));
                let slot = roots.entry(label.text.clone()).or_default();
                if !slot.contains(&root) {
                    slot.push(root);
                }
            }
        }
        Some(BusSide {
            key: name.join_key(),
            names: name.members().into_iter().collect(),
            roots,
        })
    };

    let pins = sheet
        .child_sheets
        .iter()
        .enumerate()
        .map(|(ci, cs)| {
            (0..cs.pins.len())
                .filter_map(|pi| side_of(*pin_label.get(&(ci, pi))?))
                .collect()
        })
        .collect();
    let ports = sheet
        .labels
        .iter()
        .enumerate()
        .filter(|(_, l)| l.port_name().is_some())
        .filter_map(|(li, l)| Some((l.label_type, side_of(li)?)))
        .collect();
    SheetBuses { pins, ports }
}

/// The member names both sides carry, sorted, so unions happen in a fixed
/// order. A member only one side names has nothing to join; the width check
/// in ERC reports that.
pub(super) fn shared_members<'a>(a: &'a BusSide, b: &BusSide) -> Vec<&'a str> {
    let mut shared: Vec<&str> = a
        .names
        .iter()
        .filter(|m| b.names.contains(*m))
        .map(String::as_str)
        .collect();
    shared.sort_unstable();
    shared
}
