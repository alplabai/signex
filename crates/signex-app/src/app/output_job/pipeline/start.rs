//! The start of a run: the plan of the profile asked for, its outputs'
//! work built from the project's scope as it is in memory, its checks
//! captured, and the first step started. Everything a later step needs is
//! taken here, so a run goes on after its project closes (before close,
//! mockup row 114).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use iced::Task;
use signex_types::output_job::{
    OutputJob, RunRequest, RunTrigger, expand_naming, folder_below, release_folder_name,
};

use super::{FlowNode, JobRun, Layout, NodeState, RunStage};
use crate::app::agent::WaitingCommand;
use crate::app::output_job::run::RunAsk;
use crate::app::output_job::write::JobReport;
use crate::app::{Message, Signex};

/// A run to start: the job as read (or as its tab holds it) and what is
/// asked of it.
pub struct StartRun {
    pub(crate) job_path: PathBuf,
    pub(crate) job: OutputJob,
    pub(crate) ask: RunAsk,
    pub(crate) trigger: RunTrigger,
    pub(crate) waiting: Option<WaitingCommand>,
}

impl Signex {
    /// Start the run `start` asks for; a run that cannot start writes
    /// nothing and says why.
    pub(crate) fn start_job_run(&mut self, start: StartRun) -> Task<Message> {
        match self.prepare_job_run(&start) {
            Ok(run) => {
                let id = run.id;
                self.begin_project_write(&start.job_path);
                let first = match run.checks_job.is_some() {
                    true => RunStage::Validating,
                    false => RunStage::Generating,
                };
                let mut run = run;
                run.stage = first;
                run.waiting = start.waiting;
                self.output_jobs_mut().runs.insert(run);
                self.show_run_flow(&start.job_path);
                match first {
                    RunStage::Validating => self.start_job_validate(id),
                    _ => self.generate_next_output(id),
                }
            }
            Err(reason) => {
                self.finish_unstarted_run(start.job_path, start.trigger, start.waiting, reason)
            }
        }
    }

    /// Plan the run, build its outputs' work and capture its checks.
    fn prepare_job_run(&mut self, start: &StartRun) -> Result<JobRun, String> {
        let job_path = &start.job_path;
        if self.output_jobs().runs.running(job_path).is_some() {
            return Err("a run of this job is going; stop it or wait for it".to_string());
        }
        super::super::operands::check_job_file(job_path)?;
        self.refuse_while_project_moves(job_path, "run the Export Job")
            .map_err(|e| e.to_string())?;
        let scope = self.output_job_scope(job_path)?;
        let ask = &start.ask;
        let request = RunRequest {
            profile: ask.profile.as_deref(),
            only: ask.only.as_deref(),
            selected: ask.selected,
            among: ask.among.as_deref(),
        };
        let first = start
            .job
            .run_plan(&scope.stem, &request)
            .map_err(|e| e.to_string())?;
        let variant = self.output_job_variant(job_path, first.variant.as_deref())?;
        // A profile's naming pattern renames the default files.
        let stem = first
            .naming
            .as_deref()
            .map(|pattern| expand_naming(pattern, &scope.stem, &scope.rev, &variant))
            .unwrap_or_else(|| scope.stem.clone());
        let run_plan = match stem == scope.stem {
            true => first,
            false => start
                .job
                .run_plan(&stem, &request)
                .map_err(|e| e.to_string())?,
        };
        let job_dir = job_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        let relative = folder_below(&run_plan.folder).ok_or_else(|| {
            format!(
                "the folder {:?} must be a folder beside the job file, with no `..` and no drive",
                run_plan.folder
            )
        })?;
        let folder = job_dir.join(relative);
        let build = self.output_job_work(&scope.source, &run_plan.plan.outputs, &variant)?;
        let checks_job = run_plan.validate.then(|| {
            let board = run_plan
                .plan
                .outputs
                .iter()
                .any(|output| output.kind.is_board())
                .then(|| {
                    self.document_state
                        .document_project(&scope.source)
                        .and_then(Self::project_board)
                })
                .flatten();
            self.validate_job(
                &scope.source,
                board.as_deref(),
                (build.bom_errors, build.bom_warnings),
            )
        });
        let now = chrono::Local::now();
        let packaging = &run_plan.packaging;
        let release = packaging
            .release_folder
            .then(|| release_folder_name(&stem, &scope.rev, &now.format("%Y-%m-%d").to_string()));
        let groups = packaging.group_folders.then(|| {
            run_plan
                .plan
                .outputs
                .iter()
                .map(|output| (output.name.clone(), output.kind.group()))
                .collect::<BTreeMap<_, _>>()
        });
        let nodes: Vec<FlowNode> = run_plan
            .plan
            .outputs
            .iter()
            .map(|output| FlowNode {
                name: output.name.clone(),
                kind: output.kind,
                state: NodeState::Pending,
                files: 0,
                log: Vec::new(),
            })
            .collect();
        let validate = checks_job.as_ref().map(|_| FlowNode {
            name: "Validate".to_string(),
            kind: signex_types::output_job::OutputKind::Bom,
            state: NodeState::Pending,
            files: 0,
            log: Vec::new(),
        });
        let id = self.output_jobs_mut().runs.next_id();
        Ok(JobRun {
            id,
            job: job_path.clone(),
            profile: run_plan.profile.clone(),
            variant: variant.clone(),
            trigger: start.trigger,
            stage: RunStage::Generating,
            validate,
            nodes,
            package: NodeState::Pending,
            checks: None,
            on_errors: run_plan.on_errors,
            on_warnings: run_plan.on_warnings,
            started: now.format("%Y-%m-%d %H:%M:%S").to_string(),
            started_at: Instant::now(),
            seconds: 0,
            report: JobReport {
                job: job_path.display().to_string(),
                folder: folder.display().to_string(),
                variant,
                skipped: run_plan.plan.skipped.clone(),
                warnings: build.warnings,
                profile: run_plan.profile,
                ..JobReport::default()
            },
            stopped: None,
            viewable: Vec::new(),
            layout: Layout {
                project_dir: scope.project_dir,
                job_dir,
                folder,
                release,
                groups,
                zip: packaging.zip,
                clear: packaging.clear_folder,
            },
            work: build.work.into_iter().map(Some).collect(),
            built: Vec::new(),
            checks_job,
            waiting: None,
        })
    }
}
