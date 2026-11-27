//! `query_part_heights document=<board>` (#2939): each part's designator,
//! side and height, and where the height came from: the model attached to
//! its library footprint, that footprint's 3D Body, or the Height stored on
//! the part. The models are read off the UI thread, as a DRC run reads
//! them, and the query is answered when they land. Changes nothing. Shape:
//! `docs/CLI.md`, "Part heights".

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use pcb_geom::{Side, nm_to_mm};
use serde_json::{Value, json};
use signex_drc::{BodyModels, part_body};
use signex_i18n::{Translatable, ids};

use super::document_operands::named;
use super::operand_handlers::refused;
use super::{CommandArgs, CommandOutcome};
use crate::app::agent::WaitingCommand;
use crate::app::document::DocumentKey;
use crate::app::document_ids::DocumentKind;
use crate::app::{Message, PcbMsg, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

/// The models of a board's parts, read for a waiting `query_part_heights`.
#[derive(Debug, Clone, PartialEq)]
pub struct PartHeightsRead {
    pub(crate) board: PathBuf,
    pub(crate) models: Arc<BodyModels>,
    /// The models that could not be read, in words.
    pub(crate) unread: Arc<Vec<String>>,
    pub(crate) waiting: WaitingCommand,
}

/// `query_part_heights document=<board>`: read the models of the open
/// board's parts, then answer.
pub(crate) fn query_part_heights(app: &mut Signex, args: &CommandArgs) -> Ran {
    let entry = match named(app, args) {
        Ok(entry) => entry,
        Err(reason) => return refused(reason),
    };
    let open = match entry.key {
        Some(DocumentKey::Pcb(path)) => app
            .document_state
            .pcb_board(&path)
            .cloned()
            .map(|board| (path, board)),
        _ => None,
    };
    let Some((path, board)) = open else {
        let id = if entry.kind == DocumentKind::Pcb {
            ids::PART_HEIGHTS_NOT_OPEN
        } else {
            ids::PART_HEIGHTS_NOT_A_BOARD
        };
        let reason = Translatable::new(id).arg("name", entry.name);
        return (CommandOutcome::Disabled { reason }, Task::none());
    };
    let Ok(command) = AppCommandId::new("query_part_heights") else {
        return refused("the command id is not valid");
    };
    let libraries = app.open_library_roots();
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    let task = Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                crate::app::handlers::pcb_drc_bodies::body_models(&board, &libraries)
            })
            .await
            .unwrap_or_else(|e| {
                (
                    BodyModels::new(),
                    vec![format!("the model read failed: {e}")],
                )
            })
        },
        move |(models, unread)| {
            Message::Pcb(PcbMsg::PartHeightsRead(Box::new(PartHeightsRead {
                board: path.clone(),
                models: Arc::new(models),
                unread: Arc::new(unread),
                waiting: waiting.clone(),
            })))
        },
    );
    (CommandOutcome::Deferred { ticket }, task)
}

impl Signex {
    /// The models a waiting `query_part_heights` asked for landed: answer
    /// it from the board as it is now.
    pub(crate) fn handle_part_heights_read(&mut self, read: PartHeightsRead) -> Task<Message> {
        let PartHeightsRead {
            board,
            models,
            unread,
            waiting,
        } = read;
        let target = board.clone();
        self.finish_waiting_command(waiting, &target, |app| {
            match app.document_state.pcb_board(&board) {
                Some(pcb) => {
                    CommandOutcome::Answered(part_heights_json(&board, pcb, &models, &unread))
                }
                None => CommandOutcome::Disabled {
                    reason: Translatable::new(ids::PART_HEIGHTS_NOT_OPEN)
                        .arg("name", board.display().to_string()),
                },
            }
        })
    }
}

/// The answer: one entry per part, in board order, and the models that
/// could not be read.
pub(crate) fn part_heights_json(
    path: &Path,
    board: &signex_types::pcb::PcbBoard,
    models: &BodyModels,
    unread: &[String],
) -> Value {
    let parts: Vec<Value> = board
        .footprints
        .iter()
        .map(|footprint| {
            let side = match Side::of_layer(&footprint.layer) {
                Side::Top => "top",
                Side::Bottom => "bottom",
            };
            let (height, bottom, source, problem) = match part_body(footprint, models) {
                Ok(Some(body)) => (
                    json!(nm_to_mm(body.top_nm)),
                    json!(nm_to_mm(body.bottom_nm)),
                    body.source.as_str(),
                    Value::Null,
                ),
                Ok(None) => (Value::Null, Value::Null, "none", Value::Null),
                Err(error) => (Value::Null, Value::Null, "none", json!(error.to_string())),
            };
            json!({
                "id": footprint.uuid.to_string(),
                "designator": footprint.reference,
                "side": side,
                "height_mm": height,
                "bottom_mm": bottom,
                "source": source,
                "problem": problem,
            })
        })
        .collect();
    json!({
        "document": path.display().to_string(),
        "parts": parts,
        "unread": unread,
    })
}
