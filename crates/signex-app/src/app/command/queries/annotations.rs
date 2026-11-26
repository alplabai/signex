//! `list_annotations document=<project, or one of its documents> [id=<typed
//! id>] [kind=<note|calculation|measurement|issue|decision>] [part=<designator>]`
//! (mockup row 198): every annotation block of the project's Design
//! Notebook files as a review record with its typed id, what cites it, and
//! the failing Measurements that open no Issue yet. The files are read off
//! the UI thread and the request is answered once they are; nothing reaches
//! the panel or Messages. See `docs/COMMANDS.md`, `list_annotations`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use serde_json::{Value, json};
use signex_i18n::{Translatable, ids};
use signex_notebook::{Annotation, AnnotationKind, BlockId, Filter, IsoDate, links};

use super::text_operand;
use crate::app::agent::{AgentMsg, WaitingCommand};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::annotations::read_notebook_files;
use crate::app::notebook_project::{bad_value, refusal};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::panels::annotations::{FileReadError, NotebookScan};

/// What a `list_annotations` request asks for: one id, one kind, the
/// blocks naming one part.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnnotationQuery {
    pub(crate) id: Option<BlockId>,
    pub(crate) filter: Filter,
}

/// The notebook files a `list_annotations` request waited for were read.
#[derive(Debug, Clone)]
pub struct AnnotationsListed {
    pub(crate) waiting: WaitingCommand,
    /// The project file the answer names.
    pub(crate) project: PathBuf,
    pub(crate) query: AnnotationQuery,
    pub(crate) scan: Arc<Result<NotebookScan, String>>,
}

impl Signex {
    /// Read the notebook files of the project `args` names, answered when
    /// read; refused at once when the operands name no open project or a
    /// filter is not one the command takes.
    pub(super) fn list_annotations(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> (CommandOutcome, Task<Message>) {
        let (project, dir) = match self.notebook_project_named("list_annotations", args) {
            Ok(project) => (project.path.clone(), project.dir().to_path_buf()),
            Err(reason) => return refusal(reason),
        };
        let query = match annotation_query(args) {
            Ok(query) => query,
            Err(reason) => return refusal(reason),
        };
        let waiting = self.wait_for_command(command, args);
        let ticket = waiting.ticket;
        let task = Task::perform(
            async move {
                tokio::task::spawn_blocking(move || read_notebook_files(&dir))
                    .await
                    .map_err(|e| format!("the notebook read stopped: {e}"))
                    .and_then(|scan| scan)
            },
            move |scan| {
                Message::Agent(AgentMsg::AnnotationsListed(Box::new(AnnotationsListed {
                    waiting,
                    project,
                    query,
                    scan: Arc::new(scan),
                })))
            },
        );
        (CommandOutcome::Deferred { ticket }, task)
    }

    /// The notebook files of a `list_annotations` request were read: answer
    /// its caller (the socket, or a headless run).
    pub(crate) fn handle_annotations_listed(&mut self, listed: AnnotationsListed) -> Task<Message> {
        let AnnotationsListed {
            waiting,
            project,
            query,
            scan,
        } = listed;
        let outcome = match scan.as_ref() {
            Ok(scan) => CommandOutcome::Answered(annotations_answer(&project, &query, scan)),
            Err(why) => CommandOutcome::Disabled {
                reason: Translatable::new(ids::NOTEBOOK_TAB_CMD_FILES_UNREADABLE)
                    .arg("why", why.clone()),
            },
        };
        self.finish_waiting_command(waiting, &project, move |_| outcome)
    }
}

/// The filters of `args`: an `id` that is a typed id, a `kind` that is one
/// of the five, a `part` designator.
pub(crate) fn annotation_query(args: &CommandArgs) -> Result<AnnotationQuery, Translatable> {
    let id = match text_operand(args, "id").map(str::trim) {
        None | Some("") => None,
        Some(text) => Some(
            BlockId::parse(text)
                .ok_or_else(|| bad_value(ids::NOTEBOOK_TAB_CMD_BAD_BLOCK_ID, text))?,
        ),
    };
    let mut filter = Filter::default();
    if let Some(text) = text_operand(args, "kind").map(str::trim)
        && !text.is_empty()
    {
        let kind = AnnotationKind::parse(text)
            .ok_or_else(|| bad_value(ids::NOTEBOOK_TAB_CMD_BAD_BLOCK_KIND, text))?;
        filter.hidden_kinds = AnnotationKind::ALL
            .into_iter()
            .filter(|other| *other != kind)
            .collect();
    }
    filter.part = text_operand(args, "part")
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string);
    Ok(AnnotationQuery { id, filter })
}

/// The answer: each file with the blocks the query keeps (each with the
/// ids citing it), what could not be read, every citation and the failing
/// Measurements that open no Issue yet. Problems are named, not worded, so
/// the answer reads no locale.
pub(crate) fn annotations_answer(
    project: &Path,
    query: &AnnotationQuery,
    scan: &NotebookScan,
) -> Value {
    let everything: Vec<Annotation> = scan
        .files
        .iter()
        .flat_map(|file| file.annotations.iter().cloned())
        .collect();
    let all_links = links(&everything);
    // The query sets no date window, so no day is read from the clock.
    let keeps = |a: &Annotation| {
        query.filter.shows(a, IsoDate::EPOCH)
            && query.id.as_ref().is_none_or(|id| a.id.as_ref() == Some(id))
    };
    let mut count = 0;
    let files: Vec<Value> = scan
        .files
        .iter()
        .map(|file| {
            let blocks: Vec<Value> = file
                .annotations
                .iter()
                .filter(|a| keeps(a))
                .map(|a| {
                    let cited_by: Vec<Value> = a.id.as_ref().map_or_else(Vec::new, |id| {
                        all_links
                            .iter()
                            .filter(|link| &link.to == id)
                            .map(|link| json!({"id": link.by, "citation": link.citation}))
                            .collect()
                    });
                    let mut block = serde_json::to_value(a).unwrap_or(Value::Null);
                    if let Value::Object(map) = &mut block {
                        map.insert("cited_by".into(), Value::Array(cited_by));
                    }
                    block
                })
                .collect();
            count += blocks.len();
            json!({
                "file": file.label,
                "read_error": file.read_error.as_ref().map(read_error_value),
                "blocks": blocks,
                "problems": file.problems,
            })
        })
        .collect();
    let unopened: Vec<String> = signex_notebook::unopened_failures(&everything)
        .into_iter()
        .filter_map(|a| a.id.as_ref().map(ToString::to_string))
        .collect();
    json!({
        "project": project.to_string_lossy(),
        "notebook_found": scan.folder_found,
        "count": count,
        "files": files,
        "unlisted": scan.unlisted,
        "links": all_links,
        "unopened_failures": unopened,
    })
}

/// Why a file was not read: the system's reason, or `not_utf8`.
fn read_error_value(error: &FileReadError) -> Value {
    match error {
        FileReadError::Io(reason) => json!({"io": reason}),
        FileReadError::NotUtf8 => json!("not_utf8"),
    }
}
