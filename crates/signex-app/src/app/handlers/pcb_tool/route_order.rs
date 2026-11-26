//! A route or a via asked for in full (#2470): what the routing tools and
//! the Via tool run from their operands, on the board they name, instead of
//! clicks. Each is built by the same helpers and engine calls as the tool's
//! clicks (`route`, `quick`, `router`, `route_end`, `side_by_side`), from
//! the board's own defaults and rules rather than the person's tool form,
//! corner rule and route mode (docs/DECISIONS.md, 2026-11-24), and lands as
//! one undo step. The pair and Multi-Route plans are `bus_order`. See
//! `docs/CLI.md`, "Routing a board".

use std::path::Path;

use signex_drc::DrcRules;
use signex_engine::pcb::draw::{self, TrackStyle};
use signex_engine::pcb::primitive_edit::{ViaEdit, edit_vias};
use signex_engine::pcb::route::{
    LegRequest, PadAt, PadEnd, RouteDraft, RouteMode, RoutedLeg, SEARCH_BUDGET, copper_under,
    end_via, join_pads, pad_end, start_on_copper, via_pad_end,
};
use signex_engine::pcb::{BoardCommand, BoardItem};
use signex_i18n::{Arg, Translatable, ids, t};
use signex_types::corner::CornerRule;
use signex_types::net_class::NetClass;
use signex_types::pcb::{PcbBoard, Point, Via};
use signex_types::pcb_layer::CopperLayer;
use signex_types::pcb_stack::board_copper_layers;
use uuid::Uuid;

use super::route_end::with_end_via;
use super::router::{RouterState, refusal_line, route_on, rule_via, rule_width};
use super::side_by_side::net_name;
use crate::app::Signex;
use crate::app::pcb_tools::BoardPlaceTool;
use crate::app::state::pcb_edit::CheckedStep;

/// A route of one net, a pair or several nets, named in full.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RouteOrder {
    pub tool: BoardPlaceTool,
    /// Board points in model mm: the start, the corners, the end.
    pub points: Vec<Point>,
    pub layer: Option<CopperLayer>,
    pub width_mm: Option<f64>,
    /// The route's net, or either net of a pair.
    pub net: Option<u32>,
    /// A Multi-Route's nets.
    pub nets: Vec<u32>,
    pub corner: CornerRule,
    pub mode: RouteMode,
    pub dry_run: bool,
}

/// What an order made, or for a dry run would make.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RouteMade {
    pub draft: RouteDraft,
    /// Pads of no net the route ended on, each with the net it joins.
    pub joined: Vec<(PadAt, u32)>,
    pub layer: CopperLayer,
    pub width_mm: f64,
    pub nets: Vec<u32>,
    /// The board revision after the step; before it, for a dry run.
    pub revision: u64,
}

/// A via named in full.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ViaOrder {
    /// Board point in model mm.
    pub at: Point,
    pub net: Option<u32>,
    pub span: Option<(CopperLayer, CopperLayer)>,
    pub diameter_mm: Option<f64>,
    pub drill_mm: Option<f64>,
    pub dry_run: bool,
}

/// Why `tool` did nothing: `why` after the tool's name, as the click's
/// Messages line says it.
pub(crate) fn tool_refusal(tool: BoardPlaceTool, why: impl Into<Arg>) -> Translatable {
    Translatable::new(ids::PCB_ROUTE_REFUSED)
        .arg("tool", Translatable::new(tool.name()))
        .arg("why", why)
}

