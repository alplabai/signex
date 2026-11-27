//! The steps of a run after its start: Validate and the policy that reads
//! it (stop, ask, or go on), each output made in turn, then Package. A run
//! that stops, or is stopped, writes nothing.

use std::sync::Arc;

use iced::Task;
use signex_types::output_job::{OnErrors, OnWarnings, RunTrigger};

use super::{Built, NodeState, RunMsg, RunStage, Stopped, ValidateOutcome};
use crate::app::{Message, Signex};

/// Where a stopped Validate stops.
pub(super) const AT_VALIDATE: &str = "validate";

impl Signex {
    /// Run the checks of `run` off the UI thread.
    pub(super) fn start_job_validate(&mut self, run: u64) -> Task<Message> {
        let Some(job) = self.output_jobs_mut().runs.get_mut(run) else {
            return Task::none();
        };
        let Some(checks) = job.checks_job.take() else {
            return self.generate_next_output(run);
        };
        if let Some(node) = job.validate.as_mut() {
            node.state = NodeState::Running;
        }
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || checks.run()),
            move |outcome| {
                let outcome = outcome.unwrap_or_else(|why| ValidateOutcome {
                    errors: 1,
                    lines: vec![format!("the checks did not finish: {why}")],
                    failing: vec!["validate"],
                    ..ValidateOutcome::default()
                });
                Message::from(RunMsg::Validated {
                    run,
                    outcome: Arc::new(outcome),
                })
            },
        )
    }

    /// Validate landed: stop, ask or go on, by the profile's policy. A run
    /// with no person to ask (a trigger, a command) stops where it would
    /// ask.
    pub(super) fn job_run_validated(
        &mut self,
        run: u64,
        outcome: ValidateOutcome,
    ) -> Task<Message> {
        let Some(job) = self.output_jobs_mut().runs.get_mut(run) else {
            return Task::none();
        };
        if job.stage != RunStage::Validating {
            return Task::none();
        }
        let can_ask = job.waiting.is_none() && job.trigger == RunTrigger::Manual;
        if let Some(node) = job.validate.as_mut() {
            node.state = if outcome.errors > 0 {
                NodeState::Failed
            } else if outcome.warnings > 0 {
                NodeState::Warning
            } else {
                NodeState::Done
            };
            node.log = outcome.lines.clone();
        }
        let errors = outcome.errors > 0;
        let warnings = outcome.warnings > 0;
        let summary = outcome.summary();
        let (stop, ask) = match (errors, warnings) {
            (true, _) => match job.on_errors {
                OnErrors::Stop => (true, false),
                OnErrors::Ask => (!can_ask, can_ask),
            },
            (false, true) => match job.on_warnings {
                OnWarnings::Continue => (false, false),
                OnWarnings::Stop => (true, false),
                OnWarnings::Ask => (!can_ask, can_ask),
            },
            (false, false) => (false, false),
        };
        job.report.checks = Some(super::super::write::CheckCounts {
            errors: outcome.errors,
            warnings: outcome.warnings,
        });
        job.checks = Some(outcome);
        if ask {
            job.stage = RunStage::Asking;
            return Task::none();
        }
        if stop {
            let reason = format!("{summary}; this profile stops, so nothing was generated");
            return self.stop_job_run(run, AT_VALIDATE, reason);
        }
        self.generate_next_output(run)
    }

    /// The person answered the question Validate raised.
    pub(crate) fn answer_job_run(&mut self, run: u64, go_on: bool) -> Task<Message> {
        let Some(job) = self.output_jobs().runs.get(run) else {
            return Task::none();
        };
        if job.stage != RunStage::Asking {
            return Task::none();
        }
        if go_on {
            return self.generate_next_output(run);
        }
        let summary = job
            .checks
            .as_ref()
            .map(ValidateOutcome::summary)
            .unwrap_or_default();
        self.stop_job_run(
            run,
            AT_VALIDATE,
            format!("{summary}; stopped at the question"),
        )
    }

    /// Make the next output that has not started off the UI thread, or
    /// package the run when every output is made.
    pub(super) fn generate_next_output(&mut self, run: u64) -> Task<Message> {
        let Some(job) = self.output_jobs_mut().runs.get_mut(run) else {
            return Task::none();
        };
        job.stage = RunStage::Generating;
        let next = job.work.iter().position(Option::is_some);
        let Some(node) = next else {
            return self.package_job_run(run);
        };
        let Some(work) = job.work[node].take() else {
            return Task::none();
        };
        if let Some(flow) = job.nodes.get_mut(node) {
            flow.state = NodeState::Running;
        }
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || work.render()),
            move |result| {
                let result = result
                    .map_err(|why| format!("the output did not finish: {why}"))
                    .and_then(|made| made)
                    .map(|(files, warnings)| Built { files, warnings });
                Message::from(RunMsg::OutputBuilt {
                    run,
                    node,
                    result: Arc::new(result),
                })
            },
        )
    }

    /// One output was made: light its node and make the next; a failed one
    /// stops the run with nothing written.
    pub(super) fn job_output_built(
        &mut self,
        run: u64,
        node: usize,
        result: Result<Built, String>,
    ) -> Task<Message> {
        let Some(job) = self.output_jobs_mut().runs.get_mut(run) else {
            return Task::none();
        };
        if job.stage != RunStage::Generating {
            return Task::none();
        }
        let Some(flow) = job.nodes.get_mut(node) else {
            return Task::none();
        };
        match result {
            Ok(built) => {
                flow.files = built.files.len();
                flow.state = match built.warnings.is_empty() {
                    true => NodeState::Done,
                    false => NodeState::Warning,
                };
                flow.log = built.warnings.clone();
                job.report.warnings.extend(built.warnings);
                job.built.extend(built.files);
                self.generate_next_output(run)
            }
            Err(reason) => {
                flow.state = NodeState::Failed;
                flow.log = vec![reason.clone()];
                let at = flow.name.clone();
                self.stop_job_run(run, &at, reason)
            }
        }
    }

    /// Stop `run` where it is: nothing more is made and nothing is written.
    pub(crate) fn stop_job_run(&mut self, run: u64, at: &str, reason: String) -> Task<Message> {
        let Some(job) = self.output_jobs_mut().runs.get_mut(run) else {
            return Task::none();
        };
        if !job.is_running() {
            return Task::none();
        }
        job.stopped = Some(Stopped {
            at: at.to_string(),
            reason,
        });
        job.work.clear();
        job.built.clear();
        job.checks_job = None;
        for node in &mut job.nodes {
            if matches!(node.state, NodeState::Pending | NodeState::Running) {
                node.state = NodeState::Pending;
            }
        }
        self.end_job_run(run)
    }

    /// Stop (the toolbar's Stop, `stop_output_job`): the run of `job` going
    /// now ends; a package already writing finishes its files.
    pub(crate) fn stop_job_run_of(&mut self, job: &std::path::Path) -> Task<Message> {
        let Some(run) = self.output_jobs().runs.running(job) else {
            return Task::none();
        };
        if run.stage == RunStage::Packaging {
            return Task::none();
        }
        let id = run.id;
        self.stop_job_run(id, "stop", "stopped by request".to_string())
    }
}
