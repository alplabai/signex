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

/// Blocking: read the notebook file, chapter or attachment `file`. With
/// `confine` (a headless or agent run, #2172, #3020) only a regular file
/// inside that project folder, reached through no link out of it.
pub(crate) fn read_notebook_file(
    confine: Option<&std::path::Path>,
    file: &std::path::Path,
) -> std::io::Result<Vec<u8>> {
    if let Some(dir) = confine {
        crate::app::document_ids::confined_read(dir, file).map_err(std::io::Error::other)?;
    }
    std::fs::read(file)
}

/// Blocking: refuse writing `target` for a headless or agent run (with
/// `confine`, the project folder) when it is not inside that folder, or
/// any existing folder on the way to it is a link out of it (#3020).
pub(crate) fn confined_write(
    confine: Option<&std::path::Path>,
    target: &std::path::Path,
) -> Result<(), String> {
    let Some(dir) = confine else {
        return Ok(());
    };
    use signex_i18n::RenderTranslatable;
    let refused = || {
        Translatable::new(ids::NOTEBOOK_TAB_CMD_WRITE_OUTSIDE)
            .arg("path", target.display().to_string())
            .render_en_us()
    };
    let Ok(folder) = std::fs::canonicalize(dir) else {
        return Err(refused());
    };
    // The deepest part of the path that exists, links followed, must be in
    // the project; a link at the target itself is refused too.
    let mut existing = target.to_path_buf();
    while !existing.exists() {
        if !existing.pop() {
            break;
        }
    }
    let resolved = std::fs::canonicalize(&existing).unwrap_or(existing);
    let is_link = std::fs::symlink_metadata(target).is_ok_and(|m| m.file_type().is_symlink());
    if !resolved.starts_with(&folder) || is_link {
        return Err(refused());
    }
    Ok(())
}

/// The `overwrite=true` operand of an export.
pub(crate) fn overwrite(args: &CommandArgs) -> bool {
    matches!(args.operand("overwrite"), Some(OperandValue::Bool(true)))
}

/// Refuse a write target inside a `.git` folder (any case), and on Windows
/// a name the file system would change or split: one ending in a dot or a
/// space, or holding `:` (an alternate data stream). The `path` of an
/// export or of the fabrication outputs' folder (#2470).
pub(crate) fn reserved_target(target: &std::path::Path) -> Result<(), Translatable> {
    let shown = || target.display().to_string();
    for part in target.components() {
        let std::path::Component::Normal(name) = part else {
            continue;
        };
        let name = name.to_string_lossy();
        if name.eq_ignore_ascii_case(".git") {
            return Err(Translatable::new(ids::NOTEBOOK_TAB_CMD_GIT_TARGET).arg("value", shown()));
        }
        if cfg!(windows) && (name.ends_with('.') || name.ends_with(' ') || name.contains(':')) {
            return Err(
                Translatable::new(ids::NOTEBOOK_TAB_CMD_RESERVED_NAME).arg("value", shown())
            );
        }
    }
    Ok(())
}

/// The file an export writes, `path` resolved against the project folder
/// `project_dir`: refused unless its extension is `extension`, when it
/// names a Signex document (`.snx*`) or a reserved place
/// ([`reserved_target`]), and when it exists without `overwrite`.
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
    reserved_target(&target)?;
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