impl Signex {
    /// Route `order` on the board at `path` as one undo step (none for a dry
    /// run), checked against the revision it was planned on.
    pub(crate) fn run_route_order(
        &mut self,
        path: &Path,
        order: &RouteOrder,
    ) -> Result<RouteMade, Translatable> {
        let (made, commands) = {
            let board = self
                .document_state
                .pcb_board(path)
                .ok_or_else(|| gone(order.tool))?;
            let (rules, classes) = self
                .board_design_rules(path, board)
                .map_err(|why| Translatable::new(ids::PCB_ROUTE_NO_RULES).arg("why", why))?;
            let made = match order.tool {
                BoardPlaceTool::DiffPairRouting | BoardPlaceTool::MultiRoute => {
                    super::bus_order::plan_bus(board, &rules, &classes, order)?
                }
                _ => plan_single(board, &rules, &classes, order)?,
            };
            let mut commands = made.draft.commands();
            let joins: Vec<(Uuid, Uuid, u32)> = (made.joined.iter())
                .map(|(pad, net)| (pad.footprint, pad.pad, *net))
                .collect();
            if !joins.is_empty() {
                let join = join_pads(board, &joins).ok_or_else(|| {
                    tool_refusal(order.tool, Translatable::new(ids::PCB_ROUTE_JOIN_GONE))
                })?;
                commands.push(join);
            }
            (made, commands)
        };
        let revision = self.land_order(path, order.tool, commands, order.dry_run)?;
        Ok(RouteMade { revision, ..made })
    }

    /// Place `order`'s via on the board at `path` as one undo step (none for
    /// a dry run): the via and the board revision after it.
    pub(crate) fn run_via_order(
        &mut self,
        path: &Path,
        order: &ViaOrder,
    ) -> Result<(Via, u64), Translatable> {
        let tool = BoardPlaceTool::Via;
        let via = {
            let board = self
                .document_state
                .pcb_board(path)
                .ok_or_else(|| gone(tool))?;
            plan_via(board, order)?
        };
        let command = BoardCommand::AddItems {
            items: vec![BoardItem::Via(via.clone())],
        };
        let revision = self.land_order(path, tool, vec![command], order.dry_run)?;
        Ok((via, revision))
    }

    /// Run `commands` on the board at `path` as one step labelled with
    /// `tool`'s name, as a click lands; nothing for a dry run. The revision
    /// after it.
    fn land_order(
        &mut self,
        path: &Path,
        tool: BoardPlaceTool,
        commands: Vec<BoardCommand>,
        dry_run: bool,
    ) -> Result<u64, Translatable> {
        let revision = |app: &Self| {
            (app.document_state.board_engine(path))
                .map(|engine| engine.revision())
                .ok_or_else(|| gone(tool))
        };
        let before = revision(self)?;
        if dry_run || commands.is_empty() {
            return Ok(before);
        }
        let label = t!(self.ui_state.i18n, tool.name()).into_string();
        let step = CheckedStep {
            label: &label,
            expected_revision: before,
        };
        self.execute_board_commands(path, commands, Some(step))
            .map_err(|error| not_placed(tool, error.to_string()))?;
        self.refresh_pcb_highlight();
        revision(self)
    }
}

fn not_placed(tool: BoardPlaceTool, why: String) -> Translatable {
    let why = Translatable::new(ids::PCB_ROUTE_ORDER_NOT_PLACED).arg("why", why);
    tool_refusal(tool, why)
}

fn gone(tool: BoardPlaceTool) -> Translatable {
    not_placed(tool, "the board is not open".to_string())
}

/// The copper layer an order routes on: the one it names, else the layer
/// of the netted copper under its start.
pub(super) fn order_layer(
    board: &PcbBoard,
    order: &RouteOrder,
    start: Point,
) -> Result<CopperLayer, Translatable> {
    if let Some(layer) = order.layer {
        return Ok(layer);
    }
    copper_under(board, start)
        .map(|(layer, _)| layer)
        .ok_or_else(|| tool_refusal(order.tool, Translatable::new(ids::PCB_ROUTE_ORDER_NO_LAYER)))
}

/// The refusal of a leg aimed at `pad` of another net than `route_net`.
fn other_net(tool: BoardPlaceTool, board: &PcbBoard, pad: &PadAt, route_net: u32) -> Translatable {
    let why = Translatable::new(ids::PCB_ROUTE_END_OTHER_NET)
        .arg("pad", pad.label())
        .arg("pad_net", net_name(board, pad.net))
        .arg("net", net_name(board, route_net));
    tool_refusal(tool, why)
}

