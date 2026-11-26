//! The rooms commands from their operands (#2457, #2935): what a script or
//! the CLI runs. Each names its board with `document`, runs the edit its
//! GUI route runs as one undo step and answers with what it made or moved
//! (`answer`). Create Rooms from Sheets reads the project's sheets off the
//! UI thread first and answers when they land. See `docs/CLI.md`, "Rooms
//! and placement".

use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::json;
use signex_engine::pcb::BoardCommand;
use signex_engine::pcb::rooms::{
    PlanShape, RoomOptions, SheetMembers, arrange_room, plan_rooms_from_sheets,
};
use signex_i18n::{Translatable, ids, t};
use signex_types::object_id::UuidV7Ids;
use signex_types::pcb::room::{next_room_name, palette_colour};
use signex_types::pcb::{PcbBoard, Point, Room, RoomLayer, RoomOutline, RoomSource};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::{CreateRoomsMsg, answer};
use crate::app::agent::WaitingCommand;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::project_sheets::ProjectSheetPlan;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

pub(super) type Ran = (CommandOutcome, Task<Message>);

/// A refusal with its catalog message.
pub(super) fn refused(reason: Translatable) -> Ran {
    (CommandOutcome::Disabled { reason }, Task::none())
}

/// A refusal whose reason the board, the engine or the document lookup
/// wrote, as Room Properties says it.
pub(super) fn refused_why(why: impl Into<String>) -> Ran {
    refused(Translatable::new(ids::ROOMS_REFUSED).arg("why", why.into()))
}

/// The trimmed text operand `name`; `None` when it is left out or blank.
pub(super) fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.trim()).filter(|t| !t.is_empty()),
        _ => None,
    }
}

fn point(args: &CommandArgs, name: &str) -> Option<Point> {
    match args.operand(name) {
        Some(OperandValue::Point(x, y)) => Some(Point::new(*x, *y)),
        _ => None,
    }
}

fn flag(args: &CommandArgs, name: &str) -> Option<bool> {
    match args.operand(name) {
        Some(OperandValue::Bool(on)) => Some(*on),
        _ => None,
    }
}

/// The room of `board` that `named` names: its uuid, else its name.
pub(super) fn named_room<'a>(board: &'a PcbBoard, named: &str) -> Result<&'a Room, Translatable> {
    if board.rooms.is_empty() {
        return Err(Translatable::new(ids::ROOMS_NO_ROOMS));
    }
    let by_id = Uuid::parse_str(named).ok();
    board
        .rooms
        .iter()
        .find(|r| Some(r.uuid) == by_id)
        .or_else(|| board.rooms.iter().find(|r| r.name == named))
        .ok_or_else(|| Translatable::new(ids::ROOMS_UNKNOWN_ROOM).arg("room", named.to_string()))
}

fn side(args: &CommandArgs) -> Result<Option<RoomLayer>, Translatable> {
    let Some(token) = text(args, "side") else {
        return Ok(None);
    };
    RoomLayer::from_token(token)
        .map(Some)
        .ok_or_else(|| Translatable::new(ids::ROOMS_BAD_SIDE).arg("side", token.to_string()))
}

/// `<x>,<y>;<x>,<y>;..` in mm, three corners or more.
fn corners(text: &str) -> Option<Vec<Point>> {
    crate::app::command::mm_points(text).filter(|points| points.len() >= 3)
}

/// The outline `start` and `end`, or `corners`, give.
fn outline(args: &CommandArgs) -> Result<RoomOutline, Translatable> {
    let (start, end) = (point(args, "start"), point(args, "end"));
    let outline = match (start, end, text(args, "corners")) {
        (Some(a), Some(b), None) => RoomOutline::rect_between(a, b),
        (None, None, Some(raw)) => RoomOutline::Polygon(corners(raw).ok_or_else(|| {
            Translatable::new(ids::ROOMS_BAD_CORNERS).arg("corners", raw.to_string())
        })?),
        _ => return Err(Translatable::new(ids::ROOMS_OUTLINE_NEEDED)),
    };
    if !outline.is_finite() || !outline.has_area() {
        return Err(Translatable::new(ids::ROOMS_NO_AREA));
    }
    Ok(outline)
}

/// The room `place_board_room`'s operands describe on `board`; `prefix`
/// names it `Room N` when no `name` is given.
fn room_of(board: &PcbBoard, args: &CommandArgs, prefix: &str) -> Result<Room, Translatable> {
    let outline = outline(args)?;
    let name = match text(args, "name") {
        Some(name) if board.rooms.iter().any(|r| r.name == name) => {
            return Err(Translatable::new(ids::ROOMS_NAME_TAKEN).arg("room", name.to_string()));
        }
        Some(name) => name.to_string(),
        None => next_room_name(&board.rooms, prefix),
    };
    let colour = palette_colour(board.rooms.len());
    let mut room = Room::drawn(Uuid::now_v7(), name, colour, outline);
    if let Some(raw) = text(args, "sheet") {
        let sheet = InstancePath::parse(raw)
            .filter(|s| !s.is_root())
            .ok_or_else(|| Translatable::new(ids::ROOMS_BAD_SHEET).arg("sheet", raw.to_string()))?;
        room.source = Some(RoomSource {
            sheet,
            channel: None,
        });
    }
    if let Some(layer) = side(args)? {
        room.layer = layer;
    }
    Ok(room)
}

