//! Generate design notebook's events: the options dialog, the start of a
//! run (shared by the dialog and the command's operands) and its answer.

use iced::Task;
use signex_i18n::{Translatable, ids, t};
use signex_notebook::generate::{
    GenerateOptions, generated_path, revision_of, variant_of, variants_of,
};
use signex_types::check::CheckReport;
use signex_types::project::BASE_VARIANT;

use super::request::request_from_args;
use super::{
    GenerateDialog, GenerateFinished, GenerateOutcome, GenerateRequest, NotebookGenerateMsg, job,
};
use crate::app::agent::WaitingCommand;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

const ID: &str = "generate_design_notebook";

type Ran = (CommandOutcome, Task<Message>);

/// `generate_design_notebook` run with operands (the command line, an
/// agent, a script): answered with the summary once the run lands
/// ([`CommandOutcome::Deferred`]), or refused at once with the reason.
pub(crate) fn generate_design_notebook(app: &mut Signex, args: &CommandArgs) -> Ran {
    let refused = |reason: String| (refusal(reason), Task::none());
    let command = match AppCommandId::new(ID) {
        Ok(command) => command,
        Err(reason) => return refused(reason),
    };
    let request = match request_from_args(app, args) {
        Ok(request) => request,
        Err(reason) => return refused(reason),
    };
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    match app.start_notebook_generate(request, Some(waiting)) {
        Ok(task) => (CommandOutcome::Deferred { ticket }, task),
        Err(reason) => refused(reason),
    }
}

/// The answer to a run with operands: the summary, or why nothing was made.
pub(super) fn answer(outcome: Result<GenerateOutcome, String>) -> CommandOutcome {
    match outcome {
        Ok(done) => match serde_json::to_value(&done.notebook.summary) {
            Ok(value) => CommandOutcome::Answered(value),
            Err(error) => refusal(format!("the summary could not be written: {error}")),
        },
        Err(why) => refusal(why),
    }
}

/// The command refused: "The design report was not generated: <why>".
pub(super) fn refusal(why: String) -> CommandOutcome {
    CommandOutcome::Disabled {
        reason: Translatable::new(ids::DIALOG_NB_GENERATE_FAILED).arg("why", why),
    }
}

impl Signex {
    pub(crate) fn dispatch_notebook_generate_message(
        &mut self,
        msg: NotebookGenerateMsg,
    ) -> Task<Message> {
        use NotebookGenerateMsg as M;
        match msg {
            M::DialogOpened => self.open_notebook_generate_dialog(),
            M::VariantPicked(variant) => self.with_generate_dialog(|d| d.variant = variant),
            M::TemplatePicked(template) => {
                self.with_generate_dialog(|d| d.options.template = template)
            }
            M::DocumentNumberEdited(text) => {
                self.with_generate_dialog(|d| d.document_number = text)
            }
            M::RevisionEdited(text) => self.with_generate_dialog(|d| d.document_revision = text),
            M::SectionToggled(section) => self.with_generate_dialog(|d| {
                d.options = std::mem::take(&mut d.options).toggled(section);
            }),
            M::ChecksSourcePicked(source) => {
                self.with_generate_dialog(|d| d.options.checks = source)
            }
            M::GenerateClicked => return self.generate_from_dialog(),
            M::DialogCancelled => self.ui_state.notebook_generate.dialog = None,
            M::Generated(finished) => return self.finish_notebook_generate(*finished),
            M::ReportSaved {
                project_id,
                text,
                result,
            } => self.generated_report_saved(project_id, &text, result),
        }
        Task::none()
    }

    /// Change the open dialog; a change clears the last run's problem. A run
    /// under way keeps its options.
    fn with_generate_dialog(&mut self, change: impl FnOnce(&mut GenerateDialog)) {
        if let Some(dialog) = self.ui_state.notebook_generate.dialog.as_mut()
            && !dialog.running
        {
            change(dialog);
            dialog.problem = None;
        }
    }