/// A one-net route: Track, Quick Route or Interactive Routing. The start
/// takes the click's style (`route::start_on_copper`, the width by the
/// tool's rule), each later point one leg as that tool's click adds it.
fn plan_single(
    board: &PcbBoard,
    rules: &DrcRules,
    classes: &[NetClass],
    order: &RouteOrder,
) -> Result<RouteMade, Translatable> {
    let tool = order.tool;
    let Some((&start, rest)) = order.points.split_first() else {
        return Err(tool_refusal(
            tool,
            Translatable::new(ids::PCB_ROUTE_ORDER_POINTS),
        ));
    };
    let layer = order_layer(board, order, start)?;
    let net_set = order.net.is_some();
    let mut style = TrackStyle::starting_at(board, start, layer);
    if let Some(net) = order.net {
        style.net = net;
    }
    if let Some(width) = order.width_mm {
        style.width_mm = width;
    }
    let mut style = start_on_copper(board, start, style, net_set);
    // A width given replaces the net class's, as the form's does (#2400).
    let classes: &[NetClass] = if order.width_mm.is_some() {
        &[]
    } else {
        classes
    };
    style = match tool {
        BoardPlaceTool::Track => style.with_class_width(board, classes),
        _ => TrackStyle {
            width_mm: rule_width(board, classes, rules, &style),
            ..style
        },
    };
    // Copper under the start off the route's layer: the first leg starts
    // with a via there (`route::next_leg_style`).
    let mut start_via = (style.layer != layer).then(|| Via {
        attributes: Default::default(),
        net: style.net,
        ..draw::new_via(board, Uuid::now_v7(), start)
    });
    if tool != BoardPlaceTool::Track {
        start_via = start_via.map(|via| rule_via(via, rules));
    }
    style.layer = layer;
    let mut router = RouterState {
        draft: RouteDraft::default(),
        rules: rules.clone(),
        revision: 0,
        refused: false,
        preview: None,
        bus: None,
        joined: Vec::new(),
        joined_lines: Vec::new(),
    };
    let mut joined = Vec::new();
    let mut anchor = start;
    for &to in rest {
        if style.net == 0 && !net_set {
            style.net = draw::net_under(board, to, 1 << pcb_geom::layer_bit(layer));
            style = style.with_class_width(board, classes);
        }
        let via = start_via.take();
        let legs = match tool {
            BoardPlaceTool::Track => track_leg(board, (anchor, to), &mut style, via, order)?,
            BoardPlaceTool::QuickRoute => vec![RoutedLeg {
                tracks: draw::track_run(anchor, to, order.corner, style, Uuid::now_v7),
                via,
                ..RoutedLeg::default()
            }],
            _ => {
                let (legs, pad) = router_leg(board, &router, (anchor, to), &mut style, via, order)?;
                joined.extend(pad.map(|pad| (pad, style.net)));
                legs
            }
        };
        for leg in legs {
            router.draft.add(leg);
        }
        anchor = to;
    }
    Ok(RouteMade {
        draft: router.draft,
        joined,
        layer,
        width_mm: style.width_mm,
        nets: vec![style.net],
        revision: 0,
    })
}

