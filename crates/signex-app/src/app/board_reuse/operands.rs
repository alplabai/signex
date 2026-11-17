//! The design reuse commands from their operands (#2410): an agent or the
//! CLI names the board (`document`), the block (`name`) and what to save
//! (`ids`) or where to place it (`designator`, `degrees`), and gets what was
//! saved or placed as JSON. The same `Signex` calls as the board menu.

use std::path::Path;

use iced::Task;
use serde_json::json;
use signex_engine::pcb::{BoardItemId, placement};
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::{ReuseRerun, read_blocks_task};
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: impl Into<String>) -> Ran {
    let reason = reason.into();
    (CommandOutcome::Disabled { reason }, Task::none())
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> &'a str {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => text.trim(),
        _ => "",
    }
}

/// The board items `ids` names, comma separated: a designator names its
/// footprint, an id a footprint, track or via of the board.
fn board_items(board: &PcbBoard, ids: &str) -> Result<Vec<BoardItemId>, String> {
    let mut items = Vec::new();
    for token in ids.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        let item = match Uuid::parse_str(token) {
            Ok(id) if board.footprints.iter().any(|fp| fp.uuid == id) => BoardItemId::Footprint(id),
            Ok(id) if board.segments.iter().any(|s| s.uuid == id) => BoardItemId::Segment(id),
            Ok(id) if board.vias.iter().any(|v| v.uuid == id) => BoardItemId::Via(id),
            Ok(_) => {
                return Err(format!(
                    "`{token}` is no footprint, track or via of the board"
                ));
            }
            Err(_) => BoardItemId::Footprint(placement::by_designator(board, token)?),
        };
        if !items.contains(&item) {
            items.push(item);
        }
    }
    if items.is_empty() {
        return Err("`ids` names nothing to save: list designators or item ids".to_string());
    }
    Ok(items)
}

/// The project's blocks of `board`, read first when they are not read yet:
/// then the command `id` runs again, as the caller that sent it, when they
/// land (`Deferred`). `None` when they are read.
fn read_first(app: &mut Signex, board: &Path, id: &str, args: &CommandArgs) -> Option<Ran> {
    let folder = match app.reuse_block_folder(board) {
        Ok(folder) => folder,
        Err(reason) => return Some(refused(reason)),
    };
    if !app.document_state.reuse_blocks.needs_read(&folder) {
        return None;
    }
    let Ok(command) = AppCommandId::new(id) else {
        return Some(refused("the command id is not valid"));
    };
    let edits = &mut app.document_state.agent_edits;
    edits.tickets += 1;
    let ticket = edits.tickets;
    let rerun = ReuseRerun {
        ticket,
        command,
        args: args.clone(),
        board: board.to_path_buf(),
        origin: app.agent.origin.clone(),
        serving: app.agent.serving,
    };
    let task = read_blocks_task(folder, Some(Box::new(rerun)));
    Some((CommandOutcome::Deferred { ticket }, task))
}

/// `save_board_reuse_block document=<board> name=<name> ids=<list>`. The
/// name is checked against the blocks on disk, so they are read first
/// when they are not read yet (`Deferred`).
pub(crate) fn save_board_reuse_block(app: &mut Signex, args: &CommandArgs) -> Ran {
    let board = match app.named_output_board(args) {
        Ok(board) => board,
        Err(reason) => return refused(reason),
    };
    if let Some(waiting) = read_first(app, &board, "save_board_reuse_block", args) {
        return waiting;
    }
    let items = match app.document_state.pcb_board(&board) {
        Some(pcb) => board_items(pcb, text(args, "ids")),
        None => Err("the board is not open".to_string()),
    };
    let items = match items {
        Ok(items) => items,
        Err(reason) => return refused(reason),
    };
    match app.save_reuse_block(&board, text(args, "name"), &items) {
        Ok((saved, task)) => {
            let mut answer = serde_json::to_value(&saved).unwrap_or_else(|_| json!({}));
            answer["document"] = json!(board.display().to_string());
            (CommandOutcome::Answered(answer), task)
        }
        Err(reason) => refused(reason),
    }
}

/// `place_board_reuse_block document=<board> name=<name> designator=<ref>
/// [degrees=<turn>]`. While the project's blocks are not read yet, they are
/// read first and the command runs when they land (`Deferred`).
pub(crate) fn place_board_reuse_block(app: &mut Signex, args: &CommandArgs) -> Ran {
    let board = match app.named_output_board(args) {
        Ok(board) => board,
        Err(reason) => return refused(reason),
    };
    if let Some(waiting) = read_first(app, &board, "place_board_reuse_block", args) {
        return waiting;
    }
    let anchor = match app
        .document_state
        .pcb_board(&board)
        .ok_or_else(|| "the board is not open".to_string())
        .and_then(|pcb| placement::by_designator(pcb, text(args, "designator")))
    {
        Ok(anchor) => anchor,
        Err(reason) => return refused(reason),
    };
    let degrees = match args.operand("degrees") {
        Some(OperandValue::Angle(degrees)) => *degrees,
        _ => 0.0,
    };
    match app.place_reuse_block(&board, text(args, "name"), anchor, degrees) {
        Ok(placed) => {
            let mut answer = serde_json::to_value(&placed).unwrap_or_else(|_| json!({}));
            answer["document"] = json!(board.display().to_string());
            (CommandOutcome::Answered(answer), Task::none())
        }
        Err(reason) => refused(reason),
    }
}
