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

/// The `overwrite=true` operand of an export.
pub(crate) fn overwrite(args: &CommandArgs) -> bool {
    matches!(args.operand("overwrite"), Some(OperandValue::Bool(true)))
}

/// The file an export writes, `path` resolved against the project folder
/// `project_dir`: refused unless its extension is `extension`, when it
/// names a Signex document (`.snx*`), and when it exists without
/// `overwrite`.
pub(crate) fn export_target(
    project_dir: &std::path::Path,
    path: &std::path::Path,
    extension: &str,
    overwrite: bool,
) -> Result<std::path::PathBuf, Translatable> {
    let target = if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_dir.join(path)
    };
    let shown = target.display().to_string();
    // A file extension, not UI text: ASCII lowercase in every language.
    let ext = target
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if ext.starts_with("snx") {
        return Err(Translatable::new(ids::NOTEBOOK_TAB_CMD_SNX_TARGET).arg("value", shown));
    }
    if ext != extension {
        return Err(Translatable::new(ids::NOTEBOOK_TAB_CMD_BAD_TARGET_EXT)
            .arg("ext", format!(".{extension}"))
            .arg("value", shown));
    }
    if !overwrite && target.exists() {
        return Err(Translatable::new(ids::NOTEBOOK_TAB_CMD_TARGET_EXISTS).arg("value", shown));
    }
    Ok(target)
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
