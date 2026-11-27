//! The read-only query commands (#2172): `list_documents`, `document_info`,
//! `resolve`, `find_items`, `read_document`, `query_pins`, the checks
//! `check_erc` and `check_drc` (#2199) and `check_requirements` (#2339),
//! answered once their run lands, and
//! the Rules manager reads `rules_list`, `rules_get`, `rules_effective`,
//! `rules_explain`, `rules_conflicts`, `rules_matrix_get` and
//! `rules_preset_diff` (#2398, `app::rules_tab`; a profile preset reads the
//! rule profile library, read off the update loop first), the notebook's
//! `notebook_measurements`, `notebook_timeline` and `notebook_attachments`
//! (mockup row 199, `app::notebook_measurements`), and the board
//! queries `measure_board`, `query_board_nets` and `find_similar_items`
//! (#2403), `board_panel` (#2410, `app::board_panel`), and the Project tab's
//! `project_settings` (#1689-#1693, `app::project_tab`), and
//! `list_annotations` (mockup row 198), answered once the project's notebook
//! files are read. Each answers with data through [`CommandOutcome::Answered`], never a bare
//! `Ran`, and reads only the documents (engines, boards, editors, projects,
//! the library): no active tab, window, selection or panel is looked at, and
//! none is changed. What it cannot answer yet it refuses with the reason; it
//! never guesses.
//!
//! Which ids the commands print, and where each kind stores one, is in
//! `docs/DOCUMENT_IDS.md`; the answer shapes are in `docs/COMMANDS.md`.

mod annotations;
pub use annotations::AnnotationsListed;
mod board;
mod check_drc;
mod check_erc;
mod check_requirements;
mod check_run;
pub use check_run::CheckFinished;
mod documents;
mod elements;
mod find;
mod images;
mod pins;
pub(in crate::app::command) use pins::pin_id;
mod read;
mod sheet_view;
mod transient;

use iced::Task;
use serde_json::Value;

use super::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// The ids of the query commands. `find` stays the Find dialog (Ctrl+F);
/// `find_items` is the query.
const QUERY_IDS: &[&str] = &[
    "list_documents",
    "document_info",
    "resolve",
    "find_items",
    "read_document",
    "query_pins",
    "check_erc",
    "check_drc",
    "check_requirements",
    "rules_list",
    "rules_get",
    "rules_effective",
    "rules_explain",
    "rules_conflicts",
    "measure_board",
    "query_board_nets",
    "find_similar_items",
    "rules_matrix_get",
    "rules_preset_diff",
    "board_panel",
    "project_settings",
    "list_annotations",
    // #1116: the project's image assets, read off the UI thread.
    "list_image_assets",
];

/// The queries that run a check (`check_run`): each says nothing in
/// Messages; its report names what it did not read.
const CHECK_IDS: &[&str] = &["check_erc", "check_drc", "check_requirements"];

/// Is `command` one of the checks?
pub(crate) fn is_check(command: &AppCommandId) -> bool {
    CHECK_IDS.contains(&command.as_str())
}

/// Every query command id: these and the notebook's measurement reads.
pub(crate) fn query_command_ids() -> impl Iterator<Item = &'static str> {
    QUERY_IDS
        .iter()
        .copied()
        .chain(crate::app::notebook_measurements::QUERY_IDS)
}

/// Is `command` one of the query commands?
pub fn is_query(command: &AppCommandId) -> bool {
    query_command_ids().any(|query| query == command.as_str())
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
    /// Run `command` if it is a query. `None` when it is not one, so the
    /// caller runs it as it
    /// always did. A query that is missing a required operand, or names a
    /// document that does not exist, is refused with the reason.
    pub(crate) fn run_query(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> Option<(CommandOutcome, Task<Message>)> {
        let id = command.as_str();
        if !query_command_ids().any(|query| query == id) {
            return None;
        }
        if let Some(missing) = missing_required(command, args) {
            return Some(refused(format!("`{id}` needs the operand `{missing}`")));
        }
        // Ids are made unique before anything is listed or addressed.
        self.reconcile_document_ids();
        match id {
            "check_erc" => return Some(self.check_erc(command, args)),
            "check_drc" => return Some(self.check_drc(command, args)),
            "check_requirements" => return Some(self.check_requirements(command, args)),
            // The project's notebook files are read first (mockup row 198).
            "list_annotations" => return Some(self.list_annotations(command, args)),
            // The notebook's measurement reads wait for the notebook's files.
            "notebook_measurements" | "notebook_timeline" | "notebook_attachments" => {
                return Some(crate::app::notebook_measurements::query(
                    self, command, args,
                ));
            }
            // The Rules manager's reads (#2398).
            "rules_list" | "rules_get" | "rules_effective" | "rules_explain"
            | "rules_conflicts" => {
                let outcome = match self.rules_query(id, args) {
                    Ok(value) => CommandOutcome::Answered(value),
                    Err(reason) => CommandOutcome::disabled(reason),
                };
                return Some((outcome, Task::none()));
            }
            "rules_matrix_get" => {
                let outcome = match self.rules_matrix_get(args) {
                    Ok(value) => CommandOutcome::Answered(value),
                    Err(reason) => CommandOutcome::disabled(reason),
                };
                return Some((outcome, Task::none()));
            }
            // The Project tab's sections (#1689-#1693).
            "project_settings" => {
                let outcome = match self.project_settings(args) {
                    Ok(value) => CommandOutcome::Answered(value),
                    Err(reason) => CommandOutcome::disabled(reason),
                };
                return Some((outcome, Task::none()));
            }
            // A profile preset may need the profile library read first.
            "rules_preset_diff" => return Some(self.rules_preset_diff(args)),
            // #1116: answered once the scan of the folder lands.
            "list_image_assets" => {
                return Some(match self.image_assets_sheet(args) {
                    Ok(sheet) => self.list_image_assets(command, args, sheet),
                    Err(reason) => refused(reason),
                });
            }
            _ => {}
        }
        let answer = match id {
            "list_documents" => self.list_documents(args),
            "document_info" => self.document_info(args),
            "resolve" => self.resolve_id(args),
            "find_items" => self.find(args),
            "query_pins" => self.query_pins(args),
            "measure_board" => self.measure_board(args),
            "query_board_nets" => self.query_board_nets(args),
            "find_similar_items" => self.find_similar_items(args),
            "board_panel" => self.board_panel_query(args),
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

#[cfg(test)]
pub(in crate::app) mod pins_tests;

#[cfg(test)]
mod annotations_tests;
#[cfg(test)]
mod check_drc_base_rules_tests;
#[cfg(test)]
pub(crate) mod check_drc_tests;
#[cfg(test)]
mod check_erc_tests;
#[cfg(test)]
mod check_requirements_tests;
