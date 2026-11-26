//! `export_block_diagram document=<diagram.snxblk> path=<file> [kind=svg|pdf]
//! [monochrome=<bool>] [title_block=<bool>] [pages=blocks,power_tree,budget]
//! [paper=a4|a3|a2|letter|tabloid] [orientation=landscape|portrait]
//! [variant=<name>]` (#2796): the diagram's saved file drawn and written off
//! the update thread; the caller is answered once the file is written
//! ([`CommandOutcome::Deferred`]). Refusals are catalog messages and write
//! nothing.

use std::path::PathBuf;

use iced::Task;
use serde_json::{Value, json};
use signex_commands::DocumentTarget;
use signex_i18n::{Translatable, ids};
use signex_output::{BlockDiagramFormat, BlockDiagramLook};

use super::job::{ExportJob, Exported, is_block_diagram};
use super::plan::ExportSettings;
use crate::app::block_diagram::PowerTreeRun;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::core::block_diagram_panes::{ExportPage, ExportPaper};
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

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
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
    if let Some(kind) = text(args, "kind") {
        return BlockDiagramFormat::from_id(kind).ok_or_else(|| {
            Translatable::new(ids::BLOCK_DIAGRAM_EXPORT_BAD_KIND).arg("kind", kind.to_string())
        });
    }
    BlockDiagramFormat::from_path(path).ok_or_else(|| {
        Translatable::new(ids::BLOCK_DIAGRAM_EXPORT_UNKNOWN_FORMAT)
            .arg("file", path.display().to_string())
    })
}

/// The pages, paper and orientation the operands name; an SVG holds the
/// diagram only.
fn settings_of(
    args: &CommandArgs,
    format: BlockDiagramFormat,
) -> Result<ExportSettings, Translatable> {
    let mut settings = ExportSettings {
        look: BlockDiagramLook {
            monochrome: flag(args, "monochrome").unwrap_or(false),
        },
        title_block: flag(args, "title_block").unwrap_or(true),
        ..ExportSettings::default()
    };
    if let Some(list) = text(args, "pages") {
        let mut pages = Vec::new();
        for token in list.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            let page = ExportPage::from_token(token).ok_or_else(|| {
                Translatable::new(ids::BLOCK_DIAGRAM_EXPORT_BAD_PAGE).arg("page", token.to_string())
            })?;
            if !pages.contains(&page) {
                pages.push(page);
            }
        }
        if pages.is_empty() {
            return Err(Translatable::new(ids::BLOCK_DIAGRAM_EXPORT_NO_PAGES));
        }
        settings.pages = pages;
    }
    if format == BlockDiagramFormat::Svg && settings.pages != [ExportPage::Blocks] {
        return Err(Translatable::new(ids::BLOCK_DIAGRAM_EXPORT_SVG_PAGES));
    }
    if let Some(paper) = text(args, "paper") {
        settings.paper = ExportPaper::from_token(paper).ok_or_else(|| {
            Translatable::new(ids::BLOCK_DIAGRAM_EXPORT_BAD_PAPER).arg("paper", paper.to_string())
        })?;
    }
    settings.portrait = match text(args, "orientation").map(str::trim) {
        None | Some("landscape") => false,
        Some("portrait") => true,
        Some(other) => {
            return Err(Translatable::new(ids::BLOCK_DIAGRAM_EXPORT_BAD_ORIENTATION)
                .arg("orientation", other.to_string()));
        }
    };
    Ok(settings)
}

/// The power tree read for the pages that need it: the project holding the
/// diagram, for `variant` (its active one without it).
fn power_for(
    app: &Signex,
    args: &CommandArgs,
    diagram: &std::path::Path,
) -> Result<PowerTreeRun, Translatable> {
    let variant = text(args, "variant");
    let project = crate::app::block_diagram::project_holding(&app.document_state, diagram)
        .ok_or_else(|| {
            Translatable::new(ids::BLOCK_DIAGRAM_EXPORT_NO_PROJECT)
                .arg("document", diagram.display().to_string())
        })?;
    crate::app::power_tree::power_tree_variant(project, variant).map_err(|_| {
        Translatable::new(ids::BLOCK_DIAGRAM_EXPORT_BAD_VARIANT)
            .arg("variant", variant.unwrap_or_default().to_string())
    })?;
    app.power_tree_reader(diagram, variant).ok_or_else(|| {
        Translatable::new(ids::BLOCK_DIAGRAM_EXPORT_NO_PROJECT)
            .arg("document", diagram.display().to_string())
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
    let settings = match settings_of(args, format) {
        Ok(settings) => settings,
        Err(reason) => return refused(reason),
    };
    if let Err(why) = app.refuse_while_project_moves(&path, "export to") {
        return refused(crate::app::state::untranslated(why.to_string()));
    }
    let job = app.block_diagram_job(diagram, path, format, &settings);
    let power = if job.needs_power_tree() && format == BlockDiagramFormat::Pdf {
        match power_for(app, args, &job.diagram) {
            Ok(run) => Some(run),
            Err(reason) => return refused(reason),
        }
    } else {
        None
    };
    // The tab's popover (the translator) is answered as the dialogs' export
    // is: a notice and a Messages line once the file is written.
    if app.agent.from_view {
        let started = json!({
            "started": "export_block_diagram",
            "path": job.path.display().to_string(),
        });
        let task = app.start_block_diagram_export(job, None, power);
        return (CommandOutcome::Answered(started), task);
    }
    let command = AppCommandId::new("export_block_diagram").expect("a catalog id is valid");
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    let task = app.start_block_diagram_export(job, Some(waiting), power);
    (CommandOutcome::Deferred { ticket }, task)
}

/// The answer of a written export. Paths are the host's.
pub(super) fn answered(job: &ExportJob, done: &Exported) -> CommandOutcome {
    let [min_x, min_y, max_x, max_y] = done.extent_mm;
    let pages: Vec<&str> = job.pages.iter().map(|page| page.token()).collect();
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
        "pages": pages,
        "page_count": done.page_count,
    });
    CommandOutcome::Answered(answer)
}
