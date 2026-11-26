//! An Export Job run: pick or name the job, read it off the UI thread, build
//! its outputs from the project's scope, write them off the UI thread, and
//! answer the caller (the Messages panel, or the CLI / agent that waits).

use std::path::{Path, PathBuf};

use iced::Task;
use signex_i18n::{Translatable, ids};
use signex_types::output_job::{OUTPUT_JOB_EXTENSION, OutputJob, read_output_job};

use super::write::{JobReport, build_and_write};
use super::{JobRead, JobWritten, OutputJobMsg};
use crate::app::agent::WaitingCommand;
use crate::app::command::CommandOutcome;
use crate::app::handlers::menu::export::job_outputs::JobWork;
use crate::app::{Message, Signex};

/// The project a job belongs to and the open sheet its outputs are built
/// from.
pub(super) struct JobScope {
    pub(super) project_dir: PathBuf,
    /// The project's file name, which names the default files.
    pub(super) stem: String,
    /// The project's root sheet when it is open, else its first open sheet.
    pub(super) source: PathBuf,
}

/// A run ready to write: the report so far, the folder it must stay in and
/// the outputs.
struct PreparedJob {
    report: JobReport,
    project_dir: PathBuf,
    work: Vec<JobWork>,
}

/// Read the job file off the UI thread; the outcome comes back as
/// `OutputJobMsg::JobRead`.
pub(super) fn read_job(
    job: PathBuf,
    only: Option<String>,
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
                only,
                waiting,
                result,
            })))
        },
    )
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
            source,
        })
    }

    /// The variant `named` names in `job`'s project, as the project spells
    /// it; the project's active variant when `named` is `None`.
    fn output_job_variant(&self, job: &Path, named: Option<&str>) -> Result<String, String> {
        let project = self
            .document_state
            .document_project(job)
            .ok_or_else(|| format!("{} is not in the folder of an open project", job.display()))?;
        let Some(named) = named else {
            return Ok(project.data.active_variant_name().to_string());
        };
        project
            .data
            .defined_variant(named)
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
        read_job(job, None, None)
    }

    /// The job file landed: build its outputs and write them off the UI
    /// thread, or answer why nothing is written.
    pub(super) fn handle_output_job_read(&mut self, read: JobRead) -> Task<Message> {
        let JobRead {
            job,
            only,
            waiting,
            result,
        } = read;
        let prepared = result.and_then(|loaded| self.prepare_job(&job, &loaded, only.as_deref()));
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(reason) => return self.finish_output_job(job, waiting, Err(reason)),
        };
        if let Err(e) = self.refuse_while_project_moves(&job, "run the Export Job") {
            return self.finish_output_job(job, waiting, Err(e.to_string()));
        }
        self.begin_project_write(&job);
        let PreparedJob {
            mut report,
            project_dir,
            work,
        } = prepared;
        let folder = PathBuf::from(&report.folder);
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    build_and_write(&folder, Some(&project_dir), work)
                })
                .await
                .map_err(|e| format!("the run did not finish: {e}"))
                .and_then(|written| written)
            },
            move |written| {
                let result = written.map(|(outputs, said)| {
                    report.outputs = outputs;
                    report.warnings.extend(said);
                    report
                });
                Message::from(OutputJobMsg::JobWritten(Box::new(JobWritten {
                    target: job,
                    waiting,
                    result,
                })))
            },
        )
    }

    /// The report so far, the project folder and the work of `job`.
    fn prepare_job(
        &self,
        job_path: &Path,
        job: &OutputJob,
        only: Option<&str>,
    ) -> Result<PreparedJob, String> {
        let scope = self.output_job_scope(job_path)?;
        let plan = job.plan(&scope.stem, only).map_err(|e| e.to_string())?;
        let variant = self.output_job_variant(job_path, job.variant.as_deref())?;
        let job_folder = job_path.parent().unwrap_or_else(|| Path::new(""));
        let folder = job.output_folder(job_folder).map_err(|e| e.to_string())?;
        let build = self.output_job_work(&scope.source, &plan.outputs, &variant)?;
        Ok(PreparedJob {
            report: JobReport {
                job: job_path.display().to_string(),
                folder: folder.display().to_string(),
                variant,
                outputs: Vec::new(),
                skipped: plan.skipped,
                warnings: build.warnings,
            },
            project_dir: scope.project_dir,
            work: build.work,
        })
    }

    pub(super) fn handle_output_job_written(&mut self, written: JobWritten) -> Task<Message> {
        self.end_project_write(&written.target);
        self.finish_output_job(written.target, written.waiting, written.result)
    }

    /// Say how the run ended: in the Messages panel (and the notice card for
    /// a run the person started), and to the waiting CLI or agent call.
    fn finish_output_job(
        &mut self,
        job: PathBuf,
        waiting: Option<WaitingCommand>,
        result: Result<JobReport, String>,
    ) -> Task<Message> {
        match &result {
            Ok(report) => log_report("Export Job", report),
            Err(reason) if waiting.is_none() => self.refuse_output_job(&job, reason),
            Err(reason) => crate::diagnostics::log_warning(format!(
                "Export Job {} wrote nothing: {reason}",
                job.display()
            )),
        }
        let Some(waiting) = waiting else {
            return Task::none();
        };
        let outcome = match result {
            Ok(report) => CommandOutcome::Answered(
                serde_json::to_value(&report).unwrap_or(serde_json::Value::Null),
            ),
            Err(reason) => super::operands::job_refusal(&job, reason),
        };
        self.finish_waiting_command(waiting, &job, move |_| outcome)
    }

    /// The notice card and a Messages line: nothing of `job` was written.
    fn refuse_output_job(&mut self, job: &Path, reason: &str) {
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
