//! Why a Design Notebook command is refused, as catalog lines
//! (`notebook_tab.ftl`, "Commands"), and the one rule for the open project
//! a command's `document` operand names.

use iced::Task;
use signex_i18n::{Translatable, ids};

use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::DocumentKind;
use crate::app::{LoadedProject, Message, Signex};

/// A command refused with `reason`, doing nothing.
pub(crate) fn refusal(reason: Translatable) -> (CommandOutcome, Task<Message>) {
    (CommandOutcome::Disabled { reason }, Task::none())
}

/// A reason another part of Signex gave as text (a document that cannot
/// be found, a file that cannot be read).
pub(crate) fn because(why: impl Into<String>) -> Translatable {
    Translatable::new(ids::NOTEBOOK_TAB_CMD_BECAUSE).arg("why", why.into())
}

/// "`command` needs the operand `operand`".
pub(crate) fn needs_operand(command: &str, operand: &'static str) -> Translatable {
    Translatable::new(ids::NOTEBOOK_TAB_CMD_NEEDS_OPERAND)
        .arg("command", command.to_string())
        .arg("operand", operand)
}

/// A catalog line naming the value the operand had.
pub(crate) fn bad_value(id: signex_i18n::MessageId, value: &str) -> Translatable {
    Translatable::new(id).arg("value", value.trim().to_string())
}

impl Signex {
    /// The open project the `document` operand names: the project itself,
    /// or one of its documents.
    pub(crate) fn notebook_project_named(
        &self,
        command: &str,
        args: &CommandArgs,
    ) -> Result<&LoadedProject, Translatable> {
        let Some(OperandValue::Document(named)) = args.operand("document") else {
            return Err(needs_operand(command, "document"));
        };
        let entry = self.resolve_document_target(named).map_err(because)?;
        let path = match entry.kind {
            DocumentKind::Project => entry.path.clone(),
            _ => entry.project.as_ref().map(|project| project.path.clone()),
        };
        path.and_then(|path| {
            self.document_state
                .projects
                .iter()
                .find(|project| project.path == path)
        })
        .ok_or_else(|| {
            Translatable::new(ids::NOTEBOOK_TAB_CMD_NOT_IN_PROJECT)
                .arg("command", command.to_string())
                .arg("name", entry.name.clone())
        })
    }
}
