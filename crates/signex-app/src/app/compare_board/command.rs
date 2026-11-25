//! The command-line forms of Compare Board with Schematic (#2462):
//! `check_board_against_schematic document=` answers every difference as
//! JSON and changes nothing; `apply_board_schematic_differences document=
//! [to_board=<ids>] [to_schematic=<ids>] [expect_revision=<n>]` applies the
//! differences it names through the dialog's Apply. Both name the project
//! by its board, one of its sheets or its file; the board must be open.
//! Shapes: `docs/CLI.md`, "Board against schematic".

use std::path::PathBuf;

use iced::Task;
use signex_engine::pcb::eco::Direction;
use signex_i18n::{MessageId, ids};

use super::job::JobMode;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::DocumentKind;
use crate::app::project_sheets::project_root_sheet_path;
use crate::app::state::ProjectId;
use crate::app::update_pcb::file_name;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

const APPLY: &str = "apply_board_schematic_differences";

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

/// A refusal in the dialog's words; replies stay en-US whatever the UI
/// language.
fn en_us(id: MessageId) -> String {
    signex_i18n::I18n::en_us().message_plain(id, &[])
}

fn text_operand<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text),
        _ => None,
    }
}

/// The ids of a comma separated list, trimmed, empty ones left out.
fn ids_of(text: Option<&str>) -> impl Iterator<Item = String> + '_ {
    text.into_iter()
        .flat_map(|text| text.split(','))
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

impl Signex {
    /// `check_board_against_schematic document=`: the report once the reads
    /// land.
    pub(crate) fn check_board_against_schematic(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> Ran {
        match self.compare_target(command.as_str(), args) {
            Ok(target) => self.start_compare_job(command, args, target, JobMode::Check),
            Err(why) => refused(why),
        }
    }

    /// The project `args` names (by its file, a sheet or its board) and its
    /// open board, or why there is nothing to compare.
    pub(crate) fn compare_target(
        &self,
        id: &str,
        args: &CommandArgs,
    ) -> Result<(ProjectId, PathBuf), String> {
        let Some(OperandValue::Document(named)) = args.operand("document") else {
            return Err(format!("`{id}` needs the operand `document`"));
        };
        let entry = self.resolve_document_target(named)?;
        let docs = &self.document_state;
        // A board names the project that lists it as its `pcb_file`, as
        // `check_drc` finds it; any other document names its own project.
        let (project, board) = match (entry.kind, entry.path.as_ref()) {
            (DocumentKind::Pcb, Some(path)) => {
                let named = entry.project.as_ref().map(|project| project.path.as_path());
                let project = docs.board_project(path, named);
                (project, project.map(|_| path.clone()))
            }
            (kind, path) => {
                let project_path = match kind {
                    DocumentKind::Project => path.cloned(),
                    _ => entry.project.as_ref().map(|project| project.path.clone()),
                };
                let project = project_path
                    .and_then(|path| docs.projects.iter().find(|project| project.path == path));
                (project, project.and_then(Self::project_board))
            }
        };
        let Some(project) = project else {
            return Err(format!(
                "`{id}` compares a project's board with its schematic, and {} is not part of an open project",
                entry.name
            ));
        };
        let Some(board) = board else {
            return Err(en_us(ids::COMPARE_BOARD_NO_BOARD));
        };
        if project_root_sheet_path(project).is_none() {
            return Err(en_us(ids::COMPARE_BOARD_NO_SCHEMATIC));
        }
        if docs.board_engine(&board).is_none() {
            return Err(format!(
                "`{id}`: the board {} is not open; open it first (`open_document`)",
                file_name(&board)
            ));
        }
        Ok((project.id, board))
    }
}

/// `apply_board_schematic_differences`: the differences `to_board` and
/// `to_schematic` name take that direction, every other one is skipped,
/// and the dialog's Apply runs: one board undo step and one schematic undo
/// group. Refused, with nothing applied, when an id is unknown or named
/// twice, a direction is blocked, or the board is not at `expect_revision`.
pub(crate) fn apply_board_schematic_differences(app: &mut Signex, args: &CommandArgs) -> Ran {
    let command = match AppCommandId::new(APPLY) {
        Ok(command) => command,
        Err(reason) => return refused(reason),
    };
    let picks: Vec<(String, Direction)> = ids_of(text_operand(args, "to_board"))
        .map(|id| (id, Direction::UpdatePcb))
        .chain(
            ids_of(text_operand(args, "to_schematic")).map(|id| (id, Direction::UpdateSchematic)),
        )
        .collect();
    if picks.is_empty() {
        return refused(format!(
            "`{APPLY}` needs the difference ids to apply in `to_board` or `to_schematic`, \
             as `check_board_against_schematic` answered them"
        ));
    }
    let target = match app.compare_target(APPLY, args) {
        Ok(target) => target,
        Err(why) => return refused(why),
    };
    if let Some(text) = text_operand(args, "expect_revision") {
        let Ok(expected) = text.trim().parse::<u64>() else {
            return refused(format!(
                "`expect_revision` is the board revision the check answered, not `{text}`"
            ));
        };
        let now = app
            .document_state
            .board_engine(&target.1)
            .map(|engine| engine.revision());
        if now != Some(expected) {
            return refused(format!(
                "the board {} is at revision {}, not {expected}: check it again",
                file_name(&target.1),
                now.map_or_else(|| "none".to_string(), |n| n.to_string())
            ));
        }
    }
    app.start_compare_job(&command, args, target, JobMode::Apply(picks))
}
