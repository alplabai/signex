//! Routed length and membership of a net, and a via's layer span: what a
//! track's or via's Properties panel reads (#2355). Lengths are summed in
//! integer nm, so the readout does not drift with the order of the tracks.

use signex_types::pcb::{PcbBoard, Segment, Via, ViaType};
use signex_types::pcb_layer::CopperLayer;

use crate::copper::layer_mask;
use crate::index::{ItemRef, track_shape, via_shape};
use crate::pick_filter::PickKind;
use crate::shape::Shape;
use crate::transform::pad_shape;
use crate::units::{PointNm, nm_to_mm};

/// The centre-line length of a track in nm; `None` for an end the geometry
/// cannot hold.
fn track_length_nm(segment: &Segment) -> Option<i64> {
    let start = PointNm::from_mm(segment.start, "track start").ok()?;
    let end = PointNm::from_mm(segment.end, "track end").ok()?;
    // Both ends are within MAX_ABS_NM, so the square root fits an i64.
    Some((start.distance_sq(end) as f64).sqrt().round() as i64)
}

/// The centre-line length of a track in mm.
pub fn track_length_mm(segment: &Segment) -> Option<f64> {
    track_length_nm(segment).map(nm_to_mm)
}

/// The summed length of `segments` in mm; `None` when one cannot be held.
pub fn tracks_length_mm<'a>(segments: impl IntoIterator<Item = &'a Segment>) -> Option<f64> {
    segments
        .into_iter()
        .map(track_length_nm)
        .sum::<Option<i64>>()
        .map(nm_to_mm)
}

/// The routed length of net `net` in mm: every track and copper arc on it
/// (#2373). `None` for net 0 (no net) or an item the geometry cannot hold.
pub fn net_routed_length_mm(board: &PcbBoard, net: u32) -> Option<f64> {
    if net == 0 {
        return None;
    }
    let tracks = board
        .segments
        .iter()
        .filter(|s| s.net == net)
        .map(track_length_nm);
    let arcs = board
        .free
        .arcs
        .iter()
        .filter(|a| a.net == net && crate::free::arc_is_copper(a))
        .map(|a| crate::units::mm_to_nm(a.length_mm(), "arc length").ok());
    tracks.chain(arcs).sum::<Option<i64>>().map(nm_to_mm)
}

/// The tracks, vias, pads, copper arcs and free pads on net `net`, in board
/// order. Empty for net 0.
pub fn net_items(board: &PcbBoard, net: u32) -> Vec<ItemRef> {
    if net == 0 {
        return Vec::new();
    }
    let tracks = board
        .segments
        .iter()
        .filter(|s| s.net == net)
        .map(|s| ItemRef::Track(s.uuid));
    let vias = board
        .vias
        .iter()
        .filter(|v| v.net == net)
        .map(|v| ItemRef::Via(v.uuid));
    let pads = board.footprints.iter().flat_map(|fp| {
        fp.pads
            .iter()
            .filter(|p| p.net.as_ref().is_some_and(|n| n.number == net))
            .map(|p| ItemRef::Pad {
                footprint: fp.uuid,
                pad: p.uuid,
            })
    });
    tracks
        .chain(vias)
        .chain(pads)
        .chain(crate::free::free_copper_on(board, net))
        .collect()
}

/// Every track, via and pad of `net`, then its selectable drawn areas: what
/// Select Net puts in the selection (#2403). Empty for net 0.
pub fn net_members(board: &PcbBoard, net: u32) -> Vec<ItemRef> {
    let mut items = net_items(board, net);
    if net == 0 {
        return items;
    }
    items.extend(board.zones.iter().filter(|z| z.net == net).filter_map(|z| {
        Some(ItemRef::Zone {
            zone: z.uuid,
            kind: PickKind::of_zone(z)?,
        })
    }));
    items
}

/// The copper of the tracks, vias and pads of `net`: what Highlight Net
/// draws bright (#2403). Items the geometry cannot hold are left out.
pub fn net_copper(board: &PcbBoard, net: u32) -> Vec<Shape> {
    if net == 0 {
        return Vec::new();
    }
    let tracks = board
        .segments
        .iter()
        .filter(|s| s.net == net)
        .filter_map(|s| track_shape(s).ok());
    let vias = board
        .vias
        .iter()
        .filter(|v| v.net == net)
        .filter_map(|v| via_shape(v).ok());
    let pads = board.footprints.iter().flat_map(|fp| {
        fp.pads
            .iter()
            .filter(|p| p.net.as_ref().is_some_and(|n| n.number == net))
            .filter_map(move |p| pad_shape(fp, p).ok())
    });
    tracks.chain(vias).chain(pads).collect()
}

/// The net of a track, via, pad, copper arc, free pad or drawn area; `None`
/// for a footprint body, a string, a dimension, an item not on the board, or
/// one on no net.
pub fn item_net(board: &PcbBoard, item: ItemRef) -> Option<u32> {
    let net = match item {
        ItemRef::Track(id) => board.segments.iter().find(|s| s.uuid == id)?.net,
        ItemRef::Via(id) => board.vias.iter().find(|v| v.uuid == id)?.net,
        ItemRef::Pad { footprint, pad } => {
            board
                .footprints
                .iter()
                .find(|fp| fp.uuid == footprint)?
                .pads
                .iter()
                .find(|p| p.uuid == pad)?
                .net
                .as_ref()?
                .number
        }
        ItemRef::Zone { zone, .. } => board.zones.iter().find(|z| z.uuid == zone)?.net,
        ItemRef::Arc(id) => board.free.arcs.iter().find(|a| a.uuid == id)?.net,
        ItemRef::FreePad(id) => {
            board
                .free
                .pads
                .iter()
                .find(|p| p.uuid == id)?
                .net
                .as_ref()?
                .number
        }
        ItemRef::Footprint(_)
        | ItemRef::Text(_)
        | ItemRef::Dimension(_)
        | ItemRef::Graphic { .. } => {
            return None;
        }
    };
    (net != 0).then_some(net)
}

/// The 1-based positions in `stack` (the board's copper layers, top to
/// bottom) of the first and last layer `via` reaches. A through via spans
/// the whole stack; a blind or micro via the layers it lists. `None` when
/// it lists none of `stack`.
pub fn via_span(stack: &[CopperLayer], via: &Via) -> Option<(usize, usize)> {
    if stack.is_empty() {
        return None;
    }
    if via.via_type == ViaType::Through {
        return Some((1, stack.len()));
    }
    let mask = layer_mask(via.layers.iter().map(String::as_str));
    let reached: Vec<usize> = stack
        .iter()
        .enumerate()
        .filter(|(_, l)| mask & (1 << crate::copper::layer_bit(**l)) != 0)
        .map(|(i, _)| i + 1)
        .collect();
    Some((*reached.first()?, *reached.last()?))
}

#[cfg(test)]
#[path = "net_length_tests.rs"]
mod tests;