fn document_of(path: &Path) -> String {
    path.display().to_string()
}

/// `place_board_room document=.. (start=x,y end=x,y | corners=..) name=?
/// sheet=? side=?`: the room as one undo step, answered with it.
pub(crate) fn place_room_by_operands(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match app.named_output_board(args) {
        Ok(path) => path,
        Err(why) => return refused_why(why),
    };
    let prefix = t!(app.ui_state.i18n, ids::ROOMS_DEFAULT_NAME).into_string();
    let Some(board) = app.document_state.pcb_board(&path) else {
        return refused_why("the board is not open");
    };
    let room = match room_of(board, args, &prefix) {
        Ok(room) => room,
        Err(reason) => return refused(reason),
    };
    let id = room.uuid;
    let add = BoardCommand::AddRoom {
        room: Box::new(room),
    };
    if let Err(why) = app.run_room_step(&path, vec![add], ids::ROOMS_UNDO_PLACE) {
        return refused_why(why);
    }
    let Some(board) = app.document_state.pcb_board(&path) else {
        return refused_why("the board is not open");
    };
    let Some(room) = board.rooms.iter().find(|r| r.uuid == id) else {
        return refused_why("the room was not placed");
    };
    let answer = json!({"document": document_of(&path), "room": answer::room(room, board)});
    (CommandOutcome::Answered(answer), Task::none())
}

/// What a `create_rooms_from_sheets` run with operands asks for: the
/// dialog's options and the rooms to make by name (`None`: every room).
#[derive(Debug, Clone, PartialEq)]
pub struct RoomsRequest {
    pub(crate) options: RoomOptions,
    pub(crate) only: Option<Vec<String>>,
}

/// Create Rooms from Sheets' request from the operands, over the dialog's
/// defaults.
fn rooms_request(args: &CommandArgs) -> Result<RoomsRequest, Translatable> {
    let only = text(args, "rooms").map(|list| {
        list.split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .collect()
    });
    Ok(RoomsRequest {
        options: room_options(args)?,
        only,
    })
}

fn room_options(args: &CommandArgs) -> Result<RoomOptions, Translatable> {
    let mut options = RoomOptions::default();
    if let Some(OperandValue::Length(mm)) = args.operand("margin") {
        options.margin_mm = *mm;
    }
    if let Some(shape) = text(args, "shape") {
        options.shape = match shape {
            "around_parts" => PlanShape::AroundParts,
            "even_grid" => PlanShape::EvenGrid,
            other => {
                return Err(Translatable::new(ids::ROOMS_BAD_SHAPE).arg("shape", other.to_string()));
            }
        };
    }
    if let Some(layer) = side(args)? {
        options.layer = layer;
    }
    options.move_parts = flag(args, "move_parts").unwrap_or(options.move_parts);
    options.keep_rule = flag(args, "keep_parts_inside").unwrap_or(options.keep_rule);
    options.replace_same_name = flag(args, "replace").unwrap_or(options.replace_same_name);
    Ok(options)
}

/// `create_rooms_from_sheets document=.. margin=? shape=? side=?
/// move_parts=? keep_parts_inside=? replace=? rooms=?`: the project's
/// sheets are read off the UI thread, then the rooms are made as one undo
/// step and the command is answered ([`CommandOutcome::Deferred`]). The
/// dialog's Create runs it on the sheets the dialog read, at once.
pub(crate) fn create_rooms_by_operands(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match app.named_output_board(args) {
        Ok(path) => path,
        Err(why) => return refused_why(why),
    };
    let request = match rooms_request(args) {
        Ok(request) => request,
        Err(reason) => return refused(reason),
    };
    let read = app.agent.from_view.then(|| {
        let dialog = app.ui_state.rooms.create.as_deref();
        dialog.filter(|d| d.board == path).map(|d| d.sheets.clone())
    });
    if let Some(sheets) = read.flatten() {
        let outcome = match app.make_rooms_from_sheets(&path, Ok(sheets), &request) {
            Ok(answer) => CommandOutcome::Answered(answer),
            Err(reason) => CommandOutcome::Disabled { reason },
        };
        return (outcome, Task::none());
    }
    let Some((plan, project_dir, _)) = app.room_sheet_source(&path) else {
        let board = path.file_name().map_or_else(
            || document_of(&path),
            |name| name.to_string_lossy().into_owned(),
        );
        return refused(Translatable::new(ids::ROOMS_BOARD_NO_PROJECT).arg("board", board));
    };
    let command = match AppCommandId::new("create_rooms_from_sheets") {
        Ok(command) => command,
        Err(why) => return refused_why(why),
    };
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    let task = read_sheets_for(plan, project_dir, waiting, path, request);
    (CommandOutcome::Deferred { ticket }, task)
}