    /// The options dialog for the project in view, with the drawn defaults.
    fn open_notebook_generate_dialog(&mut self) {
        let Some(project) = self.document_state.active_document_project() else {
            let line = t!(self.ui_state.i18n, ids::DIALOG_NB_GENERATE_NO_PROJECT);
            crate::diagnostics::log_info(line.into_string());
            return;
        };
        let data = &project.data;
        let dialog = GenerateDialog {
            project: project.path.clone(),
            project_file: project
                .path
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned()),
            revision: revision_of(data),
            requirements: data.requirements.len(),
            variants: variants_of(data),
            variant: variant_of(data, None).unwrap_or_else(|_| BASE_VARIANT.to_string()),
            options: GenerateOptions::default(),
            document_number: String::new(),
            document_revision: String::new(),
            running: false,
            problem: None,
        };
        self.interaction_state.context_menu = None;
        self.ui_state.notebook_generate.dialog = Some(dialog);
    }

    fn generate_from_dialog(&mut self) -> Task<Message> {
        let Some(dialog) = self.ui_state.notebook_generate.dialog.as_ref() else {
            return Task::none();
        };
        if !dialog.can_generate() {
            return Task::none();
        }
        let request = dialog.request();
        let started = self.start_notebook_generate(request, None);
        let Some(dialog) = self.ui_state.notebook_generate.dialog.as_mut() else {
            return Task::none();
        };
        match started {
            Ok(task) => {
                dialog.running = true;
                dialog.problem = None;
                task
            }
            Err(why) => {
                dialog.problem = Some(why);
                Task::none()
            }
        }
    }

    /// Start a run of `request` off the UI thread; refused at once when its
    /// project is not open. `waiting` is the command call it answers.
    pub(crate) fn start_notebook_generate(
        &mut self,
        request: GenerateRequest,
        waiting: Option<WaitingCommand>,
    ) -> Result<Task<Message>, String> {
        let project = self
            .document_state
            .projects
            .iter()
            .find(|project| project.path == request.project)
            .ok_or_else(|| format!("`{ID}`: {} is not open", request.project.display()))?;
        let job = self.notebook_generate_job(request.clone(), project);
        Ok(Task::perform(
            async move {
                match tokio::task::spawn_blocking(move || job::run(job)).await {
                    Ok((ran, outcome)) => (ran, outcome),
                    Err(error) => (Vec::new(), Err(format!("the run did not finish: {error}"))),
                }
            },
            move |(ran, outcome)| {
                let finished = GenerateFinished {
                    request: request.clone(),
                    waiting: waiting.clone(),
                    ran,
                    outcome,
                };
                Message::from(NotebookGenerateMsg::Generated(Box::new(finished)))
            },
        ))
    }

    /// A run landed: its check runs are kept as the newest of their checks,
    /// then the command call is answered, or the dialog shows the result.
    fn finish_notebook_generate(&mut self, finished: GenerateFinished) -> Task<Message> {
        let GenerateFinished {
            request,
            waiting,
            ran,
            outcome,
        } = finished;
        // The checks it ran are kept whether or not the report was written
        // (#3021).
        let runs = &mut self.document_state.check_runs;
        let made = outcome
            .as_ref()
            .map(|done| done.ran.as_slice())
            .unwrap_or(&[]);
        for (scope, report) in ran.iter().chain(made) {
            let run = runs.record(&report.check, scope, report.keys());
            let numbered = CheckReport {
                run,
                ..report.clone()
            };
            runs.keep_report(scope, numbered);
        }
        match waiting {
            Some(waiting) => {
                self.finish_waiting_command(waiting, &request.project, move |_| answer(outcome))
            }
            None => {
                self.show_generated(&request.project, outcome);
                Task::none()
            }
        }
    }

    /// The dialog's run landed: the report is held unsaved for Save (its
    /// path in the dirty set), or the dialog says why there is none.
    fn show_generated(
        &mut self,
        project: &std::path::Path,
        outcome: Result<GenerateOutcome, String>,
    ) {
        let i18n = &self.ui_state.i18n;
        match outcome {
            Ok(done) => {
                let path = generated_path();
                let line = t!(
                    i18n,
                    ids::DIALOG_NB_GENERATE_DONE,
                    "pages" => done.notebook.summary.pages as u32,
                    "path" => path.as_str()
                );
                crate::diagnostics::log_info(line.into_string());
                self.ui_state.notebook_generate.dialog = None;
                self.hold_generated_report(project, done.notebook);
            }
            Err(why) => {
                let line = t!(i18n, ids::DIALOG_NB_GENERATE_FAILED, "why" => why.as_str());
                crate::diagnostics::log_warning(line.into_string());
                if let Some(dialog) = self.ui_state.notebook_generate.dialog.as_mut() {
                    dialog.running = false;
                    dialog.problem = Some(why);
                }
            }
        }
    }
}
