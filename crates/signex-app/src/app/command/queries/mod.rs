//! The read-only query commands (#2172): `list_documents`, `document_info`,
//! `resolve`, `find` and `read_document`. Each answers with data through
//! [`CommandOutcome::Answered`], never a bare `Ran`, and reads only the
//! documents (engines, boards, editors, projects, the library): no active
//! tab, window, selection or panel is looked at, and none is changed. What it
//! cannot answer yet it refuses with the reason; it never guesses.
//!
//! Which ids the commands print, and where each kind stores one, is in
//! `docs/DOCUMENT_IDS.md`; the answer shapes are in `docs/COMMANDS.md`.

mod documents;
mod elements;
mod find;
mod read;
mod sheet_view;

use iced::Task;
use serde_json::Value;

use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// The ids of the query commands. `find` answers only when it is given a
/// filter: without one it is the Find dialog (Ctrl+F), as before.
const QUERY_IDS: &[&str] = &[
    "list_documents",
    "document_info",
    "resolve",
    "find",
    "read_document",
];

/// Every query command id.
pub(crate) fn query_command_ids() -> impl Iterator<Item = &'static str> {
    QUERY_IDS.iter().copied()
}

/// Is `command` a query a person can run from the palette: one that needs no
/// operand. The rest name a document or an id, which only a caller with
/// operands can give.
pub(crate) fn answers_without_operands(command: &AppCommandId) -> bool {
    command.as_str() == "list_documents"
}

/// What running a query did: the answer or the reason it was refused.
pub(super) type Answer = Result<Value, String>;

/// The text of operand `name`, if it was given.
pub(super) fn text_operand<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text),
        _ => None,
    }
}

/// The `bool` operand `name`, if it was given.
pub(super) fn bool_operand(args: &CommandArgs, name: &str) -> Option<bool> {
    match args.operand(name) {
        Some(OperandValue::Bool(value)) => Some(*value),
        _ => None,
    }
}

impl Signex {
    /// Run `command` if it is a query. `None` when it is not one (or is
    /// `find` with no filter, the Find dialog), so the caller runs it as it
    /// always did. A query that is missing a required operand, or names a
    /// document that does not exist, is refused with the reason.
    pub(crate) fn run_query(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> Option<(CommandOutcome, Task<Message>)> {
        let id = command.as_str();
        if !query_command_ids().any(|query| query == id) || (id == "find" && !args.has_operands()) {
            return None;
        }
        if let Some(missing) = missing_required(command, args) {
            return Some(refused(format!("`{id}` needs the operand `{missing}`")));
        }
        // Ids are made unique before anything is listed or addressed.
        self.reconcile_document_ids();
        let answer = match id {
            "list_documents" => self.list_documents(args),
            "document_info" => self.document_info(args),
            "resolve" => self.resolve_id(args),
            "find" => self.find(args),
            _ => self.read_document(args),
        };
        // The heads of referenced files that were never read are read now,
        // so the next answer names them.
        let task = self.read_all_missing_document_headers();
        Some(match answer {
            Ok(value) => (CommandOutcome::Answered(value), task),
            Err(reason) => (CommandOutcome::disabled(reason), task),
        })
    }
}

fn refused(reason: String) -> (CommandOutcome, Task<Message>) {
    (CommandOutcome::disabled(reason), Task::none())
}

/// The first required operand `command` was not given.
fn missing_required(command: &AppCommandId, args: &CommandArgs) -> Option<&'static str> {
    crate::keymap::metadata_for(command)?
        .operands
        .iter()
        .find(|op| op.required && args.operand(op.name).is_none())
        .map(|op| op.name)
}

#[cfg(test)]
pub(in crate::app) mod tests;