/// Read the sheets for a command run with operands; it is answered when
/// they land (`CreateRoomsMsg::SheetsReadFor`).
fn read_sheets_for(
    plan: ProjectSheetPlan,
    project_dir: PathBuf,
    waiting: WaitingCommand,
    board: PathBuf,
    request: RoomsRequest,
) -> Task<Message> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || super::update::sheet_members(plan, &project_dir))
                .await
                .unwrap_or_else(|e| Err(format!("the worker stopped: {e}")))
        },
        move |result| {
            Message::from(CreateRoomsMsg::SheetsReadFor {
                waiting: Box::new(waiting),
                board,
                request: Box::new(request),
                result,
            })
        },
    )
}

impl Signex {
    /// The sheets for a command run with operands landed: make the rooms
    /// and answer its caller.
    pub(super) fn finish_rooms_from_sheets(
        &mut self,
        waiting: WaitingCommand,
        board: PathBuf,
        request: Box<RoomsRequest>,
        result: Result<Vec<SheetMembers>, String>,
    ) -> Task<Message> {
        let target = board.clone();
        self.finish_waiting_command(waiting, &target, move |app| {
            match app.make_rooms_from_sheets(&board, result, &request) {
                Ok(answer) => CommandOutcome::Answered(answer),
                Err(reason) => CommandOutcome::Disabled { reason },
            }
        })
    }

    /// The rooms `sheets` get on the board at `path` as one undo step, as
    /// the dialog's Create makes the ticked ones; the rooms made, the parts
    /// moved and the names kept or replaced.
    fn make_rooms_from_sheets(
        &mut self,
        path: &Path,
        sheets: Result<Vec<SheetMembers>, String>,
        request: &RoomsRequest,
    ) -> Result<serde_json::Value, Translatable> {
        let sheets = match sheets {
            Ok(sheets) if sheets.is_empty() => return Err(Translatable::new(ids::ROOMS_NO_SHEETS)),
            Ok(sheets) => sheets,
            Err(why) => return Err(Translatable::new(ids::ROOMS_READ_FAILED).arg("why", why)),
        };
        let not_open = || Translatable::new(ids::ROOMS_REFUSED).arg("why", "the board is not open");
        let board = self.document_state.pcb_board(path).ok_or_else(not_open)?;
        let mut plan = plan_rooms_from_sheets(board, &sheets, &request.options, &mut UuidV7Ids);
        if let Some(only) = &request.only {
            plan.retain_rooms(|name| only.iter().any(|n| n == name));
        }
        let moved = answer::moves(board, &plan.moves);
        let replaced: Vec<String> = plan
            .rooms
            .iter()
            .filter_map(|p| p.replaces)
            .filter_map(|id| board.rooms.iter().find(|r| r.uuid == id))
            .map(|r| r.name.clone())
            .collect();
        let made: Vec<Uuid> = plan.rooms.iter().map(|p| p.room.uuid).collect();
        let commands = plan.commands();
        if !commands.is_empty() {
            self.run_room_step(path, commands, ids::ROOMS_UNDO_CREATE)
                .map_err(|why| Translatable::new(ids::ROOMS_REFUSED).arg("why", why))?;
        }
        let board = self.document_state.pcb_board(path).ok_or_else(not_open)?;
        let rooms: Vec<serde_json::Value> = made
            .iter()
            .filter_map(|id| board.rooms.iter().find(|r| r.uuid == *id))
            .map(|r| answer::room(r, board))
            .collect();
        Ok(json!({
            "document": document_of(path),
            "rooms": rooms,
            "moved": moved,
            "kept": plan.kept,
            "replaced": replaced,
        }))
    }
}

/// `arrange_in_room document=.. room=<name or uuid>`: the room's unlocked
/// parts packed inside it as one undo step, answered with each part moved
/// and the parts that did not fit.
pub(crate) fn arrange_in_room_by_operands(app: &mut Signex, args: &CommandArgs) -> Ran {
    let path = match app.named_output_board(args) {
        Ok(path) => path,
        Err(why) => return refused_why(why),
    };
    let Some(board) = app.document_state.pcb_board(&path) else {
        return refused_why("the board is not open");
    };
    let room = match named_room(board, text(args, "room").unwrap_or_default()) {
        Ok(room) => room.clone(),
        Err(reason) => return refused(reason),
    };
    let arranged = arrange_room(&room, board);
    let moved = answer::moves(board, &arranged.moves);
    let outside = answer::designators(board, &arranged.outside);
    let commands: Vec<BoardCommand> = arranged
        .moves
        .iter()
        .map(|&(footprint, to)| BoardCommand::MoveFootprint { footprint, to })
        .collect();
    if !commands.is_empty()
        && let Err(why) = app.run_room_step(&path, commands, ids::ROOMS_UNDO_ARRANGE)
    {
        return refused_why(why);
    }
    let answer = json!({
        "document": document_of(&path),
        "room": room.name,
        "moved": moved,
        "outside": outside,
    });
    (CommandOutcome::Answered(answer), Task::none())
}
