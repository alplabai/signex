//! The drawing order (#1583) and `delete_selection` from their operands
//! (#2326 slice 6.P2b): the items `ids` names, by uuid, on the `document`
//! named, never the person's selection. A menu row, key or the palette fills
//! both in from the document in view and its selection and runs these
//! handlers (`command::translator`), so each id has one behaviour on every
//! surface. Delete on a footprint editor tab stays the editor's own GUI
//! route until slice S-L (`docs/AGENT_SOCKET.md`).

use std::path::Path;

use iced::Task;
use serde_json::json;
use signex_engine::pcb::BoardItemId;
use signex_engine::pcb::placement;
use signex_engine::{Command, DrawingArrange};
use signex_i18n::{Translatable, ids};
use uuid::Uuid;

use super::document_operands::{named, named_sheet};
use super::operand_handlers::refused;
use super::{CommandArgs, CommandOutcome};
use crate::app::document::DocumentKey;
use crate::app::mutation_gateway::MutationOutcome;
use crate::app::{Message, Signex};

type Ran = (CommandOutcome, Task<Message>);

/// `bring_drawings_forward document=.. ids=<drawings>`.
pub(super) fn bring_drawings_forward(app: &mut Signex, args: &CommandArgs) -> Ran {
    arrange(app, args, DrawingArrange::Forward)
}

/// `bring_drawings_to_front document=.. ids=<drawings>`.
pub(super) fn bring_drawings_to_front(app: &mut Signex, args: &CommandArgs) -> Ran {
    arrange(app, args, DrawingArrange::ToFront)
}

/// `send_drawings_backward document=.. ids=<drawings>`.
pub(super) fn send_drawings_backward(app: &mut Signex, args: &CommandArgs) -> Ran {
    arrange(app, args, DrawingArrange::Backward)
}

/// `send_drawings_to_back document=.. ids=<drawings>`.
pub(super) fn send_drawings_to_back(app: &mut Signex, args: &CommandArgs) -> Ran {
    arrange(app, args, DrawingArrange::ToBack)
}

/// `delete_selection document=.. ids=<items>`: the items of an open sheet
/// or board, as one undo step.
pub(super) fn delete_selection(app: &mut Signex, args: &CommandArgs) -> Ran {
    let entry = match named(app, args) {
        Ok(entry) => entry,
        Err(reason) => return refused(reason),
    };
    let targets = targets(args);
    match entry.key {
        Some(DocumentKey::Schematic(path)) if app.document_state.engines.contains_key(&path) => {
            delete_on_sheet(app, &path, targets)
        }
        Some(DocumentKey::Pcb(path)) if app.document_state.pcb_board(&path).is_some() => {
            delete_on_board(app, &path, targets)
        }
        _ => disabled(Translatable::new(ids::DELETE_DOCUMENT_NOT_OPEN).arg("name", entry.name)),
    }
}

/// Move the drawings `ids` names on the named sheet per `how`, as one undo
/// step; a uuid that is no drawing of the sheet refuses the whole command.
fn arrange(app: &mut Signex, args: &CommandArgs, how: DrawingArrange) -> Ran {
    let path = match named_sheet(app, args, "the drawing order") {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let targets = targets(args);
    let unknown = app.document_state.engines.get(&path).and_then(|engine| {
        let drawings = &engine.document().drawings;
        (targets.iter().copied()).find(|id| !drawings.iter().any(|d| d.uuid() == *id))
    });
    if let Some(id) = unknown {
        return unknown_on_sheet(id);
    }
    let command = Command::ArrangeDrawings {
        ids: targets.clone(),
        arrange: how,
    };
    // The menu row's flags: the selection overlay is kept.
    let ran = on_sheet(app, &path, |app| {
        app.apply_engine_command_reporting(command, false, true)
    });
    answered(&path, &targets, ran)
}

/// Delete the objects `targets` names on the sheet at `path`; a uuid that
/// names none refuses the whole command.
fn delete_on_sheet(app: &mut Signex, path: &Path, targets: Vec<Uuid>) -> Ran {
    let Some(engine) = app.document_state.engines.get(path) else {
        return disabled(Translatable::new(ids::NOTICE_NOT_AVAILABLE));
    };
    let mut items = Vec::with_capacity(targets.len());
    for id in &targets {
        match engine.item_by_uuid(*id) {
            Some(item) => items.push(item),
            None => return unknown_on_sheet(*id),
        }
    }
    let command = Command::DeleteSelection { items };
    let ran = on_sheet(app, path, |app| {
        app.apply_engine_command_reporting(command, true, true)
    });
    answered(path, &targets, ran)
}

/// Delete the items `targets` names on the board at `path`. The refusals
/// are the placement's own (a locked or read-only footprint), in its words.
fn delete_on_board(app: &mut Signex, path: &Path, targets: Vec<Uuid>) -> Ran {
    let Some(board) = app.document_state.pcb_board(path) else {
        return disabled(Translatable::new(ids::NOTICE_NOT_AVAILABLE));
    };
    let mut items = Vec::with_capacity(targets.len());
    for id in &targets {
        match BoardItemId::on_board(board, *id) {
            Some(item) => items.push(item),
            None => {
                let reason = Translatable::new(ids::BOARD_TARGET_UNKNOWN).arg("id", id.to_string());
                return disabled(reason);
            }
        }
    }
    let commands = match placement::delete_commands(board, items) {
        Ok(commands) => commands,
        Err(refusal) => return refused(refusal.to_string()),
    };
    let patches = match app.execute_board_commands(path, commands, None) {
        Ok(patches) => patches,
        Err(error) => return refused(error.to_string()),
    };
    if app.active_tab_path().as_deref() == Some(path) {
        app.refresh_pcb_highlight();
    }
    changed(path, &targets, !patches.is_empty())
}

/// Run `edit` on the sheet at `path`. The sheet edits already go to (the
/// active one, or an agent's target) is edited as it is, so a person's run
/// refreshes the canvas as the menu row always has; another is made the
/// edit target for the run.
fn on_sheet<T>(app: &mut Signex, path: &Path, edit: impl FnOnce(&mut Signex) -> T) -> T {
    if app.document_state.edit_path() == Some(path) {
        edit(app)
    } else {
        app.on_edit_target(path, edit)
    }
}

/// The uuids `ids` names; none when it was not given.
fn targets(args: &CommandArgs) -> Vec<Uuid> {
    args.targets("ids")
        .map(<[Uuid]>::to_vec)
        .unwrap_or_default()
}

fn answered(path: &Path, targets: &[Uuid], ran: (MutationOutcome, Option<String>)) -> Ran {
    match ran {
        (MutationOutcome::Failed, Some(reason)) => refused(reason),
        (MutationOutcome::Failed, None) => disabled(Translatable::new(ids::NOTICE_NOT_AVAILABLE)),
        (outcome, _) => changed(path, targets, outcome == MutationOutcome::Changed),
    }
}

/// The answer: the document, the items named and whether it changed.
fn changed(path: &Path, targets: &[Uuid], changed: bool) -> Ran {
    let answer = json!({ "document": path, "ids": targets, "changed": changed });
    (CommandOutcome::Answered(answer), Task::none())
}

fn unknown_on_sheet(id: Uuid) -> Ran {
    disabled(Translatable::new(ids::SHEET_TARGET_UNKNOWN).arg("id", id.to_string()))
}

fn disabled(reason: Translatable) -> Ran {
    (CommandOutcome::Disabled { reason }, Task::none())
}
