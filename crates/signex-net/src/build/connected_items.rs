//! The "Select > Connection" input: every item connected to a seed item.

use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind};

use super::{net_extent_at, point_on_segment, pt_key};
use crate::{BusAnalysis, BusGeometry, analyse_buses};

/// The wires, junctions and labels on the net of `seed`. For a bus seed, a
/// label on a bus or a junction on a bus alone, it is the bus run (the
/// segments [`analyse_buses`] groups, T-joints and junctions included) plus
/// the labels that touch it. The seed is always part of the result, and a seed
/// of any other kind comes back alone.
///
/// A net seed reads the same [`merged_connectivity`](super::merged_connectivity)
/// as [`build_netlist`](super::build_netlist), so it follows T-junctions and
/// same-name label joins. Buses are not nets, so a net seed never pulls one in
/// and a bus seed never reaches a wire (ADR-0002 D5.4).
pub fn connected_net_items(sheet: &SchematicSheet, seed: SelectedItem) -> Vec<SelectedItem> {
    let mut out = match seed.kind {
        SelectedKind::Wire => sheet
            .wires
            .iter()
            .find(|w| w.uuid == seed.uuid)
            .map(|w| net_items(sheet, &w.start)),
        SelectedKind::Junction => sheet
            .junctions
            .iter()
            .find(|j| j.uuid == seed.uuid)
            .map(|j| junction_items(sheet, &j.position)),
        SelectedKind::Label => sheet
            .labels
            .iter()
            .position(|l| l.uuid == seed.uuid)
            .map(|li| label_items(sheet, li)),
        SelectedKind::Bus => sheet
            .buses
            .iter()
            .position(|b| b.uuid == seed.uuid)
            .map(|bi| {
                let analysis = analyse_sheet_buses(sheet);
                bus_run_items(sheet, &analysis, group_of_segment(&analysis, bi))
            }),
        _ => None,
    }
    .unwrap_or_default();
    if !out.contains(&seed) {
        out.push(seed);
    }
    out
}

fn analyse_sheet_buses(sheet: &SchematicSheet) -> BusAnalysis {
    let buses: Vec<(Point, Point)> = sheet.buses.iter().map(|b| (b.start, b.end)).collect();
    let entries: Vec<(Point, (f64, f64))> = sheet
        .bus_entries
        .iter()
        .map(|e| (e.position, e.size))
        .collect();
    let wires: Vec<(Point, Point)> = sheet.wires.iter().map(|w| (w.start, w.end)).collect();
    let junctions: Vec<Point> = sheet.junctions.iter().map(|j| j.position).collect();
    let labels: Vec<(Point, &str)> = sheet
        .labels
        .iter()
        .map(|l| (l.position, l.text.as_str()))
        .collect();
    analyse_buses(&BusGeometry {
        buses: &buses,
        entries: &entries,
        wires: &wires,
        junctions: &junctions,
        labels: &labels,
    })
}

fn group_of_segment(analysis: &BusAnalysis, segment: usize) -> usize {
    analysis
        .groups
        .iter()
        .position(|g| g.segments.contains(&segment))
        .unwrap_or_default()
}

/// A label the bus analysis attaches to a run selects that run; any other
/// label selects its net.
fn label_items(sheet: &SchematicSheet, label: usize) -> Vec<SelectedItem> {
    let analysis = analyse_sheet_buses(sheet);
    let group = analysis.groups.iter().position(|g| {
        g.names.iter().any(|n| n.label == label) || g.rejected.iter().any(|(l, _)| *l == label)
    });
    match group {
        Some(g) => bus_run_items(sheet, &analysis, g),
        None => net_items(sheet, &sheet.labels[label].position),
    }
}

/// A junction on a net selects the net; a junction on a bus and no wire
/// selects that bus run.
fn junction_items(sheet: &SchematicSheet, at: &Point) -> Vec<SelectedItem> {
    let net = net_items(sheet, at);
    let has_wire = net.iter().any(|i| i.kind == SelectedKind::Wire);
    let p = pt_key(at);
    let on_bus = sheet
        .buses
        .iter()
        .position(|b| point_on_segment(p, pt_key(&b.start), pt_key(&b.end)));
    match (has_wire, on_bus) {
        (false, Some(bi)) => {
            let analysis = analyse_sheet_buses(sheet);
            bus_run_items(sheet, &analysis, group_of_segment(&analysis, bi))
        }
        _ => net,
    }
}

fn net_items(sheet: &SchematicSheet, anchor: &Point) -> Vec<SelectedItem> {
    let extent = net_extent_at(sheet, anchor);
    let tag =
        |ids: Vec<uuid::Uuid>, kind| ids.into_iter().map(move |id| SelectedItem::new(id, kind));
    tag(extent.wires, SelectedKind::Wire)
        .chain(tag(extent.junctions, SelectedKind::Junction))
        .chain(tag(extent.labels, SelectedKind::Label))
        .collect()
}

fn bus_run_items(
    sheet: &SchematicSheet,
    analysis: &BusAnalysis,
    group: usize,
) -> Vec<SelectedItem> {
    let Some(group) = analysis.groups.get(group) else {
        return Vec::new();
    };
    let buses = group
        .segments
        .iter()
        .map(|&s| SelectedItem::new(sheet.buses[s].uuid, SelectedKind::Bus));
    let labels = group
        .names
        .iter()
        .map(|n| n.label)
        .chain(group.rejected.iter().map(|(l, _)| *l))
        .map(|l| SelectedItem::new(sheet.labels[l].uuid, SelectedKind::Label));
    buses.chain(labels).collect()
}
