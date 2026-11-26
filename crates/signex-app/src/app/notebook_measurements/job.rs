//! The work behind the measurement commands, off the UI thread: read the
//! project's notebook (the notebook file, its chapters and the attachments
//! they name), set the design's part values, and answer, export or write.
//! What it reads and decides is `signex_notebook`'s; this is the IO.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use serde_json::{Value, json};
use signex_notebook::attachment::{
    AttachmentKind, attachment_call, folder_for, mark_still_valid, plan_copy,
};
use signex_notebook::bench::record;
use signex_notebook::edit::{StaleTarget, append_call, locate};
use signex_notebook::report::{attachments_report, measurements_report, timeline_report};
use signex_notebook::{
    DEFAULT_NOTEBOOK_FILE, LoadProblem, LoadProblemKind, NOTEBOOK_DIR, NotebookSources, Reference,
};
use signex_types::atomic_io::{atomic_write, atomic_write_existing_dir};

use super::design::DesignInputs;
use crate::app::agent::WaitingCommand;

/// What a command asks of the notebook.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Ask {
    /// `notebook_measurements`.
    Measurements,
    /// `notebook_attachments`.
    Attachments,
    /// `notebook_timeline`.
    Timeline {
        reference: Reference,
        quantity: Option<String>,
    },
    /// `export_notebook_timeline`: the timeline's list as CSV at `path`.
    ExportTimeline {
        reference: Reference,
        quantity: Option<String>,
        path: PathBuf,
    },
    /// `mark_notebook_attachment_valid`.
    MarkValid(StaleTarget),
    /// `attach_notebook_file`: copy `from` into `notebook/attachments/` and
    /// add its figure at the end of the notebook file.
    Attach {
        from: PathBuf,
        folder: Option<String>,
        caption: String,
        targets: Vec<Reference>,
    },
}

/// One command's work, captured in `update`.
pub(crate) struct Job {
    pub(crate) ask: Ask,
    pub(crate) project_dir: PathBuf,
    /// `None` when the project has no sheet: then nothing is stale.
    pub(crate) design: Option<DesignInputs>,
}

/// A command's work landed: answer the caller that waits on it.
#[derive(Debug, Clone)]
pub struct NotebookRead {
    pub(crate) waiting: WaitingCommand,
    /// The project file the command named, for the reply's gate.
    pub(crate) project: PathBuf,
    pub(crate) answer: Arc<Result<Value, String>>,
}

/// Do `job`: the command's answer, or why it was refused.
pub(crate) fn run(job: Job) -> Result<Value, String> {
    let folder = job.project_dir.join(NOTEBOOK_DIR);
    let (mut sources, problems) = load(&folder)?;
    if let Some(design) = &job.design {
        sources.set_design_values(design.values());
    }
    let notebook = format!("{NOTEBOOK_DIR}/{DEFAULT_NOTEBOOK_FILE}");
    let mut answer = match job.ask {
        Ask::Measurements => to_value(&measurements_report(&sources))?,
        Ask::Attachments => to_value(&attachments_report(&sources))?,
        Ask::Timeline {
            reference,
            quantity,
        } => to_value(&timeline_report(&sources, &reference, quantity.as_deref()))?,
        Ask::ExportTimeline {
            reference,
            quantity,
            path,
        } => {
            let report = timeline_report(&sources, &reference, quantity.as_deref());
            atomic_write(&path, report.csv.as_bytes())
                .map_err(|e| format!("the timeline was not written to {}: {e}", path.display()))?;
            json!({ "path": path.display().to_string(), "readings": report.timeline.readings })
        }
        Ask::MarkValid(target) => mark_valid(&folder, &sources, &target)?,
        Ask::Attach {
            from,
            folder: asked,
            caption,
            targets,
        } => attach(
            &folder,
            &sources,
            &from,
            asked.as_deref(),
            &caption,
            &targets,
        )?,
    };
    answer["notebook"] = json!(notebook);
    answer["load_problems"] = to_value(&problems)?;
    Ok(answer)
}

