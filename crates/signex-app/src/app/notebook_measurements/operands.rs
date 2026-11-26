//! The measurement commands (mockup row 199, `docs/COMMANDS.md`). Each
//! names the project with `document` (the project file, or any document
//! of it). The reads (`notebook_measurements`, `notebook_timeline`,
//! `notebook_attachments`) are queries; `export_notebook_timeline`,
//! `mark_notebook_attachment_valid` and `attach_notebook_file` write a
//! file. Every one reads the notebook off the UI thread and is answered
//! when it lands ([`CommandOutcome::Deferred`]). Refusals are catalog
//! lines (`notebook_tab.ftl`, "Commands").

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_i18n::{Translatable, ids};
use signex_notebook::edit::StaleTarget;
use signex_notebook::{BlockId, Reference};

use super::design::DesignInputs;
use super::job::{Ask, Job, NotebookRead, TabEdit, run};
use crate::app::agent::AgentMsg;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::notebook_project::{because, export_target, needs_operand, overwrite, refusal};
use crate::app::project_sheets::open_sheets_snapshot;
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

/// The read commands, answered by [`query`].
pub(crate) const QUERY_IDS: [&str; 3] = [
    "notebook_measurements",
    "notebook_timeline",
    "notebook_attachments",
];

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.trim()).filter(|t| !t.is_empty()),
        _ => None,
    }
}

fn naming(id: signex_i18n::MessageId, command: &str) -> Translatable {
    Translatable::new(id).arg("command", command.to_string())
}

fn path(args: &CommandArgs, id: &str) -> Result<PathBuf, Translatable> {
    match args.operand("path") {
        Some(OperandValue::Path(path)) => Ok(PathBuf::from(path)),
        _ => Err(needs_operand(id, "path")),
    }
}

/// The one target `net`, `part` or `pin` names (`pin` as `U3.3`).
pub(crate) fn target_of(args: &CommandArgs, id: &str) -> Result<Reference, Translatable> {
    match targets_of(args, id)?.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err(naming(ids::NOTEBOOK_TAB_CMD_NEEDS_TARGET, id)),
        _ => Err(naming(ids::NOTEBOOK_TAB_CMD_ONE_TARGET, id)),
    }
}

/// Every target `net`, `part` and `pin` name (each optional).
fn targets_of(args: &CommandArgs, id: &str) -> Result<Vec<Reference>, Translatable> {
    let mut found = Vec::new();
    if let Some(net) = text(args, "net") {
        found.push(Reference::Net(net.to_string()));
    }
    if let Some(part) = text(args, "part") {
        found.push(Reference::Component(part.to_string()));
    }
    if let Some(pin) = text(args, "pin") {
        let pin = Reference::dotted_pin(pin).ok_or_else(|| {
            naming(ids::NOTEBOOK_TAB_CMD_BAD_PIN, id).arg("value", pin.to_string())
        })?;
        found.push(pin);
    }
    Ok(found)
}

fn timeline_of(args: &CommandArgs, id: &str) -> Result<(Reference, Option<String>), Translatable> {
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
            .ok_or_else(|| {
                naming(ids::NOTEBOOK_TAB_CMD_BAD_READING, id).arg("value", reading.to_string())
            }),
        _ => Err(naming(ids::NOTEBOOK_TAB_CMD_NEEDS_FILE_OR_ID, id)),
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

fn start_by_id(
    app: &mut Signex,
    id: &str,
    args: &CommandArgs,
    ask: Result<Ask, Translatable>,
) -> Ran {
    match AppCommandId::new(id) {
        Ok(command) => start(app, &command, args, ask),
        Err(reason) => refusal(because(reason)),
    }
}

/// Run `ask` for the project `args` names off the UI thread; the caller is
/// answered when it lands.
fn start(
    app: &mut Signex,
    command: &AppCommandId,
    args: &CommandArgs,
    ask: Result<Ask, Translatable>,
) -> Ran {
    let ask = match ask {
        Ok(ask) => ask,
        Err(reason) => return refusal(reason),
    };
    let (project, project_id, job) = match app.notebook_job(command.as_str(), args, ask) {
        Ok(found) => found,
        Err(reason) => return refusal(reason),
    };
    let waiting = app.wait_for_command(command, args);
    let ticket = waiting.ticket;
    let task = Task::perform(
        async move {
            tokio::task::spawn_blocking(move || run(job))
                .await
                .map_err(|e| {
                    Translatable::new(ids::NOTEBOOK_TAB_CMD_READ_STOPPED).arg("why", e.to_string())
                })
                .and_then(|done| done)
        },
        move |answer| {
            Message::Agent(AgentMsg::NotebookRead(Box::new(NotebookRead {
                waiting,
                project,
                project_id,
                answer: Arc::new(answer),
            })))
        },
    );
    (CommandOutcome::Deferred { ticket }, task)
}

impl Signex {
    /// The project file `args` names and the job for it, captured here: the
    /// project folder, the open sheets and the notebook file as its tab
    /// holds it. Refused when `document` names no open project, or an
    /// export path is not one it writes.
    fn notebook_job(
        &self,
        id: &str,
        args: &CommandArgs,
        ask: Ask,
    ) -> Result<(PathBuf, ProjectId, Job), Translatable> {
        let project = self.notebook_project_named(id, args)?;
        let project_id = project.id;
        let design = DesignInputs::of(project, open_sheets_snapshot(&self.document_state));
        let mut ask = ask;
        if let Ask::ExportTimeline { path, .. } = &mut ask {
            *path = export_target(project.dir(), path, "csv", overwrite(args))?;
        }
        let confined = self.headless_answers().is_some() || self.agent.serving;
        let job = Job {
            ask,
            project_dir: project.dir().to_path_buf(),
            design,
            held: self.notebook_tab_file_text(project_id),
            confine: confined.then(|| project.dir().to_path_buf()),
        };
        Ok((project.path.clone(), project_id, job))
    }

    /// Make `edit` on the notebook file text the tab holds now, through the
    /// editor's history (#3019).
    fn apply_notebook_tab_edit(
        &mut self,
        id: ProjectId,
        edit: &TabEdit,
    ) -> Result<Task<Message>, Translatable> {
        let doc = self
            .document_state
            .notebooks
            .get(&id)
            .ok_or_else(|| Translatable::new(ids::NOTEBOOK_TAB_CMD_NOT_OPEN))?;
        let text = edit.apply(doc)?;
        self.replace_notebook_tab_file_text(id, &text)
    }

    /// A measurement command's work landed: an edit of the notebook file
    /// the tab holds goes into the tab (written on Save); then the caller
    /// is answered.
    pub(crate) fn handle_notebook_read(&mut self, read: NotebookRead) -> Task<Message> {
        let NotebookRead {
            waiting,
            project,
            project_id,
            answer,
        } = read;
        let mut edited = Task::none();
        let outcome = match answer.as_ref() {
            Ok(done) => match &done.tab_edit {
                None => CommandOutcome::Answered(done.answer.clone()),
                Some(edit) => match self.apply_notebook_tab_edit(project_id, edit) {
                    Ok(task) => {
                        edited = task;
                        CommandOutcome::Answered(done.answer.clone())
                    }
                    Err(reason) => CommandOutcome::Disabled { reason },
                },
            },
            Err(reason) => CommandOutcome::Disabled {
                reason: reason.clone(),
            },
        };
        let answered = self.finish_waiting_command(waiting, &project, |_| outcome);
        Task::batch([edited, answered])
    }
}
