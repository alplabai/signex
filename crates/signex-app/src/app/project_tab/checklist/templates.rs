//! The user's own checklist templates (mockup row 206): Save as template…
//! writes the checklist's items under a name to the library file in the
//! user's config folder; the empty tab lists them under "Your templates" to
//! start from; Reset from template puts the template's items back. The file
//! is read and written off the UI thread, atomically, and a file this build
//! cannot read is never written over. Catalog commands:
//! `project_checklist_templates`, `project_save_checklist_template` and
//! `project_reset_checklist`; `project_start_checklist` takes `user:<name>`.

use std::path::PathBuf;

use iced::Task;
use serde_json::{Value, json};
use signex_engine::ProjectChecklistCommand;
use signex_i18n::{Translatable, ids};
use signex_types::project::{
    CHECKLIST_TEMPLATES, ChecklistProblem, ChecklistStage, ChecklistTemplate, SavedTemplate,
    TEMPLATE_LIBRARY_FILE, TemplateRef,
};

use super::commands::{Ran, edit, edit_in_tab, named_project, refused, required};
use super::library_file::{read_library_file, save_into_library_file};
use super::words::problem_refusal;
use super::{ChecklistMsg, ChecklistNotice, SavedTemplates, message};
use crate::app::agent::WaitingCommand;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// Why the library has no file to read or write.
const NO_CONFIG_FOLDER: &str = "there is no config folder to keep checklist templates in";

/// What a read of the library is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryUse {
    /// Show the list in the empty tab.
    Show,
    /// Answer `project_checklist_templates`.
    List,
    /// Start the checklist from the saved template of this name.
    Start(String),
    /// Reset the checklist from the saved template of this name.
    Reset(String),
}

/// The library file's path. Resolves the config folder: run it off the UI
/// thread.
fn library_path() -> Result<PathBuf, String> {
    crate::config_root::config_root()
        .map(|root| root.join(TEMPLATE_LIBRARY_FILE))
        .ok_or_else(|| NO_CONFIG_FOLDER.to_string())
}

/// Read the library off the UI thread, for `then`.
pub(super) fn read_library(
    project_id: ProjectId,
    then: LibraryUse,
    waiting: Option<WaitingCommand>,
) -> Task<Message> {
    Task::perform(
        async move {
            crate::app::path_op_jobs::run_blocking(|| {
                read_library_file(&library_path()?).map(|library| library.templates)
            })
            .await
            .and_then(std::convert::identity)
        },
        move |read| {
            let msg = ChecklistMsg::LibraryRead {
                read,
                then,
                waiting,
            };
            message(project_id, msg)
        },
    )
}

/// Add `template` to the library file off the UI thread.
fn save_template(
    project_id: ProjectId,
    template: SavedTemplate,
    waiting: Option<WaitingCommand>,
) -> Task<Message> {
    let name = template.name.clone();
    Task::perform(
        async move {
            crate::app::path_op_jobs::run_blocking(move || {
                save_into_library_file(&library_path()?, template).map(|library| library.templates)
            })
            .await
            .and_then(std::convert::identity)
        },
        move |saved| {
            let msg = ChecklistMsg::TemplateSaved {
                saved,
                name,
                waiting,
            };
            message(project_id, msg)
        },
    )
}

/// The templates as `project_checklist_templates` answers them.
fn templates_json(saved: &[SavedTemplate]) -> Value {
    let stages = |count: &dyn Fn(ChecklistStage) -> usize| -> Value {
        ChecklistStage::ALL
            .into_iter()
            .map(|stage| (stage.key().to_string(), json!(count(stage))))
            .collect::<serde_json::Map<String, Value>>()
            .into()
    };
    let builtin: Vec<Value> = CHECKLIST_TEMPLATES
        .into_iter()
        .map(|template| {
            let items = template.items();
            json!({
                "template": template.key(),
                "items": items.len(),
                "stages": stages(&|stage| items.iter().filter(|i| i.stage == stage).count()),
            })
        })
        .collect();
    let own: Vec<Value> = saved
        .iter()
        .map(|template| {
            json!({
                "template": template.key(),
                "name": template.name,
                "saved_from": template.saved_from,
                "items": template.items.len(),
                "stages": stages(&|stage| template.count(stage)),
            })
        })
        .collect();
    json!({"builtin": builtin, "saved": own})
}

