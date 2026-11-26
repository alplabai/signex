//! Differential Pair Routing and Multi-Route asked for in full (#2470):
//! the nets, where they start and the centre line's points, planned with
//! the calls a pair or bus route's clicks make (`side_by_side`), from the
//! board's own defaults and rules. Landed by `route_order`.

use pcb_geom::CopperItem;
use signex_drc::DrcRules;
use signex_engine::pcb::draw::{self, TrackStyle};
use signex_engine::pcb::route::{
    BusMember, RouteDraft, bus_draft, bus_start, diff_pair_of, draft_fits, nearest_net_point,
    route_bus_leg,
};
use signex_i18n::{Translatable, ids};
use signex_types::net_class::NetClass;
use signex_types::pcb::{PcbBoard, Point, Via};
use signex_types::pcb_layer::CopperLayer;
use uuid::Uuid;

use super::route_order::{RouteMade, RouteOrder, order_layer, tool_refusal};
use super::router::{rule_via, walk_rules};
use super::side_by_side::{BusAsk, bus_style, nearest_on_any_layer, net_name};
use crate::app::pcb_tools::BoardPlaceTool;

/// The nets an order routes side by side: the pair of its `net`, or the
/// Multi-Route's nets; two or more.
fn order_nets(
    board: &PcbBoard,
    classes: &[NetClass],
    order: &RouteOrder,
) -> Result<Vec<u32>, Translatable> {
    let tool = order.tool;
    let nets = match (tool, order.net) {
        (BoardPlaceTool::DiffPairRouting, Some(net)) => diff_pair_of(board, classes, net)
            .ok_or_else(|| {
                let why =
                    Translatable::new(ids::PCB_ROUTE_NO_PAIR).arg("net", net_name(board, net));
                tool_refusal(tool, why)
            })?
            .nets()
            .to_vec(),
        (BoardPlaceTool::DiffPairRouting, None) => Vec::new(),
        _ => order.nets.clone(),
    };
    if nets.len() < 2 {
        return Err(tool_refusal(
            tool,
            Translatable::new(ids::PCB_ROUTE_ORDER_NETS),
        ));
    }
    Ok(nets)
}

/// Each net's start: its copper on `layer` nearest the order's start, else
/// its copper on another layer with a via there (`start_bus_route`).
fn member_starts(
    board: &PcbBoard,
    rules: &DrcRules,
    tool: BoardPlaceTool,
    (nets, layer): (&[u32], CopperLayer),
    at: Point,
) -> Result<(Vec<BusMember>, Vec<Via>), Translatable> {
    let mut members = Vec::with_capacity(nets.len());
    let mut start_vias = Vec::new();
    for &net in nets {
        let (anchor, off_layer) = match nearest_net_point(board, net, layer, at) {
            Some(anchor) => (Some(anchor), false),
            None => (nearest_on_any_layer(board, net, at), true),
        };
        let anchor = anchor.ok_or_else(|| {
            let why =
                Translatable::new(ids::PCB_ROUTE_MEMBER_NO_COPPER).arg("net", net_name(board, net));
            tool_refusal(tool, why)
        })?;
        if off_layer {
            let via = Via {
                attributes: Default::default(),
                net,
                ..draw::new_via(board, Uuid::now_v7(), anchor)
            };
            start_vias.push(rule_via(via, rules));
        }
        members.push(BusMember { net, anchor });
    }
    Ok((members, start_vias))
}

/// A pair or Multi-Route: the centre line from the members' start through
/// each point after the order's start, every member's tracks along it,
/// checked against the board.
pub(super) fn plan_bus(
    board: &PcbBoard,
    rules: &DrcRules,
    classes: &[NetClass],
    order: &RouteOrder,
) -> Result<RouteMade, Translatable> {
    let tool = order.tool;
    let refused = |why: String| tool_refusal(tool, why);
    let Some((&start, rest)) = order.points.split_first() else {
        return Err(tool_refusal(
            tool,
            Translatable::new(ids::PCB_ROUTE_ORDER_POINTS),
        ));
    };
    let layer = order_layer(board, order, start)?;
    let nets = order_nets(board, classes, order)?;
    let (members, start_vias) = member_starts(board, rules, tool, (&nets, layer), start)?;
    let Some(centre_start) = bus_start(&members) else {
        return Err(tool_refusal(
            tool,
            Translatable::new(ids::PCB_ROUTE_ORDER_NETS),
        ));
    };
    let ask = BusAsk {
        tool,
        nets: &nets,
        layer,
        width_mm: TrackStyle::starting_at(board, start, layer).width_mm,
    };
    let style = bus_style(board, classes, rules, &ask);
    let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
    let route = walk_rules(board, &clearance, rules, order.corner);
    let mut centre = vec![centre_start];
    for &to in rest {
        let anchor = centre.last().copied().unwrap_or(centre_start);
        let leg = route_bus_leg(board, &members, anchor, to, &style, &route)
            .map_err(|refusal| refused(refusal.to_string()))?;
        centre.extend(leg);
    }
    let tracks = bus_draft(board, &members, &centre, &style, Uuid::now_v7)
        .map_err(|refusal| refused(refusal.to_string()))?;
    let draft = RouteDraft {
        vias: start_vias,
        ..tracks
    };
    draft_fits(board, &draft, &clearance, rules.widest_clearance())
        .map_err(|refusal| refused(refusal.to_string()))?;
    Ok(RouteMade {
        draft,
        joined: Vec::new(),
        layer,
        width_mm: style.width_mm,
        nets,
        revision: 0,
    })
}
