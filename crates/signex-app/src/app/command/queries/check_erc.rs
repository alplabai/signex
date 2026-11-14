//! `check_erc document=<sheet> [baseline=<run>]` (#2199, decision
//! https://github.com/alplabai/signex/issues/2199#issuecomment-6798772157):
//! ERC on demand. It checks the project of the named sheet (a loose sheet on
//! its own) as Run ERC does, off the UI thread, and answers with a
//! `signex_types::check` report once the run lands (`check_run`). Nothing
//! reaches the ERC panel, the ERC state or Messages; `run_erc` is unchanged.
//! See `docs/CLI.md`, "Checks on demand".

use std::path::PathBuf;

use iced::Task;

use super::check_run::CheckTarget;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::DocumentKind;
use crate::app::handlers::erc::ERC_CHECK;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

impl Signex {
    /// Start the check `args` asks for; refused at once when the operands
    /// name nothing it can check.
    pub(super) fn check_erc(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> (CommandOutcome, Task<Message>) {
        let (sheet, scope) = match self.erc_check_target(args) {
            Ok(target) => target,
            Err(reason) => return (CommandOutcome::disabled(reason), Task::none()),
        };
        let project = self
            .document_state
            .projects
            .iter()
            .find(|project| project.path == scope);
        let job = self.erc_check_job(&sheet, project);
        let target = CheckTarget {
            check: ERC_CHECK,
            document: sheet,
            scope,
        };
        self.start_check(command, args, target, move || Ok(job()))
    }

    /// The sheet `args` names and what checking it covers: its project's
    /// file, or the sheet itself when it is in no open project.
    fn erc_check_target(&self, args: &CommandArgs) -> Result<(PathBuf, PathBuf), String> {
        let Some(OperandValue::Document(target)) = args.operand("document") else {
            return Err("`check_erc` needs the operand `document`".to_string());
        };
        let entry = self.resolve_document_target(target)?;
        if entry.kind != DocumentKind::Schematic {
            return Err(format!(
                "`check_erc` checks a sheet, and {} is a {}",
                entry.name,
                entry.kind.as_str()
            ));
        }
        let Some(sheet) = entry.path else {
            return Err(format!("{} has no file to check", entry.name));
        };
        let project = entry.project.as_ref().and_then(|named| {
            self.document_state
                .projects
                .iter()
                .find(|project| project.path == named.path)
        });
        let scope = project.map_or_else(|| sheet.clone(), |project| project.path.clone());
        Ok((sheet, scope))
    }
}
