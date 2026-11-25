//! The `notebook_*` catalog commands (#2795): the agent and CLI surface of
//! the notebook tab, each naming the project with `document`.
//!
//! - `notebook_compile {document}` compiles the notebook (the text as typed
//!   when its tab holds it, else the file) off the UI thread and answers
//!   with the status bar's facts, the Problems strip, the outline and the
//!   references.
//! - `notebook_insert {document, kind, name?, line?}` writes what an insert
//!   button writes into the open notebook.
//! - `notebook_view {document, view?, panel?, next_problem?}` picks Source,
//!   Split or Preview and the side panel, or moves to the next problem.
//!
//! Shapes: `docs/DESIGN_NOTEBOOK_TAB.md`, "Commands".

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use serde_json::{Value, json};
use signex_notebook::{
    CompileOptions, Insertion, NotebookSources, NotebookStatus, Reference, compile,
};

use super::open::{NotebookRead, read_notebook};
use super::{NotebookMsg, SidePanel, ViewMode};
use crate::app::agent::WaitingCommand;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

/// Why an insert or a view change is refused with no notebook text open.
const NOT_OPEN: &str =
    "the project's Design Notebook is not open: open it with Project ▸ Design Notebook";

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.as_str()),
        _ => None,
    }
}

/// What `notebook_compile` reads: the text as typed when the tab holds it.
#[derive(Debug, Clone)]
pub(super) enum CompileSource {
    Held {
        sources: NotebookSources,
        unsaved: bool,
    },
    Disk,
}

/// `notebook_compile {document}`.
pub(crate) fn notebook_compile(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match app.rules_project_named(args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let Some(path) = app.notebook_path_of(id) else {
        return refused("the project is no longer open");
    };
    let source = match app.document_state.notebooks.get(&id) {
        Some(tab) if tab.text.is_some() => CompileSource::Held {
            sources: tab.compile_sources(),
            unsaved: tab.is_unsaved(),
        },
        _ => CompileSource::Disk,
    };
    let command = match AppCommandId::new("notebook_compile") {
        Ok(command) => command,
        Err(reason) => return refused(reason),
    };
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    let task = Task::perform(
        async move {
            let job_path = path.clone();
            let answer = tokio::task::spawn_blocking(move || compile_answer(&job_path, source))
                .await
                .map_err(|error| format!("the compile did not finish: {error}"))
                .and_then(|answer| answer);
            (path, answer)
        },
        move |(path, answer)| {
            Message::from(NotebookMsg::CommandCompiled {
                project_id: id,
                waiting: Box::new(waiting),
                path,
                answer: Arc::new(answer),
            })
        },
    );
    (CommandOutcome::Deferred { ticket }, task)
}

/// Blocking: compile the notebook at `path` from `source` and write the
/// answer.
pub(super) fn compile_answer(
    path: &std::path::Path,
    source: CompileSource,
) -> Result<Value, String> {
    let (sources, unsaved) = match source {
        CompileSource::Held { sources, unsaved } => (sources, unsaved),
        CompileSource::Disk => {
            let folder = path.parent().map_or_else(PathBuf::new, PathBuf::from);
            let NotebookRead { text, sources, .. } = read_notebook(&folder)?;
            if text.is_none() {
                return Ok(json!({
                    "notebook": path.display().to_string(),
                    "exists": false,
                }));
            }
            (sources, false)
        }
    };
    let started = std::time::Instant::now();
    let compilation = compile(
        &sources,
        &CompileOptions {
            today: super::compile::today(),
        },
    );
    let status = NotebookStatus::of(&sources, &compilation);
    let mut answer = serde_json::to_value(&status).map_err(|error| error.to_string())?;
    if let Value::Object(map) = &mut answer {
        map.insert("notebook".into(), json!(path.display().to_string()));
        map.insert("exists".into(), json!(true));
        map.insert("unsaved".into(), json!(unsaved));
        map.insert(
            "compile_ms".into(),
            json!(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)),
        );
    }
    Ok(answer)
}

impl Signex {
    /// `notebook_compile`'s compile landed: answer its caller.
    pub(super) fn notebook_command_compiled(
        &mut self,
        waiting: WaitingCommand,
        path: &std::path::Path,
        answer: &Result<Value, String>,
    ) -> Task<Message> {
        let outcome = match answer {
            Ok(value) => CommandOutcome::Answered(value.clone()),
            Err(why) => CommandOutcome::disabled(why.clone()),
        };
        self.finish_waiting_command(waiting, path, |_| outcome)
    }
}