/// The notebook file, its chapters and the attachments they name. Refused
/// when the notebook file itself cannot be read; a chapter or attachment
/// that cannot is listed, and an attachment missing from the folder is
/// what the answer reports as missing.
fn load(folder: &Path) -> Result<(NotebookSources, Vec<LoadProblem>), String> {
    let read = |path: &str| std::fs::read(folder.join(path));
    let (mut sources, problems) = NotebookSources::load(DEFAULT_NOTEBOOK_FILE, read);
    if let Some(main) = problems.iter().find(|p| p.file.is_empty()) {
        let why = match &main.kind {
            LoadProblemKind::Unreadable { reason } => reason.clone(),
            LoadProblemKind::OutsideNotebook => "it is outside the notebook folder".to_string(),
        };
        return Err(format!(
            "the project's notebook {NOTEBOOK_DIR}/{DEFAULT_NOTEBOOK_FILE} cannot be read: {why}"
        ));
    }
    for path in sources.missing_attachments() {
        if let Ok(bytes) = read(&path) {
            sources.insert(&path, bytes);
        }
    }
    Ok((sources, problems))
}

/// Mark still valid: store the current part values in the call `target`
/// names, and write its notebook file.
fn mark_valid(
    folder: &Path,
    sources: &NotebookSources,
    target: &StaleTarget,
) -> Result<Value, String> {
    let named = match target {
        StaleTarget::Attachment(file) => format!("the attachment {file}"),
        StaleTarget::Reading(id) => format!("the reading {id}"),
    };
    let found = record(sources);
    let (file, line) = locate(&found, target)
        .ok_or_else(|| format!("{named} is not in the notebook, or stores no part values"))?;
    let text = sources
        .get(&file)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .ok_or_else(|| format!("{NOTEBOOK_DIR}/{file} is not UTF-8 text"))?;
    let values = sources.design_values();
    if values.is_empty() {
        return Err(format!(
            "the schematic could not be read, so the current values of {named} are not known"
        ));
    }
    let marked = mark_still_valid(text, line, values)
        .ok_or_else(|| format!("{named} at {NOTEBOOK_DIR}/{file}:{line} stores no part values"))?;
    atomic_write_existing_dir(&folder.join(&file), marked.as_bytes())
        .map_err(|e| format!("{NOTEBOOK_DIR}/{file} was not written: {e}"))?;
    Ok(json!({ "file": format!("{NOTEBOOK_DIR}/{file}"), "line": line, "marked": named }))
}

/// Copy `from` into the attachments folder and add its figure at the end
/// of the notebook file.
fn attach(
    folder: &Path,
    sources: &NotebookSources,
    from: &Path,
    asked: Option<&str>,
    caption: &str,
    targets: &[Reference],
) -> Result<Value, String> {
    let kind = AttachmentKind::of(&from.to_string_lossy());
    let sub = asked.unwrap_or_else(|| folder_for(kind));
    let copy = plan_copy(from, sub, |path| folder.join(path).exists()).ok_or_else(|| {
        format!(
            "{} cannot be attached to the folder `{sub}`: a folder is one plain name",
            from.display()
        )
    })?;
    let bytes =
        std::fs::read(from).map_err(|e| format!("{} cannot be read: {e}", from.display()))?;
    let main = sources
        .get(sources.main())
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .ok_or_else(|| format!("{NOTEBOOK_DIR}/{DEFAULT_NOTEBOOK_FILE} is not UTF-8 text"))?;
    let call = attachment_call(copy.file(), caption, targets, sources.design_values());
    atomic_write(&folder.join(&copy.to), &bytes)
        .map_err(|e| format!("{NOTEBOOK_DIR}/{} was not written: {e}", copy.to))?;
    atomic_write_existing_dir(
        &folder.join(DEFAULT_NOTEBOOK_FILE),
        append_call(main, &call).as_bytes(),
    )
    .map_err(|e| format!("{NOTEBOOK_DIR}/{DEFAULT_NOTEBOOK_FILE} was not written: {e}"))?;
    Ok(json!({
        "file": copy.file(),
        "copied_to": format!("{NOTEBOOK_DIR}/{}", copy.to),
        "call": call,
    }))
}

fn to_value(value: &impl Serialize) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| format!("the answer could not be written: {e}"))
}
