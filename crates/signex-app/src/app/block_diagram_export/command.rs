//! `export_block_diagram document=<diagram.snxblk> path=<file> [kind=svg|pdf]
//! [monochrome=<bool>] [title_block=<bool>]` (#2796): the diagram's saved
//! file drawn and written off the update thread; the caller is answered
//! once the file is written ([`CommandOutcome::Deferred`]). Refusals are
//! catalog messages and write nothing.

use std::path::PathBuf;

use iced::Task;
use serde_json::{Value, json};
use signex_commands::DocumentTarget;
use signex_i18n::{Translatable, ids};
use signex_output::{BlockDiagramFormat, BlockDiagramLook};

use super::job::{ExportJob, Exported, is_block_diagram};
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: Translatable) -> Ran {
    (CommandOutcome::Disabled { reason }, Task::none())
}

fn flag(args: &CommandArgs, name: &str) -> Option<bool> {
    match args.operand(name) {
        Some(OperandValue::Bool(value)) => Some(*value),
        _ => None,
    }
}

/// The `.snxblk` the `document` operand names: an open or referenced
/// document, else the path as given. Anything but a `.snxblk` is refused.
fn named_diagram(app: &Signex, args: &CommandArgs) -> Result<PathBuf, Translatable> {
    let Some(OperandValue::Document(named)) = args.operand("document") else {
        return Err(Translatable::new(ids::BLOCK_DIAGRAM_EXPORT_NO_DOCUMENT));
    };
    let path = match app.resolve_document_target(named) {
        Ok(entry) => entry.path,
        Err(_) => match named {
            DocumentTarget::Path(text) => Some(PathBuf::from(text)),
            DocumentTarget::Id(_) => None,
        },
    };
    path.filter(|path| is_block_diagram(path)).ok_or_else(|| {
        Translatable::new(ids::BLOCK_DIAGRAM_EXPORT_NOT_A_DIAGRAM)
            .arg("document", named.to_string())
    })
}

/// The format `kind` names, else the one `path`'s extension names.
fn format_of(
    args: &CommandArgs,
    path: &std::path::Path,
) -> Result<BlockDiagramFormat, Translatable> {
    if let Some(OperandValue::Text(kind)) = args.operand("kind") {
        return BlockDiagramFormat::from_id(kind).ok_or_else(|| {
            Translatable::new(ids::BLOCK_DIAGRAM_EXPORT_BAD_KIND).arg("kind", kind.clone())
        });
    }
    BlockDiagramFormat::from_path(path).ok_or_else(|| {
        Translatable::new(ids::BLOCK_DIAGRAM_EXPORT_UNKNOWN_FORMAT)
            .arg("file", path.display().to_string())
    })
}

pub(crate) fn export_block_diagram(app: &mut Signex, args: &CommandArgs) -> Ran {
    let diagram = match named_diagram(app, args) {
        Ok(diagram) => diagram,
        Err(reason) => return refused(reason),
    };
    let Some(OperandValue::Path(path)) = args.operand("path") else {
        return refused(Translatable::new(ids::BLOCK_DIAGRAM_EXPORT_NO_PATH));
    };
    let path = PathBuf::from(path);
    let format = match format_of(args, &path) {
        Ok(format) => format,
        Err(reason) => return refused(reason),
    };
    if path == diagram {
        return refused(
            Translatable::new(ids::BLOCK_DIAGRAM_EXPORT_SAME_FILE)
                .arg("file", path.display().to_string()),
        );
    }
    if let Err(why) = app.refuse_while_project_moves(&path, "export to") {
        return refused(crate::app::state::untranslated(why.to_string()));
    }
    let look = BlockDiagramLook {
        monochrome: flag(args, "monochrome").unwrap_or(false),
    };
    let title_block = flag(args, "title_block").unwrap_or(true);
    let job = app.block_diagram_job(diagram, path, format, look, title_block);
    // The tab's popover (the translator) is answered as the dialogs' export
    // is: a notice and a Messages line once the file is written.
    if app.agent.from_view {
        let started = json!({
            "started": "export_block_diagram",
            "path": job.path.display().to_string(),
        });
        let task = app.start_block_diagram_export(job, None);
        return (CommandOutcome::Answered(started), task);
    }
    let command = AppCommandId::new("export_block_diagram").expect("a catalog id is valid");
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    let task = app.start_block_diagram_export(job, Some(waiting));
    (CommandOutcome::Deferred { ticket }, task)
}

/// The answer of a written export. Paths are the host's.
pub(super) fn answered(job: &ExportJob, done: &Exported) -> CommandOutcome {
    let [min_x, min_y, max_x, max_y] = done.extent_mm;
    let answer: Value = json!({
        "document": job.diagram.display().to_string(),
        "path": job.path.display().to_string(),
        "kind": job.format.id(),
        "blocks": done.blocks,
        "connections": done.connections,
        "groups": done.groups,
        "bytes": done.bytes,
        "extent_mm": { "min": [min_x, min_y], "max": [max_x, max_y] },
        "scale": done.scale,
        "uncovered_chars": done.uncovered_chars.iter().collect::<String>(),
        "read_only": done.read_only,
    });
    CommandOutcome::Answered(answer)
}
