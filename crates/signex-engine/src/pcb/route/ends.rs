//! What a route's ends land on (#2389): the copper a route starts from on
//! any copper layer, the pad a leg ends on, and the via that joins a leg to
//! a pad off its layer. The rules they serve: `docs/PCB_TOOLS.md`,
//! "Interactive Routing".

use pcb_geom::route::{NewCopper, NewKind, RouteWorld};
use pcb_geom::{
    PointNm, collect_copper, layer_bit, pad_layers, pad_pieces, pieces_on, via_layers, via_shape,
};
use signex_types::pcb::{PadNet, PcbBoard, Point, Via};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use super::{RouteDraft, RouteRefusal, RouteRules};
use crate::pcb::draw::TrackStyle;
use crate::pcb::{BoardCommand, FootprintPad};

/// A pad a route's point lands on: its footprint and pad ids, the
/// footprint's reference, the pad's number, its net (0 for none) and the
/// copper layers it reaches (a `layer_bit` mask).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PadAt {
    pub footprint: Uuid,
    pub pad: Uuid,
    pub reference: String,
    pub number: String,
    pub net: u32,
    pub layers: u32,
}

impl PadAt {
    /// The pad as a person names it, `U1.3`.
    pub fn label(&self) -> String {
        format!("{}.{}", self.reference, self.number)
    }

    /// Whether the pad reaches copper layer `layer`.
    pub fn reaches(&self, layer: CopperLayer) -> bool {
        self.layers & (1 << layer_bit(layer)) != 0
    }
}

/// What a leg of a route on `route_net` does at the pad it ends on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PadEnd {
    /// No pad there, or one of the route's net (or a route of no net yet).
    Free,
    /// A pad of no net: it joins the route's net with the route, as a
    /// connection made on the board (maintainer 2026-11-18).
    Joins { pad: PadAt, route_net: u32 },
    /// A pad of another net: the leg is refused.
    OtherNet { pad: PadAt, route_net: u32 },
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

/// `style`, started at `at`: with no net on its layer there and none set by
/// the caller (`net_set`, the tool's form), it takes the layer and net of the
/// netted copper under `at` on another copper layer (a surface pad off the
/// active layer), so the first leg adds a via there and runs on the active
/// layer (maintainer 2026-11-18). Over bare board it stays as it is, of no
/// net.
pub fn start_on_copper(
    board: &PcbBoard,
    at: Point,
    style: TrackStyle,
    net_set: bool,
) -> TrackStyle {
    if style.net != 0 || net_set {
        return style;
    }
    match copper_under(board, at) {
        Some((layer, net)) => TrackStyle {
            layer,
            net,
            ..style
        },
        None => style,
    }
}

/// The via at `to` when a leg of `style` ends on a pad off its layer (a
/// surface pad on Top while the leg runs on an inner layer). A leg of no
/// net yet takes the pad's net. `new_via` makes the via at `to` (the caller
/// sizes it and gives it its id).
pub fn end_via(
    board: &PcbBoard,
    to: Point,
    style: &mut TrackStyle,
    new_via: impl FnOnce(Point) -> Via,
) -> Option<Via> {
    let pad = pad_at(board, to, u32::MAX)?;
    if pad.reaches(style.layer) {
        return None;
    }
    if style.net == 0 {
        style.net = pad.net;
    }
    Some(Via {
        attributes: Default::default(),
        net: style.net,
        ..new_via(to)
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
                footprint: footprint.uuid,
                pad: pad.uuid,
                reference: footprint.reference.clone(),
                number: pad.number.clone(),
                net: pad.net.as_ref().map_or(0, |net| net.number),
                layers: pad_layers(footprint, pad),
            })
        })
    })
}

/// What a leg of a route on `net` ending at `to` does at the pad there, on
/// any copper layer. A route of no net yet ends anywhere (it takes the net
/// of what it lands on).
pub fn pad_end(board: &PcbBoard, to: Point, net: u32) -> PadEnd {
    if net == 0 {
        return PadEnd::Free;
    }
    let Some(pad) = pad_at(board, to, u32::MAX) else {
        return PadEnd::Free;
    };
    let route_net = net;
    match pad.net {
        0 => PadEnd::Joins { pad, route_net },
        other if other != net => PadEnd::OtherNet { pad, route_net },
        _ => PadEnd::Free,
    }
}

/// Whether `via` keeps the rules' clearance on `board` with `draft` on it,
/// and stays out of keepouts that forbid vias.
pub fn via_fits(
    board: &PcbBoard,
    draft: &RouteDraft,
    via: &Via,
    rules: &RouteRules,
) -> Result<(), RouteRefusal> {
    let working = draft.applied_to(board);
    let world = RouteWorld::with_reach(&working, rules.clearance, rules.reach);
    let shape = via_shape(via).map_err(|_| RouteRefusal::Unrepresentable)?;
    let new = NewCopper {
        kind: NewKind::Via,
        net: via.net,
        layers: via_layers(via),
        shape,
    };
    if world.is_clear(&new) {
        Ok(())
    } else {
        Err(RouteRefusal::ViaBlocked)
    }
}

/// The command that gives each `(footprint, pad, net)` of `joined` its net
/// on `board`, as a connection made on the board: the net is locked, so
/// Update PCB keeps it and lists it (`PadNetIssue::PadNetLocked`). `None`
/// when there is nothing to join, a pad is gone or a net is not in the
/// table.
pub fn join_pads(board: &PcbBoard, joined: &[(Uuid, Uuid, u32)]) -> Option<BoardCommand> {
    if joined.is_empty() {
        return None;
    }
    let mut pads = Vec::with_capacity(joined.len());
    for &(footprint, pad, net) in joined {
        let on = board.footprints.iter().find(|f| f.uuid == footprint)?;
        let mut pad = on.pads.iter().find(|p| p.uuid == pad)?.clone();
        let name = board.nets.iter().find(|n| n.number == net)?.name.clone();
        pad.net = Some(PadNet { number: net, name });
        pad.net_locked = true;
        pads.push(FootprintPad { footprint, pad });
    }
    Some(BoardCommand::ReplacePads { pads })
}
