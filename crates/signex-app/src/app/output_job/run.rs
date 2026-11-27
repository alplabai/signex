//! An Export Job run from a key, the palette or a command: pick or name the
//! job, read it off the UI thread, then hand it to the run pipeline
//! (`pipeline/`), which builds and writes it and answers the caller (the
//! Messages panel, or the CLI / agent that waits).

use std::path::{Path, PathBuf};

use iced::Task;
use signex_i18n::{Translatable, ids};
use signex_types::output_job::{OUTPUT_JOB_EXTENSION, RunTrigger, read_output_job};

use super::pipeline::StartRun;
use super::write::JobReport;
use super::{JobRead, OutputJobMsg};
use crate::app::agent::WaitingCommand;
use crate::app::{Message, Signex};

/// The project a job belongs to and the open sheet its outputs are built
/// from.
pub(super) struct JobScope {
    pub(super) project_dir: PathBuf,
    /// The project's file name, which names the default files.
    pub(super) stem: String,
    /// The project's revision parameter, empty when it has none.
    pub(super) rev: String,
    /// The project's root sheet when it is open, else its first open sheet.
    pub(super) source: PathBuf,
}

/// What a run asks for besides the job: the profile, one output, Run
/// Selected, a subset to keep up to date, and what started it.
#[derive(Debug, Clone, Default)]
pub struct RunAsk {
    pub(crate) profile: Option<String>,
    pub(crate) only: Option<String>,
    pub(crate) selected: bool,
    pub(crate) among: Option<Vec<String>>,
    pub(crate) trigger: Option<RunTrigger>,
}

/// Read the job file off the UI thread; the outcome comes back as
/// `OutputJobMsg::JobRead`.
pub(super) fn read_job(
    job: PathBuf,
    ask: RunAsk,
    waiting: Option<WaitingCommand>,
) -> Task<Message> {
    let reading = job.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                read_output_job(&reading).map_err(|e| e.to_string())
            })
            .await
            .map_err(|e| format!("the read did not finish: {e}"))
            .and_then(|read| read)
        },
        move |result| {
            Message::from(OutputJobMsg::JobRead(Box::new(JobRead {
                job,
                ask,
                waiting,
                result,
            })))
        },
    )
}

/// The revision a project names in its parameters (`Revision` or `Rev`).
fn project_rev(parameters: &std::collections::BTreeMap<String, String>) -> String {
    parameters
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("revision") || key.eq_ignore_ascii_case("rev"))
        .map(|(_, value)| value.trim().to_string())
        .unwrap_or_default()
}

impl Signex {
    /// The project of `job` and the sheet its outputs are built from; `Err`
    /// when the job is not in an open project's folder or no schematic of
    /// that project is open.
    pub(super) fn output_job_scope(&self, job: &Path) -> Result<JobScope, String> {
        let docs = &self.document_state;
        let project = docs
            .document_project(job)
            .ok_or_else(|| format!("{} is not in the folder of an open project", job.display()))?;
        let id = project.id;
        let mut open: Vec<PathBuf> = docs
            .engines
            .keys()
            .filter(|path| docs.document_project(path).is_some_and(|p| p.id == id))
            .cloned()
            .collect();
        open.sort();
        let source = crate::app::project_sheets::project_root_sheet_path(project)
            .filter(|root| docs.engines.contains_key(root))
            .or_else(|| open.into_iter().next())
            .ok_or_else(|| {
                format!(
                    "no schematic of the project {} is open: open one first",
                    project.data.name
                )
            })?;
        let stem = project
            .path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| project.data.name.clone());
        Ok(JobScope {
            project_dir: project.dir().to_path_buf(),
            stem,
            rev: project_rev(&project.data.parameters),
            source,
        })
    }

    /// The variant `named` names in `job`'s project, as the project spells
    /// it; the project's active variant when `named` is `None`.
    pub(super) fn output_job_variant(
        &self,
        job: &Path,
        named: Option<&str>,
    ) -> Result<String, String> {
        let project = self
            .document_state
            .document_project(job)
            .ok_or_else(|| format!("{} is not in the folder of an open project", job.display()))?;
        let Some(named) = named else {
            return Ok(project.data.active_variant_name().to_string());
        };
        project
            .data
            .named_variant(named)
            .map(str::to_string)
            .ok_or_else(|| format!("the project has no variant named {named:?}"))
    }

    /// `run_output_job` without operands: the picker for a job file, opened
    /// in the active document's project folder.
    pub(super) fn pick_output_job(&mut self) -> Task<Message> {
        let i18n = &self.ui_state.i18n;
        let mut dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::FILE_DIALOG_RUN_OUTPUT_JOB)
            .filter_in(i18n, ids::FILE_FILTER_OUTPUT_JOB, &[OUTPUT_JOB_EXTENSION]);
        if let Some(project) = self.document_state.active_document_project() {
            dialog = dialog.set_directory(project.dir());
        }
        Task::perform(dialog.pick_file(), |job| {
            Message::from(OutputJobMsg::JobPicked(job))
        })
    }

    /// The person picked `job`: what the canvas shows is what is written, so
    /// the project's title-block drafts are committed first.
    pub(super) fn start_picked_output_job(&mut self, job: PathBuf) -> Task<Message> {
        if let Err(reason) = self.check_output_job_start(&job) {
            self.refuse_output_job(&job, &reason);
            return Task::none();
        }
        if let Ok(scope) = self.output_job_scope(&job) {
            self.commit_drafts_in_scope_of(Some(&scope.source));
        }
        let ask = RunAsk {
            trigger: Some(RunTrigger::Manual),
            ..RunAsk::default()
        };
        read_job(job, ask, None)
    }

    /// The job file landed: start its run, or answer why nothing is written.
    pub(super) fn handle_output_job_read(&mut self, read: JobRead) -> Task<Message> {
        let JobRead {
            job,
            ask,
            waiting,
            result,
        } = read;
        let trigger = ask.trigger.unwrap_or(match waiting {
            Some(_) => RunTrigger::Command,
            None => RunTrigger::Manual,
        });
        match result {
            Ok(loaded) => self.start_job_run(StartRun {
                job_path: job,
                job: loaded,
                ask,
                trigger,
                waiting,
            }),
            Err(reason) => self.finish_unstarted_run(job, trigger, waiting, reason),
        }
    }

    /// The notice card and a Messages line: nothing of `job` was written.
    pub(super) fn refuse_output_job(&mut self, job: &Path, reason: &str) {
        self.refuse_export(
            Translatable::new(ids::NOTICE_OUTPUT_JOB_FAILED)
                .arg("job", job.display().to_string())
                .arg("reason", reason.to_string()),
        );
    }
}

/// The Messages lines of a finished run: what was written where, each
/// warning, and each output left out.
pub(super) fn log_report(what: &str, report: &JobReport) {
    log::info!(
        "{what}: wrote {} file(s) to {}",
        report.file_count(),
        report.folder
    );
    for warning in &report.warnings {
        crate::diagnostics::log_warning(format!("{what}: {warning}"));
    }
    for name in &report.skipped {
        crate::diagnostics::log_warning(format!(
            "{what}: output {name:?} is of a kind this Signex does not write; it was left out"
        ));
    }
}
