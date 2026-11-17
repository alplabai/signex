//! Find Similar Objects on a board (#2403): the comparable properties of a
//! footprint, pad, track, via or drawn area, and every object of the board
//! that agrees with a seed on the properties set to Same. Matching works on
//! property values, as on a sheet ([`crate::find_similar`]).

use pcb_geom::{ItemRef, PickKind};
use signex_types::designator::alphabetic_prefix;
use signex_types::pcb::{PadShape, PcbBoard};

use crate::{SimilarProperty, SimilarValue};

fn val(property: SimilarProperty, value: impl Into<String>) -> SimilarValue {
    SimilarValue {
        property,
        value: value.into(),
    }
}

/// A length as the dialog shows and compares it: mm to 4 decimals, no
/// trailing zeros.
fn mm(value: f64) -> String {
    let text = format!("{value:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text} mm")
}

fn net_name(board: &PcbBoard, net: u32) -> String {
    board
        .nets
        .iter()
        .find(|def| def.number == net && net != 0)
        .map_or_else(String::new, |def| def.name.clone())
}

fn shape_name(shape: PadShape) -> &'static str {
    match shape {
        PadShape::Circle => "Circle",
        PadShape::Rect => "Rectangle",
        PadShape::Oval => "Oval",
        PadShape::RoundRect => "Rounded rectangle",
    }
}

fn area_name(kind: PickKind) -> &'static str {
    match kind {
        PickKind::Keepouts => "Keepout",
        PickKind::Fills => "Fill",
        _ => "Region",
    }
}

/// Every object of `board` a selection can hold, with its comparable
/// properties, `Kind` first: footprints, their pads, tracks, vias, then
/// drawn areas, each in board order.
fn entries(board: &PcbBoard) -> Vec<(ItemRef, Vec<SimilarValue>)> {
    use SimilarProperty as P;
    let mut out = Vec::new();
    for fp in &board.footprints {
        out.push((
            ItemRef::Footprint(fp.uuid),
            vec![
                val(P::Kind, "Footprint"),
                val(P::Value, fp.value.trim()),
                val(P::Footprint, fp.footprint_id.trim()),
                val(P::DesignatorPrefix, alphabetic_prefix(&fp.reference)),
                val(P::Layer, fp.layer.as_str()),
            ],
        ));
    }
    for fp in &board.footprints {
        for pad in &fp.pads {
            let net = pad.net.as_ref().map_or("", |n| n.name.as_str());
            let mut props = vec![
                val(P::Kind, "Pad"),
                val(P::Shape, shape_name(pad.shape)),
                val(P::Size, format!("{} x {}", mm(pad.size.x), mm(pad.size.y))),
                val(P::NetName, net),
                val(P::Footprint, fp.footprint_id.trim()),
            ];
            if let Some(drill) = &pad.drill {
                props.push(val(P::Drill, mm(drill.diameter)));
            }
            out.push((
                ItemRef::Pad {
                    footprint: fp.uuid,
                    pad: pad.uuid,
                },
                props,
            ));
        }
    }
    for track in &board.segments {
        out.push((
            ItemRef::Track(track.uuid),
            vec![
                val(P::Kind, "Track"),
                val(P::Layer, track.layer.as_str()),
                val(P::Width, mm(track.width)),
                val(P::NetName, net_name(board, track.net)),
            ],
        ));
    }
    for via in &board.vias {
        out.push((
            ItemRef::Via(via.uuid),
            vec![
                val(P::Kind, "Via"),
                val(P::Size, mm(via.diameter)),
                val(P::Drill, mm(via.drill)),
                val(P::NetName, net_name(board, via.net)),
            ],
        ));
    }
    for zone in &board.zones {
        let Some(kind) = PickKind::of_zone(zone) else {
            continue;
        };
        out.push((
            ItemRef::Zone {
                zone: zone.uuid,
                kind,
            },
            vec![
                val(P::Kind, area_name(kind)),
                val(P::Layer, zone.layer.as_str()),
                val(P::NetName, zone.net_name.trim()),
            ],
        ));
    }
    out
}

/// What to call `item` in the dialog: `Footprint U1`, `Pad U1.2`, else its
/// kind. Empty when it is not on the board.
pub fn describe(board: &PcbBoard, item: ItemRef) -> String {
    let kind = properties_of(board, item)
        .first()
        .map(|v| v.value.clone())
        .unwrap_or_default();
    let name = match item {
        ItemRef::Footprint(id) => board
            .footprints
            .iter()
            .find(|fp| fp.uuid == id)
            .map(|fp| fp.reference.clone()),
        ItemRef::Pad { footprint, pad } => board
            .footprints
            .iter()
            .find(|fp| fp.uuid == footprint)
            .and_then(|fp| {
                let pad = fp.pads.iter().find(|p| p.uuid == pad)?;
                Some(format!("{}.{}", fp.reference, pad.number))
            }),
        _ => None,
    };
    match name.as_deref().map(str::trim) {
        Some(name) if !name.is_empty() && !kind.is_empty() => format!("{kind} {name}"),
        _ => kind,
    }
}

/// The comparable properties of `item` on `board`, `Kind` first; empty when
/// it is not on the board. An area's kind is read from the board, so the
/// `kind` an [`ItemRef::Zone`] carries does not have to match it.
pub fn properties_of(board: &PcbBoard, item: ItemRef) -> Vec<SimilarValue> {
    let same = |a: ItemRef, b: ItemRef| match (a, b) {
        (ItemRef::Zone { zone: x, .. }, ItemRef::Zone { zone: y, .. }) => x == y,
        _ => a == b,
    };
    entries(board)
        .into_iter()
        .find_map(|(i, props)| same(i, item).then_some(props))
        .unwrap_or_default()
}

/// Every object of `board` whose value for each of `criteria` equals the
/// criterion's; an object without such a property does not match. Empty
/// criteria match everything.
pub fn find_similar(board: &PcbBoard, criteria: &[SimilarValue]) -> Vec<ItemRef> {
    entries(board)
        .into_iter()
        .filter(|(_, have)| criteria.iter().all(|c| have.contains(c)))
        .map(|(item, _)| item)
        .collect()
}

#[cfg(test)]
#[path = "similar_tests.rs"]
mod tests;
