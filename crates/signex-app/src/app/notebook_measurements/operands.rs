//! The measurement commands (mockup row 199, `docs/COMMANDS.md`). Each
//! names the project with `document` (the project file, or any document
//! of it). The reads (`notebook_measurements`, `notebook_timeline`,
//! `notebook_attachments`) are queries; `export_notebook_timeline`,
//! `mark_notebook_attachment_valid` and `attach_notebook_file` write a
//! file. Every one reads the notebook off the UI thread and is answered
//! when it lands ([`CommandOutcome::Deferred`]).

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_notebook::edit::StaleTarget;
use signex_notebook::{BlockId, Reference};

use super::design::DesignInputs;
use super::job::{Ask, Job, NotebookRead, run};
use crate::app::agent::AgentMsg;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::DocumentKind;
use crate::app::project_sheets::open_sheets_snapshot;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

/// The read commands, answered by [`query`].
pub(crate) const QUERY_IDS: [&str; 3] = [
    "notebook_measurements",
    "notebook_timeline",
    "notebook_attachments",
];

fn refused(reason: impl Into<String>) -> Ran {
    (CommandOutcome::disabled(reason), Task::none())
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.trim()).filter(|t| !t.is_empty()),
        _ => None,
    }
}

fn path(args: &CommandArgs, id: &str) -> Result<PathBuf, String> {
    match args.operand("path") {
        Some(OperandValue::Path(path)) => Ok(PathBuf::from(path)),
        _ => Err(format!("`{id}` needs the operand `path`")),
    }
}

/// The one target `net`, `part` or `pin` names (`pin` as `U3.3`).
pub(crate) fn target_of(args: &CommandArgs, id: &str) -> Result<Reference, String> {
    match targets_of(args, id)?.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err(format!(
            "`{id}` needs one of the operands `net`, `part` or `pin`"
        )),
        _ => Err(format!(
            "`{id}` takes one of `net`, `part` or `pin`, not several"
        )),
    }
}

/// Every target `net`, `part` and `pin` name (each optional).
fn targets_of(args: &CommandArgs, id: &str) -> Result<Vec<Reference>, String> {
    let mut found = Vec::new();
    if let Some(net) = text(args, "net") {
        found.push(Reference::Net(net.to_string()));
    }
    if let Some(part) = text(args, "part") {
        found.push(Reference::Component(part.to_string()));
    }
    if let Some(pin) = text(args, "pin") {
        let pin = Reference::dotted_pin(pin)
            .ok_or_else(|| format!("`{id}`: `pin` is a part and a pin, `U3.3`, not `{pin}`"))?;
        found.push(pin);
    }
    Ok(found)
}

fn timeline_of(args: &CommandArgs, id: &str) -> Result<(Reference, Option<String>), String> {
    let quantity = text(args, "quantity").map(str::to_string);
    Ok((target_of(args, id)?, quantity))
}

/// `notebook_measurements`, `notebook_timeline`, `notebook_attachments`.
pub(crate) fn query(app: &mut Signex, command: &AppCommandId, args: &CommandArgs) -> Ran {
    let id = command.as_str();
    let ask = match id {
        "notebook_measurements" => Ok(Ask::Measurements),
        "notebook_attachments" => Ok(Ask::Attachments),
        _ => timeline_of(args, id).map(|(reference, quantity)| Ask::Timeline {
            reference,
            quantity,
        }),
    };
    start(app, command, args, ask)
}

/// `export_notebook_timeline document=<project> (net=|part=|pin=)
/// [quantity=] path=<file.csv>`.
pub(crate) fn export_notebook_timeline(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = "export_notebook_timeline";
    let ask = timeline_of(args, id).and_then(|(reference, quantity)| {
        Ok(Ask::ExportTimeline {
            reference,
            quantity,
            path: path(args, id)?,
        })
    });
    start_by_id(app, id, args, ask)
}

