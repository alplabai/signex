//! The checklist in and out of the project (mockup row 206): More ▸ Export
//! CSV… writes the items as CSV through the export writer (atomic, off the
//! UI thread, reported in Messages), and the empty tab's Import from another
//! project… reads another `.snxprj` off the UI thread and starts this
//! checklist from its items, each open, keeping stage, title and link. The
//! import is one undo step and writes nothing until Save. Catalog commands:
//! `project_export_checklist_csv` and `project_import_checklist`.

use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::json;
use signex_engine::ProjectChecklistCommand;
use signex_i18n::{Translatable, ids};
use signex_types::project::{ChecklistProblem, NewChecklistItem, parse_project};

use super::commands::{Ran, edit_in_tab, named_project, refused};
use super::words::problem_refusal;
use super::{ChecklistMsg, ChecklistNotice, message};
use crate::app::agent::WaitingCommand;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// Another project's checklist, read to import.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    /// The project it came from, by file name.
    pub from: String,
    pub template: Option<String>,
    pub items: Vec<NewChecklistItem>,
}

/// The `path` operand.
fn path_operand(args: &CommandArgs) -> Option<PathBuf> {
    match args.operand("path") {
        Some(OperandValue::Path(path)) => Some(PathBuf::from(path)),
        _ => None,
    }
}

/// Read the checklist of the project at `path`. Reads the disk: run it off
/// the UI thread.
fn read_import(path: &Path) -> Result<Imported, ChecklistNotice> {
    let from = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let data =
        parse_project(path).map_err(|error| ChecklistNotice::ImportFailed(error.to_string()))?;
    match data.design_checklist() {
        Ok(Some(checklist)) => Ok(Imported {
            from,
            template: checklist.template.clone(),
            items: checklist.new_items(),
        }),
        Ok(None) => Err(ChecklistNotice::NothingToImport(from)),
        Err(problem) => Err(ChecklistNotice::ImportFailed(problem.to_string())),
    }
}

/// Why an import was refused, for a command's answer.
fn import_refusal(notice: &ChecklistNotice) -> Translatable {
    match notice {
        ChecklistNotice::NothingToImport(from) => {
            Translatable::new(ids::CHECKLIST_REFUSAL_NOTHING_TO_IMPORT)
                .arg("project", from.as_str())
        }
        ChecklistNotice::ImportFailed(why) => {
            Translatable::new(ids::CHECKLIST_REFUSAL_IMPORT_FAILED).arg("reason", why.as_str())
        }
        other => Translatable::new(ids::CHECKLIST_REFUSAL_IMPORT_FAILED)
            .arg("reason", format!("{other:?}")),
    }
}

/// The refusal of a command without its `path`; `what` says what it is.
fn needs_path(what: signex_i18n::MessageId) -> Translatable {
    Translatable::new(ids::CHECKLIST_REFUSAL_NEEDS_OPERAND)
        .arg("name", "path")
        .arg("what", Translatable::new(what))
}

/// Read the project at `path` to import, off the UI thread.
fn import_task(
    project_id: ProjectId,
    path: PathBuf,
    waiting: Option<WaitingCommand>,
) -> Task<Message> {
    Task::perform(
        async move {
            crate::app::path_op_jobs::run_blocking(move || read_import(&path))
                .await
                .unwrap_or_else(|why| Err(ChecklistNotice::ImportFailed(why)))
        },
        move |read| {
            let msg = ChecklistMsg::ImportRead {
                read: Some(read),
                waiting,
            };
            message(project_id, msg)
        },
    )
}

