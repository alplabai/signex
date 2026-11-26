//! The read-only query commands (#2172): `list_documents`, `document_info`,
//! `resolve`, `find_items`, `read_document`, `query_pins`, the checks
//! `check_erc` and `check_drc` (#2199) and `check_requirements` (#2339),
//! answered once their run lands, and
//! the Rules manager reads `rules_list`, `rules_get`, `rules_effective`,
//! `rules_explain`, `rules_conflicts`, `rules_matrix_get` and
//! `rules_preset_diff` (#2398, `app::rules_tab`; a profile preset reads the
//! rule profile library, read off the update loop first), and the board
//! queries `measure_board`, `query_board_nets` and `find_similar_items`
//! (#2403), `board_panel` (#2410, `app::board_panel`), the layout plan's
//! `layout_plan_get` and `layout_plan_status` (#2942, `app::layout_plan`),
//! the Project tab's `project_settings` (#1689-#1693, `app::project_tab`)
//! with the Variant Manager's `compare_variants` and `variant_bom` (#2949),
//! `list_snippets` and `list_templates` (#1283, `app::design_reuse`),
//! `query_net_ties` (#1277), the BOM's `bom_lines` (#1988), the project's
//! differential pairs `list_diff_pairs` (#2952, `app::diff_pair_classes`),
//! `query_footprint_assignments` (#2937, `footprint_assign_operands`),
//! `check_board_against_schematic` (#2462, `app::compare_board`),
//! `board_status` (#2460), `query_rail_budget` (#2943), the BOM and part
//! search queries `query_bom` (#2938) and `search_parts` (#1297, `bom`), and
//! `query_library` (#2944, the rows of an open library), answered once their
//! reads land. Each
//! answers with data through [`CommandOutcome::Answered`], never a bare
//! `Ran`, and reads only the documents (engines, boards, editors, projects,
//! the library): no active tab, window, selection or panel is looked at, and
//! none is changed. What it cannot answer yet it refuses with the reason; it
//! never guesses.
//!
//! Which ids the commands print, and where each kind stores one, is in
//! `docs/DOCUMENT_IDS.md`; the answer shapes are in `docs/COMMANDS.md`.

mod board;
mod board_status;
mod bom;
mod bom_lines;
#[cfg(test)]
mod bom_tests;
mod check_drc;
mod check_erc;
mod check_requirements;
mod check_run;
pub use check_run::CheckFinished;
mod documents;
mod elements;
mod find;
mod library;
mod net_ties;
mod pins;
pub(in crate::app::command) use pins::pin_id;
mod rail_budget;
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
    "query_net_ties",
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
    // #1283: the snippets and templates of the project in view and the
    // library, answered once their folders are read.
    "list_snippets",
    "list_templates",
    "bom_lines",
    // #2949: the Variant Manager's Compare variants and Open in BOM Studio.
    "compare_variants",
    "variant_bom",
    "list_diff_pairs",
    // #2937: every part's footprint (`footprint_assign_operands`).
    "query_footprint_assignments",
    "check_board_against_schematic",
    "board_status",
    "layout_plan_get",
    "layout_plan_status",
    "query_rail_budget",
    "query_bom",
    "search_parts",
    // #2944: the rows of an open library.
    "query_library",
];

/// The queries that run a check (`check_run`): each says nothing in
/// Messages; its report names what it did not read.
const CHECK_IDS: &[&str] = &["check_erc", "check_drc", "check_requirements"];

/// Is `command` one of the checks?
pub(crate) fn is_check(command: &AppCommandId) -> bool {
    CHECK_IDS.contains(&command.as_str())
}

/// Every query command id.
pub(crate) fn query_command_ids() -> impl Iterator<Item = &'static str> {
    QUERY_IDS.iter().copied()
}

/// Is `command` one of the query commands?
pub fn is_query(command: &AppCommandId) -> bool {
    query_command_ids().any(|query| query == command.as_str())
}

/// What running a query did: the answer or the reason it was refused.
pub(super) type Answer = Result<Value, String>;

/// The text of operand `name`, if it was given.
pub(super) use super::operand_str as text_operand;

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
            // Compare Board with Schematic as a query (#2462) and the board
            // counters with it (#2460), answered once the reads land.
            "check_board_against_schematic" => {
                return Some(self.check_board_against_schematic(command, args));
            }
            "board_status" => return Some(self.board_status_query(command, args)),
            // The project's rails against its parts, read off the UI thread.
            "query_rail_budget" => return Some(self.query_rail_budget(command, args)),
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
            // The Project tab's sections (#1689-#1693) and the project's
            // differential pairs (#2952).
            "project_settings" | "compare_variants" | "variant_bom" | "list_diff_pairs" => {
                let answer = match id {
                    "compare_variants" => self.compare_variants(args),
                    "variant_bom" => self.variant_bom(args),
                    "list_diff_pairs" => self.list_diff_pairs(args),
                    _ => self.project_settings(args),
                };
                let outcome = match answer {
                    Ok(value) => CommandOutcome::Answered(value),
                    Err(reason) => CommandOutcome::disabled(reason),
                };
                return Some((outcome, Task::none()));
            }
            // A profile preset may need the profile library read first.
            "rules_preset_diff" => return Some(self.rules_preset_diff(args)),
            "list_snippets" => return Some(self.list_snippets(command, args)),
            "list_templates" => return Some(self.list_templates(command, args)),
            // #2938: answered once the BOM, built off the UI thread, lands.
            "query_bom" => return Some(self.query_bom(command, args)),
            _ => {}
        }
        let answer = match id {
            "list_documents" => self.list_documents(args),
            "document_info" => self.document_info(args),
            "resolve" => self.resolve_id(args),
            "find_items" => self.find(args),
            "query_pins" => self.query_pins(args),
            "query_net_ties" => self.query_net_ties(args),
            "bom_lines" => self.bom_lines(args),
            "measure_board" => self.measure_board(args),
            "query_board_nets" => self.query_board_nets(args),
            "find_similar_items" => self.find_similar_items(args),
            "board_panel" => self.board_panel_query(args),
            "query_footprint_assignments" => self.query_footprint_assignments(args),
            "layout_plan_get" | "layout_plan_status" => self.layout_plan_query(id, args),
            "search_parts" => self.search_parts_query(args),
            "query_library" => self.query_library(args),
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

pub(super) fn refused(reason: String) -> (CommandOutcome, Task<Message>) {
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
mod board_status_tests;
#[cfg(test)]
mod check_drc_base_rules_tests;
#[cfg(test)]
mod check_drc_bodies_tests;
#[cfg(test)]
pub(crate) mod check_drc_tests;
#[cfg(test)]
mod check_erc_tests;
#[cfg(test)]
mod check_requirements_tests;
#[cfg(test)]
mod library_tests;
#[cfg(test)]
mod rail_budget_tests;
