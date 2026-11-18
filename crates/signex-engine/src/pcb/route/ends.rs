//! What a route's ends land on (#2389): the copper a route starts from on
//! any copper layer, and the pad a leg ends on. The rules they serve:
//! `docs/PCB_TOOLS.md`, "Interactive Routing".

use pcb_geom::{PointNm, collect_copper, layer_bit, pad_pieces, pieces_on};
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::CopperLayer;

/// A pad a route's point lands on: its footprint's reference, its number
/// and its net (0 for none).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PadAt {
    pub reference: String,
    pub number: String,
    pub net: u32,
}

impl PadAt {
    /// The pad as a person names it, `U1.3`.
    pub fn label(&self) -> String {
        format!("{}.{}", self.reference, self.number)
    }
}

/// Why a leg of a route on `route_net` may not end where it was aimed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndRefusal {
    /// The pad there is on another net.
    OtherNet { pad: PadAt, route_net: u32 },
    /// The pad there is on no net. Whether a route may give a no-net pad
    /// its net is not decided yet; until it is, it is refused.
    NoNet { pad: PadAt, route_net: u32 },
}

/// The topmost copper layer holding netted copper under `at`, and its net:
/// where a route started there begins when the active layer has none (a
/// surface pad on another layer), so its first leg starts with a via.
pub fn copper_under(board: &PcbBoard, at: Point) -> Option<(CopperLayer, u32)> {
    let p = PointNm::from_mm(at, "route start").ok()?;
    let copper = collect_copper(board);
    CopperLayer::all().find_map(|layer| {
        let bit = 1 << layer_bit(layer);
        copper
            .iter()
            .find(|item| item.net != 0 && item.layers & bit != 0 && item.shape.contains(p, 0))
            .map(|item| (layer, item.net))
    })
}

/// The pad under `at` on a copper layer of `layers` (a `layer_bit` mask),
/// netted or not.
pub fn pad_at(board: &PcbBoard, at: Point, layers: u32) -> Option<PadAt> {
    let p = PointNm::from_mm(at, "route point").ok()?;
    board.footprints.iter().find_map(|footprint| {
        footprint.pads.iter().find_map(|pad| {
            let pieces = pad_pieces(footprint, pad).ok()?;
            let hit = pieces_on(pieces, layers).any(|shape| shape.contains(p, 0));
            hit.then(|| PadAt {
                reference: footprint.reference.clone(),
                number: pad.number.clone(),
                net: pad.net.as_ref().map_or(0, |net| net.number),
            })
        })
    })
}

/// Why a leg of a route on `net` may not end at `to` on `layer`: the pad
/// there is on another net, or on none. A route of no net yet ends
/// anywhere (it takes the net of what it lands on).
pub fn end_refusal(
    board: &PcbBoard,
    to: Point,
    net: u32,
    layer: CopperLayer,
) -> Option<EndRefusal> {
    if net == 0 {
        return None;
    }
    let pad = pad_at(board, to, 1 << layer_bit(layer))?;
    let route_net = net;
    match pad.net {
        0 => Some(EndRefusal::NoNet { pad, route_net }),
        other if other != net => Some(EndRefusal::OtherNet { pad, route_net }),
        _ => None,
    }
}