/// `project_export_checklist_csv {document, path}`: the checklist written
/// to `path` as CSV; answers with the file and the number of items.
pub(crate) fn project_export_checklist_csv(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match named_project(app, args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let Some(path) = path_operand(args) else {
        return refused(needs_path(ids::CHECKLIST_OPERAND_CSV));
    };
    let rows = match app.checklist_rows(id) {
        Ok(rows) => rows,
        Err(problem) => return refused(problem_refusal(&problem)),
    };
    let answer = json!({"path": path.display().to_string(), "rows": rows});
    let task = app.write_checklist_csv(id, path);
    (CommandOutcome::Answered(answer), task)
}

/// `project_import_checklist {document, path}`: this project's checklist
/// started from the one of the project at `path`.
pub(crate) fn project_import_checklist(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match named_project(app, args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let Some(path) = path_operand(args) else {
        return refused(needs_path(ids::CHECKLIST_OPERAND_IMPORT));
    };
    let command = match AppCommandId::new("project_import_checklist") {
        Ok(command) => command,
        Err(reason) => {
            let reason = Translatable::new(ids::CHECKLIST_REFUSAL_COMMAND).arg("reason", reason);
            return refused(reason);
        }
    };
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    (
        CommandOutcome::Deferred { ticket },
        import_task(id, path, Some(waiting)),
    )
}

impl Signex {
    /// How many items project `id`'s checklist has; refused without one.
    fn checklist_rows(&self, id: ProjectId) -> Result<usize, ChecklistProblem> {
        self.document_state
            .project_by_id(id)
            .ok_or(ChecklistProblem::NotStarted)?
            .data
            .design_checklist()?
            .map(|checklist| checklist.items.len())
            .ok_or(ChecklistProblem::NotStarted)
    }

    /// More ▸ Export CSV…: the save dialog, then the write.
    pub(super) fn export_csv_dialog(&mut self, project_id: ProjectId) -> Task<Message> {
        self.checklist_tab(project_id).more_open = false;
        let name = self
            .document_state
            .project_by_id(project_id)
            .map(|project| format!("{}-checklist.csv", project.data.name))
            .unwrap_or_else(|| "checklist.csv".into());
        let i18n = &self.ui_state.i18n;
        let dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::CHECKLIST_EXPORT_TITLE)
            .set_file_name(name)
            .filter_in(i18n, ids::FILE_FILTER_CSV_EXT, &["csv"]);
        Task::perform(async move { dialog.save_file().await }, move |path| {
            message(project_id, ChecklistMsg::ExportPicked(path))
        })
    }

    /// Write project `id`'s checklist as CSV to `path`, off the UI thread;
    /// the export writer reports the outcome in Messages.
    pub(super) fn write_checklist_csv(&mut self, id: ProjectId, path: PathBuf) -> Task<Message> {
        let checklist = self
            .document_state
            .project_by_id(id)
            .and_then(|project| project.data.design_checklist().ok().flatten());
        let Some(checklist) = checklist else {
            return Task::none();
        };
        let log = format!(
            "Wrote {} (design checklist, {} items)",
            path.display(),
            checklist.items.len()
        );
        self.write_export(
            path,
            checklist.to_csv().into_bytes(),
            "CSV",
            log,
            None,
            Vec::new(),
        )
    }

    /// Import from another project…: the open dialog, then the read.
    pub(super) fn import_dialog(&mut self, project_id: ProjectId) -> Task<Message> {
        let i18n = &self.ui_state.i18n;
        let dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::CHECKLIST_IMPORT_TITLE)
            .filter_in(i18n, ids::FILE_FILTER_PROJECT, &["snxprj"]);
        Task::perform(
            async move {
                let path = dialog.pick_file().await?;
                let read = crate::app::path_op_jobs::run_blocking(move || read_import(&path))
                    .await
                    .unwrap_or_else(|why| Err(ChecklistNotice::ImportFailed(why)));
                Some(read)
            },
            move |read| {
                let msg = ChecklistMsg::ImportRead {
                    read,
                    waiting: None,
                };
                message(project_id, msg)
            },
        )
    }

    /// The project to import from was read: start this checklist from its
    /// items, or say why not.
    pub(super) fn checklist_import_read(
        &mut self,
        project_id: ProjectId,
        read: Result<Imported, ChecklistNotice>,
        waiting: Option<WaitingCommand>,
    ) -> Task<Message> {
        self.answer_checklist_wait(project_id, waiting, move |app| match read {
            Ok(imported) => {
                let command = ProjectChecklistCommand::Import {
                    from: imported.from,
                    template: imported.template,
                    items: imported.items,
                    stamp: app.checklist_stamp(),
                };
                edit_in_tab(app, project_id, &command)
            }
            Err(notice) => {
                let reason = import_refusal(&notice);
                app.checklist_tab(project_id).notice = Some(notice);
                CommandOutcome::Disabled { reason }
            }
        })
    }
}
