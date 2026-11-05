//! The "Select > Connection" input: every item connected to a seed item.

use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind};

use super::{SheetConnectivity, net_extent_at, point_on_segment, pt_key};

/// The wires, junctions and labels on the net of `seed`, or for a bus seed the
/// buses joined to it end to end plus the labels sitting on them. The seed is
/// always part of the result, and a seed of any other kind comes back alone.
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
            .map(|j| net_items(sheet, &j.position)),
        SelectedKind::Label => sheet
            .labels
            .iter()
            .find(|l| l.uuid == seed.uuid)
            .map(|l| net_items(sheet, &l.position)),
        SelectedKind::Bus => bus_run_items(sheet, seed.uuid),
        _ => None,
    }
    .unwrap_or_default();
    if !out.contains(&seed) {
        out.push(seed);
    }
    out
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

fn bus_run_items(sheet: &SchematicSheet, bus: uuid::Uuid) -> Option<Vec<SelectedItem>> {
    let seed = sheet.buses.iter().find(|b| b.uuid == bus)?;
    let segments: Vec<(Point, Point)> = sheet.buses.iter().map(|b| (b.start, b.end)).collect();
    let mut conn = SheetConnectivity::from_segments(&segments, &[]);
    let root = conn.root_of(&seed.start);
    let run: Vec<&signex_types::schematic::Bus> = sheet
        .buses
        .iter()
        .filter(|b| conn.root_of(&b.start) == root)
        .collect();
    let mut out: Vec<SelectedItem> = run
        .iter()
        .map(|b| SelectedItem::new(b.uuid, SelectedKind::Bus))
        .collect();
    out.extend(
        sheet
            .labels
            .iter()
            .filter(|l| {
                let p = pt_key(&l.position);
                run.iter()
                    .any(|b| point_on_segment(p, pt_key(&b.start), pt_key(&b.end)))
            })
            .map(|l| SelectedItem::new(l.uuid, SelectedKind::Label)),
    );
    Some(out)
}
