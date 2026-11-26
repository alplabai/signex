//! Snippets and templates (#1283): a selection saved as a snippet of the
//! project or of the user's library and placed again as a design block, and
//! the sheet and project templates a new sheet or project starts from.
//! `signex_types::reuse` says where each is kept and what a template gives;
//! this slice resolves the folders, reads and writes them off the update
//! thread and answers the commands. Rules: `docs/SNIPPETS_AND_TEMPLATES.md`.

mod snippets;
mod store;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use serde_json::{Value, json};
use signex_types::reuse::{NameRefusal, ReuseKind, ReuseListing, ReuseScope};

pub(crate) use store::{
    ProjectTemplate, apply_project_template_to, read_project_template, sheet_from_template,
};

use crate::app::agent::WaitingCommand;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{EditMsg, Message, PlacementExtrasMsg, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

/// The user's library folder, under the config folder: where snippets and
/// templates shared by every project are kept.
const LIBRARY_FOLDER: &str = "library";

/// A snippet saved, as `save_snippet` answers it.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedSnippet {
    pub name: String,
    pub scope: ReuseScope,
    pub file: PathBuf,
    /// How many objects it holds.
    pub items: usize,
    /// The sheet it was saved from.
    pub document: PathBuf,
}

impl SavedSnippet {
    pub(crate) fn answer(&self) -> Value {
        json!({
            "name": self.name,
            "scope": self.scope.token(),
            "file": self.file,
            "items": self.items,
            "document": self.document,
        })
    }
}

/// Which command a listing answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListAnswer {
    Snippets,
    Templates,
}

/// The user's library folder, `None` when the platform has no config folder.
pub(crate) fn user_library_root() -> Option<PathBuf> {
    Some(crate::config_root::config_root()?.join(LIBRARY_FOLDER))
}

/// `name` trimmed, or why it cannot name a `what` (a snippet, a template).
pub(crate) fn checked_name(name: &str, what: &str) -> Result<String, String> {
    signex_types::reuse::checked_name(name).map_err(|refusal| {
        let name = name.trim();
        match refusal {
            NameRefusal::Empty => format!("a {what} needs a name"),
            NameRefusal::TooLong => format!(
                "a {what} name is at most {} characters",
                signex_types::reuse::MAX_NAME_CHARS
            ),
            NameRefusal::Characters => format!(
                "`{name}` cannot name a {what}: use letters, digits, spaces, `-`, `_` and `.` inside"
            ),
            NameRefusal::Reserved => format!("`{name}` is a name the system keeps for itself"),
        }
    })
}

/// The `scope` text as a scope: `project` when left out.
pub(crate) fn scope_of(token: Option<&str>) -> Result<ReuseScope, String> {
    match token {
        None => Ok(ReuseScope::Project),
        Some(token) => ReuseScope::from_token(token)
            .ok_or_else(|| format!("`scope` is `project` or `library`, not `{token}`")),
    }
}

impl Signex {
    /// The folder `scope` keeps its entries under: the folder of the project
    /// of `sheet` (else the project in view), or the user's library folder.
    pub(crate) fn reuse_root(
        &self,
        scope: ReuseScope,
        sheet: Option<&Path>,
    ) -> Result<PathBuf, String> {
        match scope {
            ReuseScope::Library => user_library_root()
                .ok_or_else(|| "this machine has no user folder for a library".to_string()),
            ReuseScope::Project => {
                let docs = &self.document_state;
                sheet
                    .and_then(|sheet| docs.document_project(sheet))
                    .or_else(|| self.project_in_view().and_then(|id| docs.project_by_id(id)))
                    .map(|project| project.dir().to_path_buf())
                    .ok_or_else(|| {
                        "no project is open; keep it in the `library` scope instead".to_string()
                    })
            }
        }
    }

    /// The folders a listing reads: the project in view's, when one is
    /// open, and the user's library.
    fn reuse_roots_in_view(&self) -> Vec<(ReuseScope, PathBuf)> {
        [ReuseScope::Project, ReuseScope::Library]
            .into_iter()
            .filter_map(|scope| self.reuse_root(scope, None).ok().map(|root| (scope, root)))
            .collect()
    }

    /// List the `kinds` of the folders in view off the update thread; the
    /// command `id` is answered when the listing lands (`Deferred`).
    pub(crate) fn start_reuse_listing(
        &mut self,
        id: &str,
        args: &CommandArgs,
        kinds: Vec<ReuseKind>,
        answer: ListAnswer,
    ) -> Ran {
        let Ok(command) = AppCommandId::new(id) else {
            return (
                CommandOutcome::disabled("the command id is not valid"),
                Task::none(),
            );
        };
        let roots = self.reuse_roots_in_view();
        let waiting = self.wait_for_command(&command, args);
        let ticket = waiting.ticket;
        let task = Task::perform(
            async move {
                tokio::task::spawn_blocking(move || signex_types::reuse::list(&roots, &kinds))
                    .await
                    .unwrap_or_else(|error| ReuseListing {
                        unreadable: vec![format!("the listing did not finish: {error}")],
                        ..ReuseListing::default()
                    })
            },
            move |listing| {
                Message::Edit(EditMsg::Extras(PlacementExtrasMsg::ReuseListed {
                    waiting: waiting.clone(),
                    answer,
                    listing: Arc::new(listing),
                }))
            },
        );
        (CommandOutcome::Deferred { ticket }, task)
    }

    /// A listing landed: answer the command that waits for it.
    pub(crate) fn reuse_listed(
        &mut self,
        waiting: WaitingCommand,
        answer: ListAnswer,
        listing: &ReuseListing,
    ) -> Task<Message> {
        let folders = self.reuse_roots_in_view();
        let value = listing_answer(answer, listing, &folders);
        self.finish_waiting_command(waiting, Path::new(""), |_| CommandOutcome::Answered(value))
    }
}

/// The answer to `list_snippets` or `list_templates`: the entries, what
/// could not be read and the folders read (`null` for a scope with none).
pub(crate) fn listing_answer(
    answer: ListAnswer,
    listing: &ReuseListing,
    folders: &[(ReuseScope, PathBuf)],
) -> Value {
    let folder = |scope: ReuseScope| {
        folders
            .iter()
            .find(|(s, _)| *s == scope)
            .map(|(_, root)| root.clone())
    };
    let folders = json!({
        "project": folder(ReuseScope::Project),
        "library": folder(ReuseScope::Library),
    });
    match answer {
        ListAnswer::Snippets => {
            let snippets: Vec<Value> = listing
                .of_kind(ReuseKind::Snippet)
                .map(|entry| {
                    json!({
                        "name": entry.name,
                        "scope": entry.scope.token(),
                        "file": entry.file,
                    })
                })
                .collect();
            json!({ "snippets": snippets, "unreadable": listing.unreadable, "folders": folders })
        }
        ListAnswer::Templates => {
            let templates: Vec<Value> = listing
                .entries
                .iter()
                .filter_map(|entry| {
                    let kind = entry.kind.template_token()?;
                    Some(json!({
                        "name": entry.name,
                        "kind": kind,
                        "scope": entry.scope.token(),
                        "file": entry.file,
                    }))
                })
                .collect();
            json!({ "templates": templates, "unreadable": listing.unreadable, "folders": folders })
        }
    }
}