/// The catalog id `id`, for a command that waits for its file work.
fn command_id(id: &str) -> Result<AppCommandId, Translatable> {
    AppCommandId::new(id).map_err(|error| {
        Translatable::new(ids::CHECKLIST_REFUSAL_COMMAND).arg("reason", error.to_string())
    })
}

/// Wait for the library read of `then`, for the command `id`.
fn deferred_read(
    app: &mut Signex,
    project_id: ProjectId,
    id: &str,
    args: &CommandArgs,
    then: LibraryUse,
) -> Ran {
    let command = match command_id(id) {
        Ok(command) => command,
        Err(reason) => return refused(reason),
    };
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    let task = read_library(project_id, then, Some(waiting));
    (CommandOutcome::Deferred { ticket }, task)
}

/// `project_checklist_templates {document}`: the built-in templates and the
/// user's saved ones, each with its item count per stage.
pub(crate) fn project_checklist_templates(app: &mut Signex, args: &CommandArgs) -> Ran {
    match named_project(app, args) {
        Ok(id) => deferred_read(
            app,
            id,
            "project_checklist_templates",
            args,
            LibraryUse::List,
        ),
        Err(reason) => refused(reason),
    }
}

/// `project_save_checklist_template {document, name}`: the checklist's
/// items saved under `name` (one of that name is replaced).
pub(crate) fn project_save_checklist_template(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match named_project(app, args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    let name = match required(args, "name", ids::CHECKLIST_OPERAND_TEMPLATE_NAME) {
        Ok(name) => name,
        Err(reason) => return refused(reason),
    };
    let template = match app.checklist_as_template(id, name) {
        Ok(template) => template,
        Err(problem) => return refused(problem_refusal(&problem)),
    };
    let command = match command_id("project_save_checklist_template") {
        Ok(command) => command,
        Err(reason) => return refused(reason),
    };
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    let task = save_template(id, template, Some(waiting));
    (CommandOutcome::Deferred { ticket }, task)
}

/// `project_start_checklist` with `template=user:<name>`: the library is
/// read first.
pub(super) fn start_from_saved(
    app: &mut Signex,
    id: ProjectId,
    args: &CommandArgs,
    name: String,
) -> Ran {
    deferred_read(
        app,
        id,
        "project_start_checklist",
        args,
        LibraryUse::Start(name),
    )
}

/// `project_reset_checklist {document}`: the template the checklist started
/// from, its items put back in place of every item, as one undo step.
pub(crate) fn project_reset_checklist(app: &mut Signex, args: &CommandArgs) -> Ran {
    let id = match named_project(app, args) {
        Ok(id) => id,
        Err(reason) => return refused(reason),
    };
    match app.checklist_template_ref(id) {
        Ok(TemplateRef::Builtin(template)) => {
            let command = app.reset_command(template);
            edit(app, id, &command)
        }
        Ok(TemplateRef::Saved(name)) => deferred_read(
            app,
            id,
            "project_reset_checklist",
            args,
            LibraryUse::Reset(name),
        ),
        Err(problem) => refused(problem_refusal(&problem)),
    }
}

impl Signex {
    /// Read the user's saved templates into the tab.
    pub(crate) fn read_checklist_templates(&mut self, project_id: ProjectId) -> Task<Message> {
        read_library(project_id, LibraryUse::Show, None)
    }

    /// The reset of the built-in `template`, its items worded now.
    pub(super) fn reset_command(&self, template: ChecklistTemplate) -> ProjectChecklistCommand {
        ProjectChecklistCommand::Reset {
            template: template.key().to_string(),
            items: self.template_items(template),
            stamp: self.checklist_stamp(),
        }
    }

    /// The start from the saved `template`.
    pub(super) fn saved_start_command(&self, template: &SavedTemplate) -> ProjectChecklistCommand {
        ProjectChecklistCommand::Start {
            template: Some(template.key()),
            items: template.new_items(),
            stamp: self.checklist_stamp(),
        }
    }

    /// The template project `id`'s checklist started from.
    fn checklist_template_ref(&self, id: ProjectId) -> Result<TemplateRef, ChecklistProblem> {
        let project = self
            .document_state
            .project_by_id(id)
            .ok_or(ChecklistProblem::NotStarted)?;
        let checklist = project
            .data
            .design_checklist()?
            .ok_or(ChecklistProblem::NotStarted)?;
        checklist
            .template
            .as_deref()
            .and_then(TemplateRef::parse)
            .ok_or(ChecklistProblem::NoTemplate)
    }

    /// Project `id`'s checklist as a template called `name`.
    fn checklist_as_template(
        &self,
        id: ProjectId,
        name: &str,
    ) -> Result<SavedTemplate, ChecklistProblem> {
        let project = self
            .document_state
            .project_by_id(id)
            .ok_or(ChecklistProblem::NotStarted)?;
        let checklist = project
            .data
            .design_checklist()?
            .ok_or(ChecklistProblem::NotStarted)?;
        checklist.to_saved_template(name, Some(&project.data.name))
    }

    /// More ▸ Save as template…: the name box opens with the project's name.
    pub(super) fn start_template_name(&mut self, project_id: ProjectId) {
        let name = self
            .document_state
            .project_by_id(project_id)
            .map(|project| project.data.name.clone())
            .unwrap_or_default();
        let tab = self.checklist_tab(project_id);
        tab.more_open = false;
        tab.template_name = Some(name);
    }

    /// The name box's Save: the checklist is written to the library.
    pub(super) fn save_template_from_tab(&mut self, project_id: ProjectId) -> Task<Message> {
        let name = self
            .checklist_tab(project_id)
            .template_name
            .clone()
            .unwrap_or_default();
        match self.checklist_as_template(project_id, &name) {
            Ok(template) => {
                self.checklist_tab(project_id).problem = None;
                save_template(project_id, template, None)
            }
            Err(problem) => {
                self.checklist_tab(project_id).problem = Some(problem);
                Task::none()
            }
        }
    }

    /// The library was written, or not: the tab's list and notice, and the
    /// waiting command's answer.
    pub(super) fn checklist_template_saved(
        &mut self,
        project_id: ProjectId,
        saved: Result<Vec<SavedTemplate>, String>,
        name: String,
        waiting: Option<WaitingCommand>,
    ) -> Task<Message> {
        let tab = self.checklist_tab(project_id);
        let outcome = match saved {
            Ok(list) => {
                let answer = json!({
                    "name": name,
                    "template": TemplateRef::Saved(name.clone()).key(),
                    "templates": templates_json(&list)["saved"],
                });
                tab.saved = SavedTemplates::Read(list);
                tab.template_name = None;
                tab.notice = Some(ChecklistNotice::TemplateSaved(name));
                CommandOutcome::Answered(answer)
            }
            Err(why) => {
                tab.notice = Some(ChecklistNotice::TemplateNotSaved(why.clone()));
                CommandOutcome::Disabled {
                    reason: Translatable::new(ids::CHECKLIST_REFUSAL_TEMPLATE_NOT_SAVED)
                        .arg("reason", why),
                }
            }
        };
        self.answer_checklist_wait(project_id, waiting, move |_| outcome)
    }

    /// The library was read: the tab's list, then what the read was for.
    pub(super) fn checklist_library_read(
        &mut self,
        project_id: ProjectId,
        read: Result<Vec<SavedTemplate>, String>,
        then: LibraryUse,
        waiting: Option<WaitingCommand>,
    ) -> Task<Message> {
        let tab = self.checklist_tab(project_id);
        let list = match read {
            Ok(list) => {
                tab.saved = SavedTemplates::Read(list.clone());
                list
            }
            Err(why) => {
                tab.saved = SavedTemplates::Unreadable(why.clone());
                tab.notice = Some(ChecklistNotice::LibraryUnreadable(why.clone()));
                let reason =
                    Translatable::new(ids::CHECKLIST_REFUSAL_LIBRARY_UNREADABLE).arg("reason", why);
                return self.answer_checklist_wait(project_id, waiting, move |_| {
                    CommandOutcome::Disabled { reason }
                });
            }
        };
        let (name, reset) = match then {
            LibraryUse::Show => return Task::none(),
            LibraryUse::List => {
                let answer = templates_json(&list);
                return self.answer_checklist_wait(project_id, waiting, move |_| {
                    CommandOutcome::Answered(answer)
                });
            }
            LibraryUse::Start(name) => (name, false),
            LibraryUse::Reset(name) => (name, true),
        };
        let found = list.into_iter().find(|template| template.name == name);
        self.answer_checklist_wait(project_id, waiting, move |app| {
            let Some(template) = found else {
                let problem = ChecklistProblem::UnknownSavedTemplate(name);
                let reason = problem_refusal(&problem);
                app.checklist_tab(project_id).problem = Some(problem);
                return CommandOutcome::Disabled { reason };
            };
            let command = if reset {
                ProjectChecklistCommand::Reset {
                    template: template.key(),
                    items: template.new_items(),
                    stamp: app.checklist_stamp(),
                }
            } else {
                app.saved_start_command(&template)
            };
            edit_in_tab(app, project_id, &command)
        })
    }

    /// More ▸ Reset from template: the template's items back, as one undo
    /// step. A saved template is taken from the list the tab read.
    pub(super) fn reset_from_tab(&mut self, project_id: ProjectId) {
        self.checklist_tab(project_id).more_open = false;
        let command = match self.checklist_template_ref(project_id) {
            Ok(TemplateRef::Builtin(template)) => self.reset_command(template),
            Ok(TemplateRef::Saved(name)) => {
                let found = self
                    .checklist_tab_ref(project_id)
                    .and_then(|tab| tab.saved.list().iter().find(|t| t.name == name).cloned());
                match found {
                    Some(template) => ProjectChecklistCommand::Reset {
                        template: template.key(),
                        items: template.new_items(),
                        stamp: self.checklist_stamp(),
                    },
                    None => {
                        let problem = ChecklistProblem::UnknownSavedTemplate(name);
                        self.checklist_tab(project_id).problem = Some(problem);
                        return;
                    }
                }
            }
            Err(problem) => {
                self.checklist_tab(project_id).problem = Some(problem);
                return;
            }
        };
        if self.edit_checklist_from_tab(project_id, &command).is_ok() {
            let tab = self.checklist_tab(project_id);
            tab.selected = None;
            tab.drafts.clear();
        }
    }

    /// Run `run` for the command `waiting` stands for and answer it; from the
    /// tab (no command waits) just run it.
    pub(super) fn answer_checklist_wait(
        &mut self,
        project_id: ProjectId,
        waiting: Option<WaitingCommand>,
        run: impl FnOnce(&mut Self) -> CommandOutcome,
    ) -> Task<Message> {
        match waiting {
            Some(waiting) => {
                let target = self
                    .document_state
                    .project_by_id(project_id)
                    .map(|project| project.path.clone())
                    .unwrap_or_default();
                self.finish_waiting_command(waiting, &target, run)
            }
            None => {
                let _ = run(self);
                Task::none()
            }
        }
    }
}
