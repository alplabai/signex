//! The board Tools commands from their operands (#2408, #2409): what a CLI
//! or an agent runs. Each names its board with `document` and the items it
//! acts on by uuid (`id`, `ids`) or by point (`start`, `end`), never by the
//! person's selection, builds the same edit the menu row builds, runs it as
//! one undo step and answers with what it did (`ToolReport` as JSON).

use std::collections::BTreeSet;
use std::path::PathBuf;

use iced::Task;
use signex_engine::pcb::board_tools::{
    StitchArea, ToolEdit, ToolRefusal, add_teardrops, arrange_in_box, default_stitch_pitch_mm,
    gloss_tracks, keepout_from_selection, outline_from_tracks, remove_teardrops, stitch_vias,
};
use signex_i18n::{Translatable, ids};
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::{clearance_rules, pair_gap};
use crate::app::command::{CommandArgs, CommandOutcome, OperandError, OperandValue, parse_targets};
use crate::app::{DocumentKey, Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

/// The loaded board `document` names.
fn named_board(app: &Signex, args: &CommandArgs) -> Result<PathBuf, String> {
    let Some(OperandValue::Document(target)) = args.operand("document") else {
        return Err("the operand `document` is needed".to_string());
    };
    let entry = app.resolve_document_target(target)?;
    match entry.key {
        Some(DocumentKey::Pcb(path)) => Ok(path),
        Some(_) => Err(format!(
            "this acts on a board, and {} is a {}",
            entry.name,
            entry.kind.as_str()
        )),
        None => Err(format!(
            "{} is referenced by a project but not loaded; open it first",
            entry.name
        )),
    }
}

/// The items `ids` names; empty when it was not given.
fn targets(args: &CommandArgs) -> BTreeSet<Uuid> {
    args.targets("ids")
        .map_or_else(BTreeSet::new, |ids| ids.iter().copied().collect())
}

/// The zone the text operand `id` names, if any: empty comma parts are
/// skipped and a zone named twice is one, as before #2326.
fn zone_id(args: &CommandArgs) -> Result<Option<Uuid>, String> {
    let Some(OperandValue::Text(text)) = args.operand("id") else {
        return Ok(None);
    };
    if text.split(',').all(|part| part.trim().is_empty()) {
        return Ok(None);
    }
    let one_zone = || "`id` names one zone".to_string();
    match parse_targets("id", text) {
        Ok(ids) => match ids[..] {
            [zone] => Ok(Some(zone)),
            _ => Err(one_zone()),
        },
        Err(OperandError::Invalid { got, .. }) => Err(format!("`{got}` in `id` is not a uuid")),
        Err(_) => Err(one_zone()),
    }
}

/// Refused whole when an id of `ids` names nothing `acts_on` takes on the
/// named board (#2326): a command never runs on the rest and skips one.
fn unknown_target(
    app: &Signex,
    args: &CommandArgs,
    acts_on: impl Fn(&PcbBoard, Uuid) -> bool,
) -> Option<Ran> {
    let path = named_board(app, args).ok()?;
    let board = app.document_state.pcb_board(&path)?;
    let id = args
        .targets("ids")?
        .iter()
        .find(|id| !acts_on(board, **id))?;
    let reason = Translatable::new(ids::BOARD_TARGET_UNKNOWN).arg("id", id.to_string());
    Some((CommandOutcome::Disabled { reason }, Task::none()))
}

/// Refused when `ids` names both zones and lines: the keepout is made from
/// the zones alone, so the lines would be skipped without a word.
fn zones_and_lines(app: &Signex, args: &CommandArgs) -> Option<Ran> {
    let path = named_board(app, args).ok()?;
    let board = app.document_state.pcb_board(&path)?;
    let named = args.targets("ids")?;
    let is_zone = |id: &Uuid| board.zones.iter().any(|z| z.uuid == *id);
    let mixed = named.iter().any(is_zone) && named.iter().any(|id| is_track(board, *id));
    let reason = Translatable::new(ids::BOARD_KEEPOUT_MIXED);
    mixed.then_some((CommandOutcome::Disabled { reason }, Task::none()))
}

fn is_track(board: &PcbBoard, id: Uuid) -> bool {
    board.segments.iter().any(|s| s.uuid == id)
}

/// The board point the operand `name` gives, if any.
fn point(args: &CommandArgs, name: &str) -> Result<Option<Point>, String> {
    match args.operand(name) {
        None => Ok(None),
        Some(OperandValue::Point(x, y)) => Ok(Some(Point::new(*x, *y))),
        Some(_) => Err(format!("`{name}` takes a board coordinate x,y in mm")),
    }
}

/// The net the `net` operand names on `board`, by its name.
fn net(board: &PcbBoard, args: &CommandArgs) -> Result<Option<u32>, ToolRefusal> {
    let Some(OperandValue::Text(name)) = args.operand("net") else {
        return Ok(None);
    };
    board
        .nets
        .iter()
        .find(|n| n.name == *name)
        .map(|n| Some(n.number))
        .ok_or_else(|| ToolRefusal::UnknownNet(name.clone()))
}

/// Build an edit on the named board with `build`, run it as one undo step
/// and answer with its report. A tool that lays copper (`copper`) is
/// refused when the board's clearance rule cannot be read.
fn run(
    app: &mut Signex,
    args: &CommandArgs,
    copper: bool,
    build: impl FnOnce(&PcbBoard, &dyn Fn(u32, u32) -> i64) -> Result<ToolEdit, String>,
) -> Ran {
    let path = match named_board(app, args) {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let Some(board) = app.document_state.pcb_board(&path) else {
        return refused(format!("no board is open at {}", path.display()));
    };
    let rules = match clearance_rules(app, &path, board) {
        Ok(rules) => Some(rules),
        Err(refusal) if copper => return refused(refusal.to_string()),
        Err(_) => None,
    };
    let gap = |a: u32, b: u32| pair_gap(rules.as_ref(), a, b);
    let built = match build(board, &gap) {
        Ok(edit) => Ok(edit),
        Err(reason) => return refused(reason),
    };
    match app.run_board_tool(&path, built) {
        Ok(report) => match serde_json::to_value(&report) {
            Ok(answer) => (CommandOutcome::Answered(answer), Task::none()),
            Err(error) => refused(error.to_string()),
        },
        Err(reason) => refused(reason),
    }
}

/// `stitch_vias document=.. (id=<zone> | start=x,y end=x,y) net=? pitch=?`.
pub(crate) fn stitch_by_operands(app: &mut Signex, args: &CommandArgs) -> Ran {
    run(app, args, true, |board, gap| {
        let zone = zone_id(args)?;
        let corners = (point(args, "start")?, point(args, "end")?);
        let area = match (zone, corners) {
            (Some(zone), (None, None)) => StitchArea::Zone(zone),
            (None, (Some(a), Some(b))) => StitchArea::Rect(a, b),
            _ => return Err("name the area with `id` (a zone) or with `start` and `end`".into()),
        };
        let pitch = match args.operand("pitch") {
            Some(OperandValue::Length(mm)) => *mm,
            _ => default_stitch_pitch_mm(board),
        };
        let net = net(board, args).map_err(|r| r.to_string())?;
        stitch_vias(board, area, net, pitch, gap, Uuid::now_v7).map_err(|r| r.to_string())
    })
}

/// The nets `net` names, or every net.
fn nets_of(board: &PcbBoard, args: &CommandArgs) -> Result<Option<BTreeSet<u32>>, String> {
    Ok(net(board, args)
        .map_err(|r| r.to_string())?
        .map(|n| BTreeSet::from([n])))
}

/// `add_teardrops document=.. net=?`.
pub(crate) fn add_teardrops_by_operands(app: &mut Signex, args: &CommandArgs) -> Ran {
    run(app, args, true, |board, gap| {
        let nets = nets_of(board, args)?;
        add_teardrops(board, nets.as_ref(), gap, Uuid::now_v7).map_err(|r| r.to_string())
    })
}

/// `remove_teardrops document=.. net=?`.
pub(crate) fn remove_teardrops_by_operands(app: &mut Signex, args: &CommandArgs) -> Ran {
    run(app, args, false, |board, _| {
        let nets = nets_of(board, args)?;
        remove_teardrops(board, nets.as_ref()).map_err(|r| r.to_string())
    })
}

/// `gloss_selected_routes document=.. ids=<tracks>`.
pub(crate) fn gloss_by_operands(app: &mut Signex, args: &CommandArgs) -> Ran {
    if let Some(refused) = unknown_target(app, args, is_track) {
        return refused;
    }
    run(app, args, true, |board, gap| {
        gloss_tracks(board, &targets(args), gap).map_err(|r| r.to_string())
    })
}

/// `board_shape_from_selection document=.. ids=<lines>`.
pub(crate) fn board_shape_by_operands(app: &mut Signex, args: &CommandArgs) -> Ran {
    if let Some(refused) = unknown_target(app, args, is_track) {
        return refused;
    }
    run(app, args, false, |board, _| {
        outline_from_tracks(board, &targets(args)).map_err(|r| r.to_string())
    })
}

/// `keepout_from_selection document=.. ids=<zones or lines>`.
pub(crate) fn keepout_by_operands(app: &mut Signex, args: &CommandArgs) -> Ran {
    let zone_or_line =
        |board: &PcbBoard, id| is_track(board, id) || board.zones.iter().any(|z| z.uuid == id);
    if let Some(refused) = unknown_target(app, args, zone_or_line) {
        return refused;
    }
    if let Some(refused) = zones_and_lines(app, args) {
        return refused;
    }
    run(app, args, false, |board, _| {
        let ids = targets(args);
        let zones: BTreeSet<Uuid> = board
            .zones
            .iter()
            .map(|z| z.uuid)
            .filter(|id| ids.contains(id))
            .collect();
        keepout_from_selection(board, &zones, &ids, Uuid::now_v7()).map_err(|r| r.to_string())
    })
}

/// `arrange_in_box document=.. ids=<components> start=x,y end=x,y`.
pub(crate) fn arrange_in_box_by_operands(app: &mut Signex, args: &CommandArgs) -> Ran {
    run(app, args, false, |board, _| {
        let (Some(a), Some(b)) = (point(args, "start")?, point(args, "end")?) else {
            return Err("the box needs `start` and `end`".into());
        };
        arrange_in_box(board, &targets(args), a, b).map_err(|r| r.to_string())
    })
}
