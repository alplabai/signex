//! The routing tools and the Via tool from their operands (#2470): what the
//! command line runs to route a board net by net through the interactive
//! router, on the board `document` names. The points count from the board
//! origin with Y up; a point also takes a pad as `<designator>.<pad>`. The
//! route is planned and landed by `handlers::pcb_tool::route_order` as one
//! undo step, and answered with the tracks, vias and pads it made. See
//! `docs/CLI.md`, "Routing a board".

use std::path::Path;

use iced::Task;
use serde_json::{Value, json};
use signex_engine::pcb::route::RouteMode;
use signex_i18n::{Translatable, ids};
use signex_types::corner::{CornerMode, CornerRule};
use signex_types::format::{mm_as_stored_nm, parse_pcb_layer, pcb_layer_token};
use signex_types::pcb::{PcbBoard, Point, Segment, Via};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::pcb_stack::board_copper_layers;

use super::operand_handlers::refused;
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::handlers::pcb_tool::{RouteMade, RouteOrder, ViaOrder, tool_refusal};
use crate::app::pcb_tools::BoardPlaceTool;
use crate::app::{Message, Signex};
use crate::panels::pcb_primitive_properties::parse_number;

type Ran = (CommandOutcome, Task<Message>);

fn disabled(reason: Translatable) -> Ran {
    (CommandOutcome::Disabled { reason }, Task::none())
}

/// `place_board_track document=.. start=.. end=.. [through=..] [layer=..]
/// [width=..] [net=..] [corner=..] [dry_run=..]`.
pub(super) fn place_board_track(app: &mut Signex, args: &CommandArgs) -> Ran {
    route(app, args, BoardPlaceTool::Track)
}

/// `place_board_quick_route`, as `place_board_track`: straight legs at the
/// rule width, other nets not avoided.
pub(super) fn place_board_quick_route(app: &mut Signex, args: &CommandArgs) -> Ran {
    route(app, args, BoardPlaceTool::QuickRoute)
}

/// `place_board_route`, as `place_board_track` with `mode=walkaround|shove`.
pub(super) fn place_board_route(app: &mut Signex, args: &CommandArgs) -> Ran {
    route(app, args, BoardPlaceTool::InteractiveRouting)
}

/// `place_board_diff_pair document=.. net=.. start=.. end=.. [through=..]
/// [layer=..] [corner=..] [dry_run=..]`.
pub(super) fn place_board_diff_pair(app: &mut Signex, args: &CommandArgs) -> Ran {
    route(app, args, BoardPlaceTool::DiffPairRouting)
}

/// `place_board_multi_route document=.. nets=a,b,.. start=.. end=..
/// [through=..] [layer=..] [corner=..] [dry_run=..]`.
pub(super) fn place_board_multi_route(app: &mut Signex, args: &CommandArgs) -> Ran {
    route(app, args, BoardPlaceTool::MultiRoute)
}

