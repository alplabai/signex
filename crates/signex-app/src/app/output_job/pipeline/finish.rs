//! The end of a run: Package off the UI thread, then the Summary (mockup row
//! 114, "Run complete" and "Step failed"): the run is timed, said in
//! Messages, kept in its job's history when it ran from the window, and
//! answered to the CLI or agent call that waits for it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_types::output_job::{RunRecord, RunResult, RunTrigger, append_run_record};

use super::package::{Packaged, write_package};
use super::{JobRun, NodeState, RunMsg, RunStage, Stopped};
use crate::app::agent::WaitingCommand;
use crate::app::command::CommandOutcome;
use crate::app::output_job::write::JobReport;
use crate::app::{Message, Signex};

/// The name Messages gives a run.
const WHAT: &str = "Export Job";

/// `folder` relative to `base`, `/`-separated; the whole path when it is not
/// below `base`.
fn relative_text(folder: &Path, base: &Path) -> String {
    match folder.strip_prefix(base) {
        Ok(rest) => rest
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/"),
        Err(_) => folder.display().to_string(),
    }
}

/// The history record of an ended run.
fn record_of(run: &JobRun) -> RunRecord {
    let folder = Path::new(&run.report.folder);
    RunRecord {
        started: run.started.clone(),
        profile: run.profile.clone(),
        variant: run.variant.clone(),
        trigger: run.trigger,
        result: match run.stopped {
            Some(_) => RunResult::Stopped,
            None => RunResult::Complete,
        },
        outputs: count(run.report.outputs.len()),
        files: count(run.report.file_count()),
        warnings: count(run.report.warnings.len()),
        errors: count(run.checks.as_ref().map_or(0, |checks| checks.errors)),
        seconds: run.seconds,
        folder: relative_text(folder, &run.layout.job_dir),
        zip: run.report.zip.clone(),
        stopped_at: run.stopped.as_ref().map(|stopped| stopped.at.clone()),
        reason: run.stopped.as_ref().map(|stopped| stopped.reason.clone()),
        viewable: run.viewable.clone(),
        warning_lines: run.report.warnings.clone(),
        unknown: Default::default(),
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl Signex {
    /// Write every made file off the UI thread.
    pub(super) fn package_job_run(&mut self, run: u64) -> Task<Message> {
        let Some(job) = self.output_jobs_mut().runs.get_mut(run) else {
            return Task::none();
        };
        job.stage = RunStage::Packaging;
        job.package = NodeState::Running;
        let files = std::mem::take(&mut job.built);
        let layout = job.layout.clone();
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || write_package(&layout, files)),
            move |result| {
                let result = result
                    .map_err(|why| format!("the write did not finish: {why}"))
                    .and_then(|packaged| packaged);
                Message::from(RunMsg::Packaged {
                    run,
                    result: Arc::new(result),
                })
            },
        )
    }

    /// Package landed: the run is complete, or nothing was written.
    pub(super) fn job_run_packaged(
        &mut self,
        run: u64,
        result: Result<Packaged, String>,
    ) -> Task<Message> {
        let Some(job) = self.output_jobs_mut().runs.get_mut(run) else {
            return Task::none();
        };
        if job.stage != RunStage::Packaging {
            return Task::none();
        }
        match result {
            Ok(packaged) => {
                job.package = NodeState::Done;
                job.report.folder = packaged.folder.display().to_string();
                job.report.outputs = packaged.outputs;
                job.report.zip = packaged.zip;
                job.report.removed = packaged.removed;
                job.viewable = packaged.viewable;
            }
            Err(reason) => {
                job.package = NodeState::Failed;
                job.stopped = Some(Stopped {
                    at: "package".to_string(),
                    reason,
                });
            }
        }
        self.end_job_run(run)
    }

    /// The run ended, complete or stopped: time it, say so, keep it in the
    /// history (a run from the window) and answer its caller.
    pub(super) fn end_job_run(&mut self, run: u64) -> Task<Message> {
        let Some(job) = self.output_jobs_mut().runs.get_mut(run) else {
            return Task::none();
        };
        job.stage = RunStage::Done;
        job.seconds = count(job.started_at.elapsed().as_secs() as usize);
        let job_path = job.job.clone();
        let waiting = job.waiting.take();
        let trigger = job.trigger;
        let record = record_of(job);
        let written: Vec<String> = job.report.outputs.iter().map(|o| o.name.clone()).collect();
        let result: Result<JobReport, String> = match &job.stopped {
            None => Ok(job.report.clone()),
            Some(stopped) => Err(format!("stopped at {}: {}", stopped.at, stopped.reason)),
        };
        self.end_project_write(&job_path);
        self.note_job_outputs_written(&job_path, &written);
        self.show_run_end(&job_path, result.is_ok());
        let mut tasks = Vec::new();
        if waiting.is_none() && trigger != RunTrigger::Command {
            tasks.push(self.record_job_run(&job_path, record));
        }
        tasks.push(self.say_run_ended(job_path, trigger, waiting, result));
        Task::batch(tasks)
    }

    /// A run that could not start: nothing was written.
    pub(crate) fn finish_unstarted_run(
        &mut self,
        job: PathBuf,
        trigger: RunTrigger,
        waiting: Option<WaitingCommand>,
        reason: String,
    ) -> Task<Message> {
        self.say_run_ended(job, trigger, waiting, Err(reason))
    }

    /// Messages, the notice card for a run the person started with no tab
    /// to show it, and the answer to a waiting call.
    fn say_run_ended(
        &mut self,
        job: PathBuf,
        trigger: RunTrigger,
        waiting: Option<WaitingCommand>,
        result: Result<JobReport, String>,
    ) -> Task<Message> {
        let by_person = matches!(trigger, RunTrigger::Manual | RunTrigger::Selected);
        match &result {
            Ok(report) => super::super::run::log_report(WHAT, report),
            Err(reason) if waiting.is_none() && by_person && !self.has_output_job_tab(&job) => {
                self.refuse_output_job(&job, reason);
            }
            Err(reason) => crate::diagnostics::log_warning(format!(
                "{WHAT} {} wrote nothing: {reason}",
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
            Err(reason) => super::super::operands::job_refusal(&job, reason),
        };
        self.finish_waiting_command(waiting, &job, move |_| outcome)
    }

    /// Keep `record` in the history of `job`: in its open draft, written at
    /// once when the draft is clean; else read, added to and written back
    /// off the UI thread.
    fn record_job_run(&mut self, job: &Path, record: RunRecord) -> Task<Message> {
        let clean = !self.document_state.is_document_dirty(job);
        if let Some(draft) = self.document_state.output_jobs.get_mut(job) {
            draft.record_run(record);
            return match clean {
                true => self.write_output_job_draft(job, None),
                false => Task::none(),
            };
        }
        let path = job.to_path_buf();
        let reported = job.to_path_buf();
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || {
                append_run_record(&path, record).map_err(|e| e.to_string())
            }),
            move |result| {
                Message::from(RunMsg::Recorded {
                    job: reported,
                    result: result.and_then(|written| written),
                })
            },
        )
    }
}
