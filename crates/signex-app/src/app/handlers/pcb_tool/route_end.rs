//! Where an Interactive Routing leg ends (#2389, maintainer 2026-11-18): a
//! pad of another net refuses the leg, a pad of no net joins the route's
//! net with it, and a pad off the leg's layer is reached with a via at the
//! end, in the same undo step. Rules: `docs/PCB_TOOLS.md`, "Interactive
//! Routing".

use pcb_geom::CopperItem;
use signex_drc::DrcRules;
use signex_engine::pcb::draw::TrackStyle;
use signex_engine::pcb::route::{PadAt, PadEnd, RouteRefusal, RoutedLeg, pad_at, pad_end, via_fits};
use signex_i18n::{Arg, I18n, ids, t};
use signex_types::corner::CornerRule;
use signex_types::pcb::{PcbBoard, Point, Via};
use uuid::Uuid;

use super::BoardToolDefaults;
use super::router::{RouterState, rule_via, walk_rules};
use crate::app::pcb_tools::BoardPlaceTool;

/// What a leg ending at a point does there: the style it routes with (a
/// route of no net yet takes the net of a pad off its layer), the pad of
/// no net it joins, and the via that reaches a pad off its layer.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct LegEnd {
    pub style: TrackStyle,
    pub joins: Option<PadAt>,
    pub via: Option<Via>,
}

/// The end of a leg of `style` at `to`; `Err` with the line saying why a
/// leg may not end there (a pad of another net).
pub(super) fn leg_end(
    i18n: &I18n,
    board: &PcbBoard,
    defaults: &BoardToolDefaults,
    rules: &DrcRules,
    to: Point,
    mut style: TrackStyle,
) -> Result<LegEnd, String> {
    let via = end_via(board, defaults, to, &mut style);
    let via = via.map(|via| rule_via(via, rules));
    let joins = match pad_end(board, to, style.net) {
        PadEnd::Free => None,
        PadEnd::Joins { pad, .. } => Some(pad),
        PadEnd::OtherNet { pad, route_net } => {
            return Err(other_net_line(i18n, board, &pad, route_net));
        }
    };
    Ok(LegEnd { style, joins, via })
}

/// The via at `to` when a leg of `style` ends on a pad off its layer (a
/// surface pad on Top while the leg runs on an inner layer). A leg of no
/// net yet takes the pad's net.
pub(super) fn end_via(
    board: &PcbBoard,
    defaults: &BoardToolDefaults,
    to: Point,
    style: &mut TrackStyle,
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
        ..defaults.via(board, Uuid::now_v7(), to)
    })
}

/// `leg`, and after it the via at its end when there is one, checked
/// against the board with the route so far and the leg on it.
pub(super) fn with_end_via(
    board: &PcbBoard,
    router: &RouterState,
    leg: RoutedLeg,
    via: Option<Via>,
    corner: CornerRule,
) -> Result<Vec<RoutedLeg>, RouteRefusal> {
    let Some(via) = via else {
        return Ok(vec![leg]);
    };
    let mut draft = router.draft.clone();
    draft.add(leg.clone());
    let rules = &router.rules;
    let clearance = |a: &CopperItem, b: &CopperItem| rules.clearance_between(a, b);
    let route = walk_rules(board, &clearance, rules, corner);
    via_fits(board, &draft, &via, &route)?;
    let end = RoutedLeg {
        via: Some(via),
        ..RoutedLeg::default()
    };
    Ok(vec![leg, end])
}

/// `why`, said as Interactive Routing's.
pub(super) fn router_line(i18n: &I18n, why: String) -> String {
    let tool = t!(i18n, BoardPlaceTool::InteractiveRouting.name()).into_string();
    t!(i18n, ids::PCB_ROUTE_REFUSED, "tool" => tool, "why" => why).into_string()
}

/// The line for a leg aimed at `pad` of another net than `route_net`.
fn other_net_line(i18n: &I18n, board: &PcbBoard, pad: &PadAt, route_net: u32) -> String {
    let args = [
        ("pad", Arg::from(pad.label())),
        ("pad_net", Arg::from(net_label(board, pad.net))),
        ("net", Arg::from(net_label(board, route_net))),
    ];
    let why = i18n.message(ids::PCB_ROUTE_END_OTHER_NET, &args);
    router_line(i18n, why.into_string())
}

/// The line for `pad` of no net joining net `net` with the route.
pub(super) fn joined_line(i18n: &I18n, board: &PcbBoard, pad: &PadAt, net: u32) -> String {
    let args = [
        ("pad", Arg::from(pad.label())),
        ("net", Arg::from(net_label(board, net))),
    ];
    i18n.message(ids::PCB_ROUTE_PAD_JOINED, &args).into_string()
}

/// A net's name on `board`, else its number.
fn net_label(board: &PcbBoard, net: u32) -> String {
    board
        .nets
        .iter()
        .find(|n| n.number == net)
        .map_or_else(|| net.to_string(), |n| n.name.clone())
}