/// `mark_notebook_attachment_valid document=<project> (file=<attachment> |
/// id=<MEAS-..>)`: Mark still valid.
pub(crate) fn mark_notebook_attachment_valid(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = "mark_notebook_attachment_valid";
    let ask = match (text(args, "file"), text(args, "id")) {
        (Some(file), None) => Ok(Ask::MarkValid(StaleTarget::Attachment(file.to_string()))),
        (None, Some(reading)) => BlockId::parse(reading)
            .map(|block| Ask::MarkValid(StaleTarget::Reading(block)))
            .ok_or_else(|| format!("`{id}`: `{reading}` is not a block id such as MEAS-005")),
        _ => Err(format!(
            "`{id}` needs one of the operands `file` (an attachment) or `id` (a reading)"
        )),
    };
    start_by_id(app, id, args, ask)
}

/// `attach_notebook_file document=<project> path=<file> [kind=<folder>]
/// [text=<caption>] [net=] [part=] [pin=]`: the file copied into
/// `notebook/attachments/` and its figure added to the notebook.
pub(crate) fn attach_notebook_file(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = "attach_notebook_file";
    let ask = path(args, id).and_then(|from| {
        Ok(Ask::Attach {
            from,
            folder: text(args, "kind").map(str::to_string),
            caption: text(args, "text").unwrap_or_default().to_string(),
            targets: targets_of(args, id)?,
        })
    });
    start_by_id(app, id, args, ask)
}

fn start_by_id(app: &mut Signex, id: &str, args: &CommandArgs, ask: Result<Ask, String>) -> Ran {
    match AppCommandId::new(id) {
        Ok(command) => start(app, &command, args, ask),
        Err(reason) => refused(reason),
    }
}

/// Run `ask` for the project `args` names off the UI thread; the caller is
/// answered when it lands.
fn start(
    app: &mut Signex,
    command: &AppCommandId,
    args: &CommandArgs,
    ask: Result<Ask, String>,
) -> Ran {
    let ask = match ask {
        Ok(ask) => ask,
        Err(reason) => return refused(reason),
    };
    let (project, job) = match app.notebook_job(command.as_str(), args, ask) {
        Ok(found) => found,
        Err(reason) => return refused(reason),
    };
    let waiting = app.wait_for_command(command, args);
    let ticket = waiting.ticket;
    let task = Task::perform(
        async move {
            tokio::task::spawn_blocking(move || run(job))
                .await
                .map_err(|e| format!("the notebook read stopped: {e}"))
                .and_then(|answer| answer)
        },
        move |answer| {
            Message::Agent(AgentMsg::NotebookRead(Box::new(NotebookRead {
                waiting,
                project,
                answer: Arc::new(answer),
            })))
        },
    );
    (CommandOutcome::Deferred { ticket }, task)
}

impl Signex {
    /// The project file `args` names and the job for it, captured here: the
    /// project folder and the open sheets. Refused when `document` names
    /// no open project.
    fn notebook_job(
        &self,
        id: &str,
        args: &CommandArgs,
        ask: Ask,
    ) -> Result<(PathBuf, Job), String> {
        let Some(OperandValue::Document(named)) = args.operand("document") else {
            return Err(format!("`{id}` needs the operand `document`"));
        };
        let entry = self.resolve_document_target(named)?;
        let path = match entry.kind {
            DocumentKind::Project => entry.path.clone(),
            _ => entry.project.as_ref().map(|project| project.path.clone()),
        };
        let project = path.and_then(|path| {
            self.document_state
                .projects
                .iter()
                .find(|project| project.path == path)
        });
        let Some(project) = project else {
            return Err(format!(
                "`{id}` reads a project's notebook, and {} is not part of an open one",
                entry.name
            ));
        };
        let design = DesignInputs::of(project, open_sheets_snapshot(&self.document_state));
        let job = Job {
            ask,
            project_dir: project.dir().to_path_buf(),
            design,
        };
        Ok((project.path.clone(), job))
    }

    /// A measurement command's work landed: answer its caller.
    pub(crate) fn handle_notebook_read(&mut self, read: NotebookRead) -> Task<Message> {
        let NotebookRead {
            waiting,
            project,
            answer,
        } = read;
        let outcome = match answer.as_ref() {
            Ok(value) => CommandOutcome::Answered(value.clone()),
            Err(reason) => CommandOutcome::disabled(reason.clone()),
        };
        self.finish_waiting_command(waiting, &project, |_| outcome)
    }
}
