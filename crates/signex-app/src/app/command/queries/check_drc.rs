//! `check_drc document=<board> [baseline=<run>]` (#2199, decisions
//! https://github.com/alplabai/signex/issues/2199#issuecomment-6817821301 and
//! https://github.com/alplabai/signex/issues/2199#issuecomment-6818011610):
//! DRC on demand. It checks the one board named with Run DRC's rules engine
//! and layers, in the board's own project (Run DRC takes the active one,
//! #2332), off the UI thread, and answers with a `signex_types::check`
//! report once the run lands (`check_run`); the baseline is scoped to the
//! board file. Nothing
//! reaches the DRC panel, the DRC state or Messages; `run_drc` is
//! unchanged. See `docs/CLI.md`, "Checks on demand".

use std::path::Path;

use iced::Task;

use super::check_run::CheckTarget;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::{DocumentKind, referenced_files};
use crate::app::handlers::DRC_CHECK;
use crate::app::state::LoadedProject;
use crate::app::state::scope::path_key_with;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

impl Signex {
    /// Start the check `args` asks for; refused at once when the operands
    /// name no board.
    pub(super) fn check_drc(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> (CommandOutcome, Task<Message>) {
        let Some(OperandValue::Document(named)) = args.operand("document") else {
            let reason = "`check_drc` needs the operand `document`".to_string();
            return (CommandOutcome::disabled(reason), Task::none());
        };
        let entry = match self.resolve_document_target(named) {
            Ok(entry) => entry,
            Err(reason) => return (CommandOutcome::disabled(reason), Task::none()),
        };
        if entry.kind != DocumentKind::Pcb {
            let reason = format!(
                "`check_drc` checks a board, and {} is a {}",
                entry.name,
                entry.kind.as_str()
            );
            return (CommandOutcome::disabled(reason), Task::none());
        }
        let Some(board) = entry.path else {
            let reason = format!("{} has no file to check", entry.name);
            return (CommandOutcome::disabled(reason), Task::none());
        };
        let named = entry.project.as_ref().map(|named| named.path.clone());
        let project = self.board_project(&board, named.as_deref());
        let job = self.drc_check_job(&board, project);
        let target = CheckTarget {
            check: DRC_CHECK,
            document: board.clone(),
            scope: board,
        };
        self.start_check(command, args, target, job)
    }

    /// The project of `board`: the one its entry names (`named`), else the
    /// open project whose `pcb_file` is the board. The document entry of an
    /// open board names no project (ownership follows listed sheets only),
    /// while a closed one's does.
    fn board_project(&self, board: &Path, named: Option<&Path>) -> Option<&LoadedProject> {
        let projects = &self.document_state.projects;
        if let Some(named) = named {
            return projects.iter().find(|project| project.path == named);
        }
        projects.iter().find(|project| {
            let key = |path: &Path| path_key_with(path, project.case_fold);
            referenced_files(project)
                .iter()
                .any(|(path, kind)| *kind == DocumentKind::Pcb && key(path) == key(board))
        })
    }
}
