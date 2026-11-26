//! `export_notebook_pdf document=<project> [path=<file>] [options]` (mockup
//! row 202): the project's notebook as PDF, made and written off the update
//! thread, answered once the file is written with what it holds. The same
//! export and write path as the dialog. Operands: `docs/NOTEBOOK_PDF.md`.

use iced::Task;
use signex_i18n::{Translatable, ids};
use signex_notebook::export::{ExportOptions, PageSelection, Paper, StaleRule};
use signex_notebook::{AnnotationKind, DEFAULT_NOTEBOOK_FILE, NOTEBOOK_DIR, NOTEBOOK_EXTENSION};

use super::{BuildRequest, jobs};
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::notebook_project::{bad_value, because, export_target, overwrite, refusal};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

const ID: &str = "export_notebook_pdf";

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

fn flag(args: &CommandArgs, name: &str) -> Option<bool> {
    match args.operand(name) {
        Some(OperandValue::Bool(value)) => Some(*value),
        _ => None,
    }
}

/// The export settings the operands give; the rest is the dialog's default.
pub(super) fn export_options(args: &CommandArgs) -> Result<ExportOptions, Translatable> {
    let mut options = ExportOptions::default();
    if let Some(pages) = text(args, "pages") {
        options.pages = PageSelection::parse(pages, 1)
            .ok_or_else(|| bad_value(ids::NOTEBOOK_TAB_CMD_BAD_PAGES, pages))?;
    }
    if let Some(paper) = text(args, "paper") {
        options.paper =
            Paper::parse(paper).ok_or_else(|| bad_value(ids::NOTEBOOK_TAB_CMD_BAD_PAPER, paper))?;
    }
    if let Some(variant) = text(args, "variant") {
        options.variant = variant.trim().to_string();
    }
    if let Some(stale) = text(args, "stale") {
        options.stale = StaleRule::parse(stale)
            .ok_or_else(|| bad_value(ids::NOTEBOOK_TAB_CMD_BAD_STALE, stale))?;
    }
    if let Some(kinds) = text(args, "annotations") {
        options.annotations = annotation_kinds(kinds)?;
    }
    let flags: [(&str, &mut bool); 4] = [
        ("title_block", &mut options.title_block),
        ("contents", &mut options.contents),
        ("bookmarks", &mut options.bookmarks),
        ("python_source", &mut options.python_source),
    ];
    for (name, slot) in flags {
        if let Some(on) = flag(args, name) {
            *slot = on;
        }
    }
    Ok(options)
}

/// The notebook file `name` names, a path inside `notebook/`: the folder may
/// be written in front, and a name with no extension is a `.snxnbk` file
/// (`generated/design-report` is `generated/design-report.snxnbk`).
pub(super) fn notebook_file(name: &str) -> String {
    let name = name.trim();
    let name = name
        .strip_prefix(NOTEBOOK_DIR)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(name);
    let file = name.rsplit('/').next().unwrap_or(name);
    if file.contains('.') {
        name.to_string()
    } else {
        format!("{name}.{NOTEBOOK_EXTENSION}")
    }
}

/// `note,issue` or `all` or `none`.
fn annotation_kinds(
    list: &str,
) -> Result<std::collections::BTreeSet<AnnotationKind>, Translatable> {
    match list.trim() {
        "all" => return Ok(AnnotationKind::ALL.into_iter().collect()),
        "none" | "" => return Ok(Default::default()),
        _ => {}
    }
    list.split(',')
        .map(|token| {
            AnnotationKind::parse(token.trim())
                .ok_or_else(|| bad_value(ids::NOTEBOOK_TAB_CMD_BAD_ANNOTATIONS, token))
        })
        .collect()
}

/// Export the notebook `args` name; answered once the PDF is written.
pub(crate) fn export_notebook_pdf(app: &mut Signex, args: &CommandArgs) -> Ran {
    let command = match AppCommandId::new(ID) {
        Ok(command) => command,
        Err(reason) => return refusal(because(reason)),
    };
    let options = match export_options(args) {
        Ok(options) => options,
        Err(reason) => return refusal(reason),
    };
    let project = match app.notebook_project_named(ID, args) {
        Ok(project) => project,
        Err(reason) => return refusal(reason),
    };
    let notebook = text(args, "notebook")
        .map(notebook_file)
        .unwrap_or_else(|| DEFAULT_NOTEBOOK_FILE.to_string());
    let path = match args.operand("path") {
        Some(OperandValue::Path(path)) => {
            let path = std::path::Path::new(path);
            match export_target(project.dir(), path, "pdf", overwrite(args)) {
                Ok(target) => Some(target),
                Err(reason) => return refusal(reason),
            }
        }
        _ => None,
    };
    let (id, project_dir, project_name) = (
        project.id,
        project.dir().to_path_buf(),
        project.data.name.clone(),
    );
    let design = app.notebook_design_inputs(id);
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    let request = BuildRequest {
        project_dir,
        project_name,
        notebook,
        path,
        overwrite: Some(overwrite(args)),
        options,
        generation: None,
        waiting: Some(waiting),
    };
    (
        CommandOutcome::Deferred { ticket },
        jobs::build_task(request, design),
    )
}
