//! The rows of each board report.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use pcb_geom::Side;
use signex_types::designator::compare_references;
use signex_types::pcb::{ComponentType, Footprint, PcbBoard};

use super::{ReportCell, ReportSide};

type Row = Vec<ReportCell>;

fn text(s: &str) -> ReportCell {
    ReportCell::Text(s.to_string())
}

/// What makes two components one BOM line: value, footprint, description
/// and type.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct BomKey {
    value: String,
    footprint: String,
    description: String,
    /// The type's place in `ComponentType::ALL`.
    kind: usize,
}

/// One line per value, footprint, description and type, listing the
/// designators in natural order; lines ordered by their first designator.
/// Types the BOM leaves out (#2365) are not counted.
pub(super) fn bom(board: &PcbBoard) -> Vec<Row> {
    let mut groups: BTreeMap<BomKey, (ComponentType, Vec<&str>)> = BTreeMap::new();
    for fp in &board.footprints {
        let kind = fp.component.component_type;
        if !kind.in_bom() {
            continue;
        }
        let key = BomKey {
            value: fp.value.clone(),
            footprint: fp.footprint_id.clone(),
            description: fp.component.description.clone(),
            kind: ComponentType::ALL
                .iter()
                .position(|k| *k == kind)
                .unwrap_or(0),
        };
        groups
            .entry(key)
            .or_insert_with(|| (kind, Vec::new()))
            .1
            .push(fp.reference.as_str());
    }
    let mut lines: Vec<(BomKey, ComponentType, Vec<&str>)> = groups
        .into_iter()
        .map(|(key, (kind, mut designators))| {
            designators.sort_by(|a, b| compare_references(a, b));
            (key, kind, designators)
        })
        .collect();
    lines.sort_by(|a, b| first(&a.2, &b.2).then_with(|| a.0.cmp(&b.0)));
    lines
        .into_iter()
        .map(|(key, kind, designators)| {
            vec![
                ReportCell::Text(designators.join(", ")),
                ReportCell::Count(designators.len()),
                ReportCell::Text(key.value),
                ReportCell::Text(key.footprint),
                ReportCell::Text(key.description),
                ReportCell::Type(kind),
            ]
        })
        .collect()
}

fn first(a: &[&str], b: &[&str]) -> Ordering {
    compare_references(
        a.first().copied().unwrap_or(""),
        b.first().copied().unwrap_or(""),
    )
}

/// Every net of the board by number (net 0 is no net): the board's net
/// table, and any net a pad names that the table does not list.
fn nets(board: &PcbBoard) -> Vec<(u32, String)> {
    let mut nets: BTreeMap<u32, String> = board
        .nets
        .iter()
        .filter(|n| n.number != 0)
        .map(|n| (n.number, n.name.clone()))
        .collect();
    for pad in board.footprints.iter().flat_map(|fp| fp.pads.iter()) {
        if let Some(net) = pad.net.as_ref().filter(|n| n.number != 0) {
            nets.entry(net.number).or_insert_with(|| net.name.clone());
        }
    }
    let mut nets: Vec<(u32, String)> = nets.into_iter().collect();
    nets.sort_by(|a, b| compare_references(&a.1, &b.1).then(a.0.cmp(&b.0)));
    nets
}

/// Each net, its pad count and its pads as `designator.pad`.
pub(super) fn netlist(board: &PcbBoard) -> Vec<Row> {
    let mut nodes: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for fp in &board.footprints {
        for pad in &fp.pads {
            if let Some(net) = pad.net.as_ref().filter(|n| n.number != 0) {
                nodes
                    .entry(net.number)
                    .or_default()
                    .push(format!("{}.{}", fp.reference, pad.number));
            }
        }
    }
    nets(board)
        .into_iter()
        .map(|(number, name)| {
            let mut pads = nodes.remove(&number).unwrap_or_default();
            pads.sort_by(|a, b| compare_references(a, b));
            vec![
                ReportCell::Text(name),
                ReportCell::Count(pads.len()),
                ReportCell::Text(pads.join(" ")),
            ]
        })
        .collect()
}

/// Each net's routed length (every track on it), track count and via count.
pub(super) fn net_length(board: &PcbBoard) -> Vec<Row> {
    nets(board)
        .into_iter()
        .map(|(number, name)| {
            let length = pcb_geom::net_routed_length_mm(board, number)
                .map_or(ReportCell::Missing, ReportCell::Millimetres);
            let tracks = board.segments.iter().filter(|s| s.net == number).count();
            let vias = board.vias.iter().filter(|v| v.net == number).count();
            vec![
                ReportCell::Text(name),
                length,
                ReportCell::Count(tracks),
                ReportCell::Count(vias),
            ]
        })
        .collect()
}

/// Each component in designator order, placed as the user reads it: from
/// the board origin, Y up.
pub(super) fn components(board: &PcbBoard) -> Vec<Row> {
    let mut footprints: Vec<&Footprint> = board.footprints.iter().collect();
    footprints.sort_by(|a, b| compare_references(&a.reference, &b.reference));
    let origin = board.options.origin;
    footprints
        .into_iter()
        .map(|fp| {
            let at = origin.to_user(fp.position);
            let side = match Side::of_layer(&fp.layer) {
                Side::Top => ReportSide::Top,
                Side::Bottom => ReportSide::Bottom,
            };
            vec![
                text(&fp.reference),
                text(&fp.footprint_id),
                text(&fp.value),
                ReportCell::Millimetres(at.x),
                ReportCell::Millimetres(at.y),
                ReportCell::Degrees(fp.rotation),
                ReportCell::Side(side),
            ]
        })
        .collect()
}
