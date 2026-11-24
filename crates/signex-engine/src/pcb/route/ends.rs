//! What a route's ends land on (#2389): the copper a route starts from on
//! any copper layer, the pad a leg ends on, and the via that joins a leg to
//! a pad off its layer. The rules they serve: `docs/PCB_TOOLS.md`,
//! "Interactive Routing".

use pcb_geom::route::{NewCopper, NewKind, RouteWorld};
use pcb_geom::{
    PointNm, collect_copper, layer_bit, pad_layers, pad_pieces, pieces_on, via_layers, via_shape,
};
use signex_types::pcb::{PadNet, PcbBoard, Point, Via};
use signex_types::pcb_free::free_pad_frame;
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
        match (self.reference.is_empty(), self.number.is_empty()) {
            (false, _) => format!("{}.{}", self.reference, self.number),
            // A free pad belongs to no part.
            (true, false) => self.number.clone(),
            (true, true) => "free pad".to_string(),
        }
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
/// surface pad on Top while the leg runs on an inner layer, or a point
/// outside a through pad's smaller copper on the leg's layer). A leg of no
/// net yet takes the lowest net among the pads the via reaches, so the
/// result does not depend on their order; [`pad_end`] then refuses it if
/// they differ. `new_via` makes the via at `to` (the caller sizes it and
/// gives it its id).
pub fn end_via(
    board: &PcbBoard,
    to: Point,
    style: &mut TrackStyle,
    new_via: impl FnOnce(Point) -> Via,
) -> Option<Via> {
    if pads_at(board, to, u32::MAX).is_empty()
        || !pads_at(board, to, 1 << layer_bit(style.layer)).is_empty()
    {
        return None;
    }
    let via = new_via(to);
    if style.net == 0 {
        let reached = pads_at(board, to, via_layers(&via));
        let nets = reached.iter().map(|pad| pad.net).filter(|net| *net != 0);
        style.net = nets.min().unwrap_or(0);
    }
    Some(Via {
        attributes: Default::default(),
        net: style.net,
        ..via
    })
}

/// Every pad under `at` on a copper layer of `layers` (a `layer_bit` mask),
/// by pad id, so no answer depends on board order. A part's pad counts
/// netted or not; a free pad only with a net, as one of no net cannot join
/// a route.
fn pads_at(board: &PcbBoard, at: Point, layers: u32) -> Vec<PadAt> {
    let Ok(p) = PointNm::from_mm(at, "route point") else {
        return Vec::new();
    };
    let frame = free_pad_frame();
    let placed = board
        .footprints
        .iter()
        .flat_map(|footprint| footprint.pads.iter().map(move |pad| (footprint, pad)));
    let free = board
        .free
        .pads
        .iter()
        .filter(|pad| pad.net.as_ref().is_some_and(|net| net.number != 0))
        .map(|pad| (&frame, pad));
    let mut hits: Vec<PadAt> = placed
        .chain(free)
        .filter(|(footprint, pad)| {
            pad_pieces(footprint, pad)
                .is_ok_and(|pieces| pieces_on(pieces, layers).any(|shape| shape.contains(p, 0)))
        })
        .map(|(footprint, pad)| PadAt {
            footprint: footprint.uuid,
            pad: pad.uuid,
            reference: footprint.reference.clone(),
            number: pad.number.clone(),
            net: pad.net.as_ref().map_or(0, |net| net.number),
            layers: pad_layers(footprint, pad),
        })
        .collect();
    hits.sort_by_key(|hit| hit.pad);
    hits
}

/// What a leg of `style` ending at `to` does at the pads there: those on
/// its layer, or with an end `via` those on the via's span. Any pad of
/// another net refuses it. A route of no net yet ends anywhere (it takes
/// the net of what it lands on).
pub fn pad_end(board: &PcbBoard, to: Point, style: &TrackStyle, via: Option<&Via>) -> PadEnd {
    let layers = via.map_or(1 << layer_bit(style.layer), via_layers);
    pad_end_on(board, to, style.net, layers)
}

/// What a layer-change `via` of a route on `net` does at the pads on its
/// span, by the rule of [`pad_end`].
pub fn via_pad_end(board: &PcbBoard, via: &Via, net: u32) -> PadEnd {
    pad_end_on(board, via.position, net, via_layers(via))
}

fn pad_end_on(board: &PcbBoard, at: Point, net: u32, layers: u32) -> PadEnd {
    if net == 0 {
        return PadEnd::Free;
    }
    let pads = pads_at(board, at, layers);
    let route_net = net;
    let foreign = pads
        .iter()
        .filter(|pad| pad.net != 0 && pad.net != net)
        .min_by_key(|pad| (pad.net, pad.pad));
    if let Some(pad) = foreign {
        let pad = pad.clone();
        return PadEnd::OtherNet { pad, route_net };
    }
    match pads.into_iter().find(|pad| pad.net == 0) {
        Some(pad) => PadEnd::Joins { pad, route_net },
        None => PadEnd::Free,
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