fn route(app: &mut Signex, args: &CommandArgs, tool: BoardPlaceTool) -> Ran {
    let path = match app.editable_named_board(args) {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let Some(board) = app.document_state.pcb_board(&path) else {
        return refused("the board is not open");
    };
    let order = match route_order(board, args, tool) {
        Ok(order) => order,
        Err(why) => return disabled(why),
    };
    let made = match app.run_route_order(&path, &order) {
        Ok(made) => made,
        Err(why) => return disabled(why),
    };
    let Some(board) = app.document_state.pcb_board(&path) else {
        return refused("the board is not open");
    };
    let answer = route_answer(board, &path, &order, &made);
    (CommandOutcome::Answered(answer), Task::none())
}

/// `place_board_via document=.. at=.. [net=..] [layers=..]
/// [via_diameter=..] [via_drill=..] [dry_run=..]`.
pub(super) fn place_board_via(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match app.editable_named_board(args) {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let Some(board) = app.document_state.pcb_board(&path) else {
        return refused("the board is not open");
    };
    let order = match via_order(board, args) {
        Ok(order) => order,
        Err(why) => return disabled(why),
    };
    let (via, revision) = match app.run_via_order(&path, &order) {
        Ok(placed) => placed,
        Err(why) => return disabled(why),
    };
    let Some(board) = app.document_state.pcb_board(&path) else {
        return refused("the board is not open");
    };
    let answer = json!({
        "document": path.display().to_string(),
        "dry_run": order.dry_run,
        "revision": revision,
        "vias": [via_json(board, &via)],
    });
    (CommandOutcome::Answered(answer), Task::none())
}

/// The route the operands of `tool` ask for on `board`.
fn route_order(
    board: &PcbBoard,
    args: &CommandArgs,
    tool: BoardPlaceTool,
) -> Result<RouteOrder, Translatable> {
    let refuse = |why: Translatable| tool_refusal(tool, why);
    let mut points = vec![board_point(board, args, "start").map_err(refuse)?];
    points.extend(through(board, args).map_err(refuse)?);
    points.push(board_point(board, args, "end").map_err(refuse)?);
    let layer = (text(args, "layer").map(|name| copper_layer(board, name)))
        .transpose()
        .map_err(refuse)?;
    let width_mm = (text(args, "width").map(|value| length("width", value)))
        .transpose()
        .map_err(refuse)?;
    let net = (text(args, "net").map(|name| net_number(board, name)))
        .transpose()
        .map_err(refuse)?;
    let nets = match text(args, "nets") {
        Some(names) => net_list(board, names).map_err(refuse)?,
        None => Vec::new(),
    };
    let corner = (text(args, "corner").map(corner_rule))
        .transpose()
        .map_err(refuse)?
        .unwrap_or_default();
    let mode = (text(args, "mode").map(route_mode))
        .transpose()
        .map_err(refuse)?
        .unwrap_or_default();
    Ok(RouteOrder {
        tool,
        points,
        layer,
        width_mm,
        net,
        nets,
        corner,
        mode,
        dry_run: flag(args, "dry_run"),
    })
}

/// The via the operands of `place_board_via` ask for on `board`.
fn via_order(board: &PcbBoard, args: &CommandArgs) -> Result<ViaOrder, Translatable> {
    let refuse = |why: Translatable| tool_refusal(BoardPlaceTool::Via, why);
    let at = board_point(board, args, "at").map_err(refuse)?;
    let net = (text(args, "net").map(|name| net_number(board, name)))
        .transpose()
        .map_err(refuse)?;
    let span = (text(args, "layers").map(|names| via_span(board, names)))
        .transpose()
        .map_err(refuse)?;
    let size = |name: &'static str| {
        text(args, name)
            .map(|value| length(name, value))
            .transpose()
            .map_err(refuse)
    };
    Ok(ViaOrder {
        at,
        net,
        span,
        diameter_mm: size("via_diameter")?,
        drill_mm: size("via_drill")?,
        dry_run: flag(args, "dry_run"),
    })
}

use super::operand_str as text;

fn flag(args: &CommandArgs, name: &str) -> bool {
    matches!(args.operand(name), Some(OperandValue::Bool(true)))
}

/// The board point the operand `name` gives: a coordinate from the board
/// origin with Y up, or the centre of the pad `<designator>.<pad>`.
fn board_point(board: &PcbBoard, args: &CommandArgs, name: &str) -> Result<Point, Translatable> {
    match args.operand(name) {
        Some(OperandValue::Point(x, y)) => {
            stored(board.options.origin.from_user(Point::new(*x, *y)))
        }
        Some(OperandValue::PinRef { designator, pin }) => pad_point(board, designator, pin),
        _ => Err(Translatable::new(ids::PCB_ROUTE_ORDER_POINTS)),
    }
}

/// `p` when the board file can store it (#2985).
fn stored(p: Point) -> Result<Point, Translatable> {
    let fits = mm_as_stored_nm(p.x).is_some() && mm_as_stored_nm(p.y).is_some();
    fits.then_some(p)
        .ok_or_else(|| Translatable::new(ids::PCB_ROUTE_ORDER_RANGE))
}

/// The centre of the one pad `pin` of the part `designator`.
fn pad_point(board: &PcbBoard, designator: &str, pin: &str) -> Result<Point, Translatable> {
    let label = format!("{designator}.{pin}");
    let hits: Vec<_> = (board.footprints.iter())
        .filter(|footprint| footprint.reference == designator)
        .flat_map(|footprint| {
            (footprint.pads.iter())
                .filter(|pad| pad.number == pin)
                .map(move |pad| (footprint, pad))
        })
        .collect();
    match hits[..] {
        [(footprint, pad)] => pcb_geom::pad_world_center(footprint, pad)
            .map(|centre| centre.to_mm())
            .map_err(|_| Translatable::new(ids::PCB_ROUTE_ORDER_PAD).arg("pad", label)),
        [] => Err(Translatable::new(ids::PCB_ROUTE_ORDER_PAD).arg("pad", label)),
        _ => Err(Translatable::new(ids::PCB_ROUTE_ORDER_PAD_MANY).arg("pad", label)),
    }
}

/// The corners `through` lists, `x,y;x,y`, as board points.
fn through(board: &PcbBoard, args: &CommandArgs) -> Result<Vec<Point>, Translatable> {
    let Some(list) = text(args, "through").filter(|list| !list.trim().is_empty()) else {
        return Ok(Vec::new());
    };
    let origin = board.options.origin;
    let points =
        super::mm_points(list).ok_or_else(|| Translatable::new(ids::PCB_ROUTE_ORDER_POINTS))?;
    points
        .into_iter()
        .map(|p| stored(origin.from_user(p)))
        .collect()
}

/// The copper layer `name` names, when the board has it.
fn copper_layer(board: &PcbBoard, name: &str) -> Result<CopperLayer, Translatable> {
    parse_pcb_layer(name.trim())
        .and_then(PcbLayer::copper)
        .filter(|layer| board_copper_layers(&board.layers).contains(layer))
        .ok_or_else(|| Translatable::new(ids::PCB_ROUTE_ORDER_LAYER).arg("layer", name.to_string()))
}

/// The two copper layers of the board a via joins, `TopCopper,Inner2Copper`.
fn via_span(board: &PcbBoard, names: &str) -> Result<(CopperLayer, CopperLayer), Translatable> {
    let copper = board_copper_layers(&board.layers);
    let layers: Option<Vec<CopperLayer>> = (names.split(','))
        .map(|name| parse_pcb_layer(name.trim()).and_then(PcbLayer::copper))
        .collect();
    match layers.as_deref() {
        Some(&[a, b]) if a != b && copper.contains(&a) && copper.contains(&b) => Ok((a, b)),
        _ => Err(Translatable::new(ids::PCB_ROUTE_ORDER_SPAN)),
    }
}

/// A length in mm above zero that the board file can store (#2985): a
/// size past its range would make every later Save fail.
fn length(operand: &'static str, value: &str) -> Result<f64, Translatable> {
    parse_number(value)
        .filter(|mm| *mm > 0.0 && mm_as_stored_nm(*mm).is_some())
        .ok_or_else(|| Translatable::new(ids::PCB_ROUTE_ORDER_LENGTH).arg("operand", operand))
}

/// The number of the board net named `name`.
fn net_number(board: &PcbBoard, name: &str) -> Result<u32, Translatable> {
    (board.nets.iter())
        .find(|net| net.name == name)
        .map(|net| net.number)
        .ok_or_else(|| Translatable::new(ids::BOARD_NET_UNKNOWN).arg("net", name.to_string()))
}

/// The nets `names` lists, comma separated, each once; two or more.
fn net_list(board: &PcbBoard, names: &str) -> Result<Vec<u32>, Translatable> {
    let mut nets = Vec::new();
    for name in names.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        let net = net_number(board, name)?;
        if !nets.contains(&net) {
            nets.push(net);
        }
    }
    if nets.len() < 2 {
        return Err(Translatable::new(ids::PCB_ROUTE_ORDER_NETS));
    }
    Ok(nets)
}

