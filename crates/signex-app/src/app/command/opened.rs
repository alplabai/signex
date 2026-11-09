//! `open_document` and `new_document` answer with the document they made
//! (#2172). A headless run goes to completion inside the dispatch: the open or
//! the project creation is a chain of background tasks, run here to their end,
//! and the answer is the document that appeared: its id, path and kind. A GUI
//! caller is unchanged: it never reaches this.

use std::collections::HashSet;
use std::path::PathBuf;

use iced::Task;
use serde_json::json;

use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::{DocumentEntry, DocumentKind};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// The commands that make a document and so answer with it.
pub(crate) fn answers_with_the_new_document(command: &AppCommandId) -> bool {
    matches!(command.as_str(), "open_document" | "new_document")
}

/// What was asked for and what existed before the command ran.
pub(crate) struct OpenSnapshot {
    asked: Option<PathBuf>,
    before: HashSet<Option<PathBuf>>,
}

impl Signex {
    /// Taken before the command is dispatched: the `path` operand, else the
    /// dialog's `path` answer, and the documents that already exist.
    pub(crate) fn open_snapshot(&self, args: &CommandArgs) -> OpenSnapshot {
        let operand = match args.operand("path") {
            Some(OperandValue::Path(path)) => Some(path.clone()),
            _ => None,
        };
        OpenSnapshot {
            asked: operand
                .or_else(|| self.headless_answers().and_then(|a| a.peek("path")))
                .map(PathBuf::from),
            before: self
                .document_entries()
                .into_iter()
                .map(|entry| entry.path)
                .collect(),
        }
    }

    /// What a headless dispatch of `command` answers when it ran `task`: the
    /// task is driven to its end first. The document is the one at the dialog's
    /// `path` answer, else the project that was just created.
    pub(crate) fn settle_and_answer_new_document(
        &mut self,
        command: &AppCommandId,
        snapshot: OpenSnapshot,
        task: Task<Message>,
    ) -> (CommandOutcome, Task<Message>) {
        let OpenSnapshot { asked, before } = snapshot;
        if let Err(error) = crate::task_driver::TaskDriver::new().settle(self, task) {
            return (
                CommandOutcome::disabled(format!("`{}` did not finish: {error}", command.as_str())),
                Task::none(),
            );
        }
        self.reconcile_document_ids();
        let entries = self.document_entries();
        let at_asked = |entry: &&DocumentEntry| {
            asked
                .as_deref()
                .zip(entry.path.as_deref())
                .is_some_and(|(want, got)| {
                    let fold = self.identity_fold_for(got);
                    crate::app::state::scope::path_key_with(want, fold)
                        == crate::app::state::scope::path_key_with(got, fold)
                })
        };
        let created = |entry: &&DocumentEntry| {
            entry.kind == DocumentKind::Project && !before.contains(&entry.path)
        };
        let found = if command.as_str() == "open_document" {
            entries.iter().find(at_asked)
        } else {
            entries.iter().find(created)
        };
        let outcome = match found.and_then(|entry| Some((entry, entry.id?))) {
            Some((entry, id)) => CommandOutcome::Answered(json!({
                "id": id.to_string(),
                "path": entry.path.as_ref().map(|p| p.display().to_string()),
                "kind": entry.kind.as_str(),
            })),
            None => CommandOutcome::disabled(format!(
                "`{}` made no document; Messages says why",
                command.as_str()
            )),
        };
        (outcome, Task::none())
    }
}
