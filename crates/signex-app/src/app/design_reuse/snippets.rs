//! Saving a snippet (#1283): the file is written off the update thread, and
//! the command run with operands is answered when the write lands. The GUI's
//! Save as Snippet names the selection after its designators and runs the
//! same operand handler (`command::translator`).

use iced::Task;
use signex_i18n::{command_ids, ids, t};
use uuid::Uuid;

use super::{Ran, SavedSnippet, store};
use crate::app::agent::WaitingCommand;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue, translator};
use crate::app::{EditMsg, Message, PlacementExtrasMsg, Signex};
use crate::keymap::AppCommandId;

impl Signex {
    /// Write `text` as the snippet `saved` names; `overwrite` replaces one of
    /// that name. Run with operands, the command waits for the write and is
    /// answered when it lands (`Deferred`); from the GUI it answers at once
    /// and Messages says how the write went.
    pub(crate) fn start_snippet_write(
        &mut self,
        args: &CommandArgs,
        saved: SavedSnippet,
        text: String,
        overwrite: bool,
    ) -> Ran {
        if let Err(refusal) = self.refuse_while_project_moves(&saved.file, "save the snippet to") {
            return (CommandOutcome::disabled(refusal.to_string()), Task::none());
        }
        let waiting = if self.agent.from_view {
            None
        } else {
            match AppCommandId::new("save_snippet") {
                Ok(command) => Some(self.wait_for_command(&command, args)),
                Err(reason) => return (CommandOutcome::disabled(reason), Task::none()),
            }
        };
        self.begin_project_write(&saved.file);
        let outcome = match &waiting {
            Some(waiting) => CommandOutcome::Deferred {
                ticket: waiting.ticket,
            },
            None => CommandOutcome::Answered(saved.answer()),
        };
        (outcome, write_snippet_task(saved, text, overwrite, waiting))
    }

    /// A snippet write landed: say it in Messages and answer the command
    /// that waits for it.
    pub(crate) fn snippet_written(
        &mut self,
        saved: SavedSnippet,
        result: Result<(), String>,
        waiting: Option<WaitingCommand>,
    ) -> Task<Message> {
        self.end_project_write(&saved.file);
        let file = saved.file.display().to_string();
        let outcome = match &result {
            Ok(()) => {
                let line = t!(
                    self.ui_state.i18n,
                    ids::SNIPPET_SAVED,
                    "name" => saved.name.as_str(),
                    "file" => file.as_str(),
                    "items" => saved.items.to_string()
                );
                crate::diagnostics::log_info(line.into_string());
                CommandOutcome::Answered(saved.answer())
            }
            Err(why) => {
                let line = t!(
                    self.ui_state.i18n,
                    ids::SNIPPET_NOT_WRITTEN,
                    "name" => saved.name.as_str(),
                    "file" => file.as_str(),
                    "why" => why.as_str()
                );
                crate::diagnostics::log_warning(line.into_string());
                CommandOutcome::disabled(why.clone())
            }
        };
        match waiting {
            Some(waiting) => {
                let document = saved.document.clone();
                self.finish_waiting_command(waiting, &document, |_| outcome)
            }
            None => Task::none(),
        }
    }

    /// `save_snippet` from the palette or a menu: the selection on the sheet
    /// in view, named after its designators, saved to the project by the
    /// command's operand handler with the sheet, the name and the items
    /// filled in.
    pub(crate) fn save_selection_as_snippet(&mut self) -> Task<Message> {
        let what = t!(self.ui_state.i18n, command_ids::CMD_MENU_SAVE_SNIPPET).into_string();
        let sheet = match self.document_state.active_path.clone() {
            Some(sheet) if self.has_active_schematic() => sheet,
            _ => {
                self.refuse_snippet(&what, "open a schematic sheet first");
                return Task::none();
            }
        };
        let mut ids: Vec<Uuid> = Vec::new();
        for item in &self.interaction_state.active_canvas().selected {
            if !ids.contains(&item.uuid) {
                ids.push(item.uuid);
            }
        }
        if ids.is_empty() {
            self.refuse_snippet(&what, "select the circuit to save first");
            return Task::none();
        }
        let name = self.snippet_name_for(&sheet, &ids);
        let operands = vec![
            ("name", OperandValue::Text(name)),
            ("ids", OperandValue::Targets(ids)),
        ];
        let (outcome, task) = translator::run(self, "save_snippet", &sheet, operands);
        if let CommandOutcome::Disabled { reason } = outcome {
            let why = reason.render(&self.ui_state.i18n);
            self.refuse_snippet(&what, &why);
        }
        task
    }

    /// The designators of the parts among `ids` on `sheet`, sorted and
    /// joined by `-`: the name a snippet saved from the menu takes, `snippet`
    /// when it holds no part.
    fn snippet_name_for(&self, sheet: &std::path::Path, ids: &[Uuid]) -> String {
        let Some(engine) = self.document_state.engines.get(sheet) else {
            return "snippet".to_string();
        };
        let mut names: Vec<&str> = engine
            .document()
            .symbols
            .iter()
            .filter(|symbol| ids.contains(&symbol.uuid))
            .map(|symbol| symbol.reference.trim())
            .filter(|name| !name.is_empty() && super::checked_name(name, "snippet").is_ok())
            .collect();
        names.sort_unstable();
        names.dedup();
        let name: String = names
            .join("-")
            .chars()
            .take(signex_types::reuse::MAX_NAME_CHARS)
            .collect();
        match super::checked_name(&name, "snippet") {
            Ok(name) => name,
            Err(_) => "snippet".to_string(),
        }
    }

    /// Say in Messages that `what` did nothing, and why.
    fn refuse_snippet(&self, what: &str, why: &str) {
        let line = t!(
            self.ui_state.i18n,
            ids::SNIPPET_REFUSED,
            "what" => what,
            "why" => why
        );
        crate::diagnostics::log_warning(line.into_string());
    }
}

/// Write a snippet's file off the update thread.
fn write_snippet_task(
    saved: SavedSnippet,
    text: String,
    overwrite: bool,
    waiting: Option<WaitingCommand>,
) -> Task<Message> {
    let file = saved.file.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || store::write_snippet(&file, &text, overwrite))
                .await
                .unwrap_or_else(|error| Err(format!("the write did not finish: {error}")))
        },
        move |result| {
            Message::Edit(EditMsg::Extras(PlacementExtrasMsg::SnippetWritten {
                saved: saved.clone(),
                result,
                waiting: waiting.clone(),
            }))
        },
    )
}