/// One Track leg: straight legs bent by the corner rule, a via at its end
/// for a pad off its layer; refused at a pad of another net
/// (`route::route_clicked`).
fn track_leg(
    board: &PcbBoard,
    (anchor, to): (Point, Point),
    style: &mut TrackStyle,
    start: Option<Via>,
    order: &RouteOrder,
) -> Result<Vec<RoutedLeg>, Translatable> {
    let end = end_via(board, to, style, |at| {
        draw::new_via(board, Uuid::now_v7(), at)
    });
    if let Some(refusal) = start_conflict(board, start.as_ref(), style.net, order.tool) {
        return Err(refusal);
    }
    if let PadEnd::OtherNet { pad, route_net } = pad_end(board, to, style, end.as_ref()) {
        return Err(other_net(order.tool, board, &pad, route_net));
    }
    let tracks = draw::track_run(anchor, to, order.corner, *style, Uuid::now_v7);
    if tracks.is_empty() && start.is_none() {
        return Ok(Vec::new());
    }
    let run = RoutedLeg {
        tracks,
        via: start,
        ..RoutedLeg::default()
    };
    let end = RoutedLeg {
        via: end,
        ..RoutedLeg::default()
    };
    Ok(vec![run, end])
}

/// One Interactive Routing leg, routed against the route so far, and the
/// pad of no net it joins (`router::router_clicked`, `route_end::leg_end`).
fn router_leg(
    board: &PcbBoard,
    router: &RouterState,
    (anchor, to): (Point, Point),
    style: &mut TrackStyle,
    start: Option<Via>,
    order: &RouteOrder,
) -> Result<(Vec<RoutedLeg>, Option<PadAt>), Translatable> {
    let tool = order.tool;
    let new_via = |at| draw::new_via(board, Uuid::now_v7(), at);
    let end = end_via(board, to, style, new_via).map(|via| rule_via(via, &router.rules));
    if let Some(refusal) = start_conflict(board, start.as_ref(), style.net, tool) {
        return Err(refusal);
    }
    let joins = match pad_end(board, to, style, end.as_ref()) {
        PadEnd::Free => None,
        PadEnd::Joins { pad, .. } => Some(pad),
        PadEnd::OtherNet { pad, route_net } => return Err(other_net(tool, board, &pad, route_net)),
    };
    let leg = LegRequest {
        from: anchor,
        to,
        style: *style,
        via: start,
    };
    let modes = (order.corner, order.mode, SEARCH_BUDGET);
    let legs = route_on(board, router, leg, modes, Uuid::now_v7)
        .and_then(|leg| with_end_via(board, router, leg, end, order.corner))
        .map_err(|refusal| Translatable::new(refusal_line(refusal)))?;
    Ok((legs, joins))
}

/// The refusal of a layer-change via at a route's start that would reach a
/// pad of another net.
fn start_conflict(
    board: &PcbBoard,
    via: Option<&Via>,
    net: u32,
    tool: BoardPlaceTool,
) -> Option<Translatable> {
    match via_pad_end(board, via?, net) {
        PadEnd::OtherNet { pad, route_net } => Some(other_net(tool, board, &pad, route_net)),
        PadEnd::Free | PadEnd::Joins { .. } => None,
    }
}

/// The via `order` names: the board's via at its point (`draw::new_via`,
/// what a click with an empty form places), then its net, size and span.
fn plan_via(board: &PcbBoard, order: &ViaOrder) -> Result<Via, Translatable> {
    let tool = BoardPlaceTool::Via;
    let mut via = draw::new_via(board, Uuid::now_v7(), order.at);
    if let Some(net) = order.net {
        via.net = net;
    }
    if let Some(diameter) = order.diameter_mm {
        via.diameter = diameter;
    }
    match order.drill_mm {
        Some(drill) if drill > via.diameter => {
            let why = Translatable::new(ids::PCB_ROUTE_ORDER_DRILL)
                .arg("drill", drill.to_string())
                .arg("diameter", via.diameter.to_string());
            return Err(tool_refusal(tool, why));
        }
        Some(drill) => via.drill = drill,
        None => via.drill = via.drill.min(via.diameter),
    }
    if let Some((a, b)) = order.span {
        let copper = board_copper_layers(&board.layers);
        edit_vias(
            std::slice::from_mut(&mut via),
            &ViaEdit::Span(a, b),
            &copper,
        )
        .map_err(|error| not_placed(tool, error.to_string()))?;
    }
    Ok(via)
}
