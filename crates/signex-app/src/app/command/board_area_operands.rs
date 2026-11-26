//! The area tools from their operands (#2470): `place_board_fill`,
//! `place_board_region`, `place_board_polygon_pour`, `place_board_keepout`
//! and `place_board_pour_cutout` on the board `document` names, on the
//! `layer` named, over the outline named (`start` and `end` for a Fill,
//! `corners` for the others, in mm from the board origin with Y up), and
//! for a copper area on the board net `net`. Each adds the zone the tool's
//! click adds for that outline (`signex_engine::pcb::area`), without the
//! person's form, as one board undo step, and answers with it as JSON; a
//! pour's answer carries its fill.

use std::collections::BTreeSet;
use std::path::Path;

use iced::Task;
use serde_json::{Value, json};
use signex_engine::pcb::area::{AreaKind, named_area};
use signex_engine::pcb::draw;
use signex_i18n::t;
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::{PcbBoard, Point, Zone};
use uuid::Uuid;

use super::board_edit_operands::mm_corners;
use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::handlers::pcb_pour::pour_report;
use crate::app::pcb_tools::BoardPlaceTool;
use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

fn point(args: &CommandArgs, name: &str) -> Option<Point> {
    match args.operand(name) {
        Some(OperandValue::Point(x, y)) => Some(Point::new(*x, *y)),
        _ => None,
    }
}

/// The outline the operands name, in the board's own coordinates.
fn outline(board: &PcbBoard, kind: AreaKind, args: &CommandArgs) -> Result<Vec<Point>, String> {
    let origin = board.options.origin;
    if kind == AreaKind::Fill {
        let (Some(start), Some(end)) = (point(args, "start"), point(args, "end")) else {
            return Err("a fill needs the operands `start` and `end`".to_string());
        };
        return Ok(draw::rectangle(
            origin.from_user(start),
            origin.from_user(end),
        ));
    }
    let raw = text(args, "corners").ok_or("this area needs the operand `corners`")?;
    let corners = mm_corners(raw)?;
    Ok(corners.into_iter().map(|p| origin.from_user(p)).collect())
}

/// The net `net` names on `board`: `None` when it is left out, `Some(0)`
/// for an empty name (no net).
fn named_net(board: &PcbBoard, args: &CommandArgs) -> Result<Option<u32>, String> {
    let Some(name) = text(args, "net") else {
        return Ok(None);
    };
    if name.trim().is_empty() {
        return Ok(Some(0));
    }
    pcb_geom::net_named(board, name)
        .map(Some)
        .ok_or_else(|| format!("the board has no net named `{name}`"))
}

/// The zone the operands of `kind` make on `board`.
fn new_zone(board: &PcbBoard, kind: AreaKind, args: &CommandArgs) -> Result<Zone, String> {
    let token = text(args, "layer").ok_or("needs the operand `layer`")?;
    let layer = parse_pcb_layer(token.trim())
        .ok_or_else(|| format!("`{token}` is not a board layer name"))?;
    let outline = outline(board, kind, args)?;
    let net = named_net(board, args)?;
    named_area(board, kind, Uuid::now_v7(), outline, layer, net).map_err(|why| why.to_string())
}

/// The area `id` as an answer carries it, with a pour's fill.
fn answer(board: &PcbBoard, path: &Path, kind: AreaKind, id: Uuid, revision: u64) -> Value {
    let zone = board.zones.iter().find(|zone| zone.uuid == id);
    let mut answer = json!({
        "document": path.display().to_string(),
        "id": id.to_string(),
        "kind": kind.id(),
        "layer": zone.map(|z| z.layer.as_str()),
        "net": zone.map(|z| z.net),
        "net_name": zone.map(|z| z.net_name.as_str()),
        "revision": revision,
        "zone": zone.and_then(|z| serde_json::to_value(z).ok()),
    });
    if zone.is_some_and(Zone::is_pour) {
        let fills = pcb_geom::zone_fills(board);
        let report = pour_report(board, &fills, Some(&BTreeSet::from([id])));
        answer["fill"] = report.into_iter().next().unwrap_or(Value::Null);
    }
    answer
}

/// Add the area `tool` makes from `args` to the board `document` names, as
/// one undo step labelled with the tool's name.
fn place_area(app: &mut Signex, args: &CommandArgs, tool: BoardPlaceTool) -> Ran {
    let Some(kind) = tool.area_kind() else {
        return refused("not an area tool");
    };
    let path = match app.editable_named_board(args) {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let Some(engine) = app.document_state.board_engine(&path) else {
        return refused("the board is not open");
    };
    let revision = engine.revision();
    let zone = match new_zone(engine.board(), kind, args) {
        Ok(zone) => zone,
        Err(reason) => return refused(reason),
    };
    let id = zone.uuid;
    let label = t!(app.ui_state.i18n, tool.name()).into_string();
    let step = CheckedStep {
        label: &label,
        expected_revision: revision,
    };
    if let Err(error) = app.execute_board_commands(&path, vec![draw::place_zone(zone)], Some(step))
    {
        return refused(error.to_string());
    }
    let Some(engine) = app.document_state.board_engine(&path) else {
        return refused("the board is not open");
    };
    let answer = answer(engine.board(), &path, kind, id, engine.revision());
    (CommandOutcome::Answered(answer), Task::none())
}

/// `place_board_fill document=<board> layer=<copper> start=x,y end=x,y
/// [net=..]`.
pub(super) fn place_fill(app: &mut Signex, args: &CommandArgs) -> Ran {
    place_area(app, args, BoardPlaceTool::Fill)
}

/// `place_board_region document=<board> layer=<copper> corners=.. [net=..]`.
pub(super) fn place_region(app: &mut Signex, args: &CommandArgs) -> Ran {
    place_area(app, args, BoardPlaceTool::SolidRegion)
}

/// `place_board_polygon_pour document=<board> layer=<copper> corners=..
/// [net=..]`.
pub(super) fn place_polygon_pour(app: &mut Signex, args: &CommandArgs) -> Ran {
    place_area(app, args, BoardPlaceTool::PolygonPour)
}

/// `place_board_keepout document=<board> layer=<copper or keep-out>
/// corners=..`.
pub(super) fn place_keepout(app: &mut Signex, args: &CommandArgs) -> Ran {
    place_area(app, args, BoardPlaceTool::Keepout)
}

/// `place_board_pour_cutout document=<board> layer=<copper> corners=..`.
pub(super) fn place_pour_cutout(app: &mut Signex, args: &CommandArgs) -> Ran {
    place_area(app, args, BoardPlaceTool::PourCutout)
}