/// The insertion `kind` (and `name`, for a reference) names.
pub(super) fn insertion_named(kind: &str, name: Option<&str>) -> Result<Insertion, String> {
    let named = || {
        name.map(str::trim)
            .filter(|name| !name.is_empty())
            .ok_or_else(|| format!("`kind={kind}` needs the operand `name`"))
    };
    match kind {
        "component" => Ok(Insertion::Reference(Reference::Component(named()?.into()))),
        "net" => Ok(Insertion::Reference(Reference::Net(named()?.into()))),
        "pin" => {
            let name = named()?;
            Reference::dotted_pin(name)
                .map(Insertion::Reference)
                .ok_or_else(|| format!("`{name}` is no pin: write it as `U3.12`"))
        }
        other => Insertion::parse(other).ok_or_else(|| {
            format!(
                "`kind` is component, pin, net or one of {}",
                Insertion::TOKENS.join(", ")
            )
        }),
    }
}

/// `notebook_insert {document, kind, name?, line?}`.
pub(crate) fn notebook_insert(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match open_notebook(app, args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let kind = text(args, "kind").unwrap_or_default();
    let insertion = match insertion_named(kind, text(args, "name")) {
        Ok(insertion) => insertion,
        Err(reason) => return refused(reason),
    };
    if let Some(line) = text(args, "line") {
        let Some(line) = line.trim().parse::<u32>().ok().filter(|line| *line > 0) else {
            return refused("`line` is a line number of the notebook file, from 1");
        };
        app.notebook_line_picked(id, line);
    }
    let line = app
        .document_state
        .notebooks
        .get(&id)
        .and_then(|tab| tab.text.as_ref().map(|text| tab.file_line(text.cursor().0)));
    match app.notebook_insert_at_cursor(id, &insertion) {
        (Some(inserted), task) => {
            let answer = json!({
                "inserted": inserted.text,
                "id": inserted.id,
                "line": line,
            });
            (CommandOutcome::Answered(answer), task)
        }
        (None, _) => refused("the notebook is read-only: a newer Signex wrote it"),
    }
}

/// `notebook_view {document, view?, panel?, next_problem?}`.
pub(crate) fn notebook_view(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match open_notebook(app, args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let view = match text(args, "view").map(|token| ViewMode::parse(token.trim())) {
        Some(None) => return refused("`view` is source, split or preview"),
        Some(Some(view)) => Some(view),
        None => None,
    };
    let panel = match text(args, "panel").map(|token| SidePanel::parse(token.trim())) {
        Some(None) => return refused("`panel` is outline, pages or references"),
        Some(Some(panel)) => Some(panel),
        None => None,
    };
    if let (Some(view), Some(tab)) = (view, app.document_state.notebooks.get_mut(&id)) {
        tab.view = view;
    }
    let task = match panel {
        Some(panel) => app.notebook_panel_picked(id, panel),
        None => Task::none(),
    };
    let problem = match args.operand("next_problem") {
        Some(OperandValue::Bool(true)) => app.notebook_next_problem(id),
        _ => None,
    };
    let answer = view_answer(app, id, problem);
    (CommandOutcome::Answered(answer), task)
}

/// The tab's view, panel, page and cursor, and the problem moved to.
fn view_answer(app: &Signex, id: ProjectId, problem: Option<(u32, u32)>) -> Value {
    let Some(tab) = app.document_state.notebooks.get(&id) else {
        return json!({});
    };
    let cursor = tab.text.as_ref().map(|text| {
        let (line, column) = text.cursor();
        json!({ "line": tab.file_line(line), "column": column + 1 })
    });
    json!({
        "view": tab.view.token(),
        "panel": tab.panel.token(),
        "page": tab.page + 1,
        "cursor": cursor,
        "problem": problem.map(|(line, column)| json!({ "line": line, "column": column })),
    })
}

/// The project `args` names, when its notebook tab holds text to act on.
fn open_notebook(app: &mut Signex, args: &CommandArgs) -> Result<ProjectId, String> {
    let id = app.rules_project_named(args)?;
    match app.document_state.notebooks.get(&id) {
        Some(tab) if tab.text.is_some() => Ok(id),
        _ => Err(NOT_OPEN.into()),
    }
}
