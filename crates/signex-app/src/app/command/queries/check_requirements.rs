//! `check_requirements document=<project, or one of its sheets or its board>
//! [baseline=<run>]` (#2339): the project's requirements list run as
//! deterministic checks, off the UI thread, answered with a
//! `signex_types::check` report once the run lands (`check_run`). The
//! baseline is scoped to the project file. Nothing reaches a panel, the ERC
//! or DRC state or Messages. See `docs/CLI.md`, "Requirement checks".

use iced::Task;

use super::check_run::CheckTarget;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::DocumentKind;
use crate::app::handlers::REQUIREMENTS_CHECK;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

impl Signex {
    /// Start the check `args` asks for; refused at once when the operands
    /// name no open project.
    pub(super) fn check_requirements(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> (CommandOutcome, Task<Message>) {
        let Some(OperandValue::Document(named)) = args.operand("document") else {
            let reason = "`check_requirements` needs the operand `document`".to_string();
            return (CommandOutcome::disabled(reason), Task::none());
        };
        let entry = match self.resolve_document_target(named) {
            Ok(entry) => entry,
            Err(reason) => return (CommandOutcome::disabled(reason), Task::none()),
        };
        let path = match entry.kind {
            DocumentKind::Project => entry.path.clone(),
            _ => entry.project.as_ref().map(|project| project.path.clone()),
        };
        let project = path.and_then(|path| {
            self.document_state
                .projects
                .iter()
                .find(|project| project.path == path)
        });
        let Some(project) = project else {
            let reason = format!(
                "`check_requirements` checks a project, and {} is not part of an open one",
                entry.name
            );
            return (CommandOutcome::disabled(reason), Task::none());
        };
        let job = self.requirements_check_job(project);
        let target = CheckTarget {
            check: REQUIREMENTS_CHECK,
            document: entry.path.unwrap_or_else(|| project.path.clone()),
            scope: project.path.clone(),
        };
        self.start_check(command, args, target, job)
    }
}
