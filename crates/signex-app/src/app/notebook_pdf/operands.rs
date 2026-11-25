//! `export_notebook_pdf document=<project> [path=<file>] [options]` (mockup
//! row 202): the project's notebook as PDF, made and written off the update
//! thread, answered once the file is written with what it holds. The same
//! export and write path as the dialog. Operands: `docs/NOTEBOOK_PDF.md`.

use iced::Task;
use signex_notebook::export::{ExportOptions, PageSelection, Paper, StaleRule};
use signex_notebook::{AnnotationKind, DEFAULT_NOTEBOOK_FILE, NOTEBOOK_DIR, NOTEBOOK_EXTENSION};

use super::{BuildRequest, jobs};
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::DocumentKind;
use crate::app::{LoadedProject, Message, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

const ID: &str = "export_notebook_pdf";

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

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
pub(super) fn export_options(args: &CommandArgs) -> Result<ExportOptions, String> {
    let mut options = ExportOptions::default();
    if let Some(pages) = text(args, "pages") {
        options.pages = PageSelection::parse(pages, 1).ok_or_else(|| {
            format!("`pages` is all, current, a page or a range such as 3-7, not `{pages}`")
        })?;
    }
    if let Some(paper) = text(args, "paper") {
        options.paper = Paper::parse(paper)
            .ok_or_else(|| format!("`paper` is document, a4 or letter, not `{paper}`"))?;
    }
    if let Some(variant) = text(args, "variant") {
        options.variant = variant.trim().to_string();
    }
    if let Some(stale) = text(args, "stale") {
        options.stale = StaleRule::parse(stale)
            .ok_or_else(|| format!("`stale` is warn or fail, not `{stale}`"))?;
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
fn annotation_kinds(list: &str) -> Result<std::collections::BTreeSet<AnnotationKind>, String> {
    match list.trim() {
        "all" => return Ok(AnnotationKind::ALL.into_iter().collect()),
        "none" | "" => return Ok(Default::default()),
        _ => {}
    }
    list.split(',')
        .map(|token| {
            AnnotationKind::parse(token.trim()).ok_or_else(|| {
                format!(
                    "`annotations` lists note, calculation, measurement, issue and decision, not `{}`",
                    token.trim()
                )
            })
        })
        .collect()
}

impl Signex {
    /// The open project `document` names: the project itself, or one of
    /// its documents.
    fn named_notebook_project(&self, args: &CommandArgs) -> Result<&LoadedProject, String> {
        let Some(OperandValue::Document(named)) = args.operand("document") else {
            return Err(format!("`{ID}` needs the operand `document`"));
        };
        let entry = self.resolve_document_target(named)?;
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
            format!(
                "`{ID}` exports a project's notebook, and {} is not part of an open one",
                entry.name
            )
        })
    }
}

/// Export the notebook `args` name; answered once the PDF is written.
pub(crate) fn export_notebook_pdf(app: &mut Signex, args: &CommandArgs) -> Ran {
    let command = match AppCommandId::new(ID) {
        Ok(command) => command,
        Err(reason) => return refused(reason),
    };
    let options = match export_options(args) {
        Ok(options) => options,
        Err(reason) => return refused(reason),
    };
    let project = match app.named_notebook_project(args) {
        Ok(project) => project,
        Err(reason) => return refused(reason),
    };
    let notebook = text(args, "notebook")
        .map(notebook_file)
        .unwrap_or_else(|| DEFAULT_NOTEBOOK_FILE.to_string());
    let path = match args.operand("path") {
        Some(OperandValue::Path(path)) => Some(std::path::PathBuf::from(path)),
        _ => None,
    };
    let (id, project_dir, project_name) = (
        project.id,
        project.dir().to_path_buf(),
        project.data.name.clone(),
    );
    let names = app.notebook_design_names(id);
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    let request = BuildRequest {
        project_dir,
        project_name,
        notebook,
        path,
        options,
        generation: None,
        waiting: Some(waiting),
    };
    (
        CommandOutcome::Deferred { ticket },
        jobs::build_task(request, names),
    )
}