/// How each leg bends: `90`, `45_start`, `45_end` or `free`.
pub(super) fn corner_rule(name: &str) -> Result<CornerRule, Translatable> {
    let mode = match name.trim() {
        "90" => CornerMode::Ortho90,
        "45_start" => CornerMode::Angle45Start,
        "45_end" => CornerMode::Angle45End,
        "free" => CornerMode::FreeAngle,
        _ => return Err(Translatable::new(ids::PCB_ROUTE_ORDER_CORNER)),
    };
    Ok(CornerRule {
        mode,
        is_flipped: false,
    })
}

fn route_mode(name: &str) -> Result<RouteMode, Translatable> {
    match name.trim() {
        "walkaround" => Ok(RouteMode::Walkaround),
        "shove" => Ok(RouteMode::Shove),
        _ => Err(Translatable::new(ids::PCB_ROUTE_ORDER_MODE)),
    }
}

/// A net by name; `null` for no net.
fn net_value(board: &PcbBoard, net: u32) -> Value {
    (board.nets.iter())
        .find(|n| n.number == net && net != 0)
        .map_or(Value::Null, |n| json!(n.name))
}

/// A board point as the status bar shows it, `[x, y]` from the origin.
fn user_point(board: &PcbBoard, at: Point) -> Value {
    let shown = board.options.origin.to_user(at);
    json!([shown.x, shown.y])
}

fn track_json(board: &PcbBoard, track: &Segment) -> Value {
    json!({
        "id": track.uuid.to_string(),
        "start": user_point(board, track.start),
        "end": user_point(board, track.end),
        "width_mm": track.width,
        "layer": track.layer,
        "net": net_value(board, track.net),
    })
}

fn via_json(board: &PcbBoard, via: &Via) -> Value {
    json!({
        "id": via.uuid.to_string(),
        "at": user_point(board, via.position),
        "layers": via.layers,
        "via_type": via.via_type,
        "diameter_mm": via.diameter,
        "drill_mm": via.drill,
        "net": net_value(board, via.net),
    })
}

/// What a route made, as `docs/CLI.md` "Routing a board" lists it.
fn route_answer(board: &PcbBoard, path: &Path, order: &RouteOrder, made: &RouteMade) -> Value {
    let draft = &made.draft;
    json!({
        "document": path.display().to_string(),
        "dry_run": order.dry_run,
        "revision": made.revision,
        "layer": pcb_layer_token(PcbLayer::Copper(made.layer)),
        "width_mm": made.width_mm,
        "nets": made.nets.iter().map(|net| net_value(board, *net)).collect::<Vec<_>>(),
        "tracks": draft.tracks.iter().map(|t| track_json(board, t)).collect::<Vec<_>>(),
        "vias": draft.vias.iter().map(|v| via_json(board, v)).collect::<Vec<_>>(),
        "pushed": draft.shoved.iter().map(|t| t.uuid.to_string()).collect::<Vec<_>>(),
        "joined_pads": made.joined.iter().map(|(pad, net)| json!({
            "pad": pad.label(),
            "net": net_value(board, *net),
        })).collect::<Vec<_>>(),
    })
}
