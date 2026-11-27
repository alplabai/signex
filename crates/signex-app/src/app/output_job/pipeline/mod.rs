//! An Export Job run as a pipeline (#1299, mockup row 114, "Run pipeline"):
//! Validate (ERC, DRC and the BOM checks, off the UI thread), Generate (one
//! output at a time off the UI thread, so the flow lights node by node),
//! Package (every file written at once, a zip beside them) and Summary.
//! Nothing is written unless every output builds. The run's state is the
//! flow the tab shows (`view/flow.rs`). Rules: docs/OUTPUT_JOBS.md, "Runs".

mod finish;
mod package;
mod stages;
mod start;
mod validate;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use iced::Task;
use signex_types::output_job::{OnErrors, OnWarnings, OutputGroup, OutputKind, RunTrigger};

pub use package::Packaged;
pub use start::StartRun;
pub(crate) use validate::ChecksJob;
pub use validate::ValidateOutcome;

use super::write::JobReport;
use crate::app::agent::WaitingCommand;
use crate::app::handlers::menu::export::job_outputs::{JobFile, JobWork};
use crate::app::{Message, Signex};

/// Where a node of the run flow is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeState {
    #[default]
    Pending,
    Running,
    Done,
    /// Done, with warnings.
    Warning,
    Failed,
}

/// One output of the flow: its state, how many files it made and its log.
#[derive(Debug, Clone)]
pub struct FlowNode {
    pub name: String,
    pub kind: OutputKind,
    pub state: NodeState,
    pub files: usize,
    pub log: Vec<String>,
}

impl FlowNode {
    pub fn group(&self) -> OutputGroup {
        self.kind.group()
    }
}

/// Where a run is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStage {
    Validating,
    /// Validate found a problem and the profile asks the person.
    Asking,
    Generating,
    Packaging,
    Done,
}

/// Where a stopped run stopped, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stopped {
    /// `validate`, `package`, or an output's name.
    pub at: String,
    pub reason: String,
}

/// How the files of a run are laid out on disk.
#[derive(Debug, Clone)]
pub(crate) struct Layout {
    pub project_dir: PathBuf,
    pub job_dir: PathBuf,
    /// The output folder before a release folder is added.
    pub folder: PathBuf,
    /// The release folder's name, when the profile writes one.
    pub release: Option<String>,
    /// The group of each output, by name, when files go in group folders.
    pub groups: Option<BTreeMap<String, OutputGroup>>,
    pub zip: bool,
    pub clear: bool,
}

/// One run, from its start until the next run of the same job replaces it.
pub struct JobRun {
    pub id: u64,
    pub job: PathBuf,
    pub profile: Option<String>,
    pub variant: String,
    pub trigger: RunTrigger,
    pub stage: RunStage,
    /// The Validate node, when the run validates.
    pub validate: Option<FlowNode>,
    pub nodes: Vec<FlowNode>,
    pub package: NodeState,
    pub checks: Option<ValidateOutcome>,
    pub on_errors: OnErrors,
    pub on_warnings: OnWarnings,
    /// Local time the run started, `YYYY-MM-DD HH:MM:SS`.
    pub started: String,
    pub started_at: Instant,
    /// How long the run took, once it ended.
    pub seconds: u32,
    pub report: JobReport,
    pub stopped: Option<Stopped>,
    /// The files the Gerber Viewer reads, relative to the folder written.
    pub viewable: Vec<String>,
    pub(crate) layout: Layout,
    /// Each node's work until it starts.
    pub(crate) work: Vec<Option<JobWork>>,
    pub(crate) built: Vec<JobFile>,
    pub(crate) checks_job: Option<ChecksJob>,
    pub(crate) waiting: Option<WaitingCommand>,
}

impl JobRun {
    pub fn is_running(&self) -> bool {
        self.stage != RunStage::Done
    }

    /// Steps done and steps in all: Validate, each output and Package.
    pub fn progress(&self) -> (usize, usize) {
        let mut total = self.nodes.len() + 1;
        let mut done = self
            .nodes
            .iter()
            .filter(|node| matches!(node.state, NodeState::Done | NodeState::Warning))
            .count();
        if let Some(node) = &self.validate {
            total += 1;
            done += usize::from(matches!(node.state, NodeState::Done | NodeState::Warning));
        }
        done += usize::from(self.package == NodeState::Done);
        (done, total)
    }

    /// Outputs done and outputs in all.
    pub fn outputs_done(&self) -> (usize, usize) {
        let done = self
            .nodes
            .iter()
            .filter(|node| matches!(node.state, NodeState::Done | NodeState::Warning))
            .count();
        (done, self.nodes.len())
    }

    /// Whether the run asks the person to go on past Validate.
    pub fn is_asking(&self) -> bool {
        self.stage == RunStage::Asking
    }
}

/// Every run this session, the newest of each job kept after it ends.
#[derive(Default)]
pub struct RunBook {
    next: u64,
    runs: BTreeMap<u64, JobRun>,
}

impl RunBook {
    fn next_id(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    pub fn get(&self, run: u64) -> Option<&JobRun> {
        self.runs.get(&run)
    }

    pub(crate) fn get_mut(&mut self, run: u64) -> Option<&mut JobRun> {
        self.runs.get_mut(&run)
    }

    /// The newest run of `job`, running or ended.
    pub fn latest(&self, job: &Path) -> Option<&JobRun> {
        self.runs.values().rev().find(|run| run.job == job)
    }

    /// The run of `job` going now.
    pub fn running(&self, job: &Path) -> Option<&JobRun> {
        self.latest(job).filter(|run| run.is_running())
    }

    /// Keep `run` and forget the ended runs of its job before it.
    fn insert(&mut self, run: JobRun) {
        let job = run.job.clone();
        self.runs
            .retain(|_, kept| kept.job != job || kept.is_running());
        self.runs.insert(run.id, run);
    }
}

/// One output's files and warnings, made off the UI thread.
#[derive(Clone)]
pub struct Built {
    pub(crate) files: Vec<JobFile>,
    pub(crate) warnings: Vec<String>,
}

impl std::fmt::Debug for Built {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Built({} files)", self.files.len())
    }
}

/// What happened to a run, namespaced under `OutputJobMsg::Run`.
#[derive(Debug, Clone)]
pub enum RunMsg {
    /// Validate finished.
    Validated {
        run: u64,
        outcome: Arc<ValidateOutcome>,
    },
    /// The output of flow node `node` was made, or why not.
    OutputBuilt {
        run: u64,
        node: usize,
        result: Arc<Result<Built, String>>,
    },
    /// Every file was written, or why none was.
    Packaged {
        run: u64,
        result: Arc<Result<Packaged, String>>,
    },
    /// The run was kept in its job file's history, or why not.
    Recorded {
        job: PathBuf,
        result: Result<(), String>,
    },
}

impl From<RunMsg> for Message {
    fn from(msg: RunMsg) -> Self {
        Message::from(super::OutputJobMsg::Run(msg))
    }
}

impl Signex {
    /// Route a run message.
    pub(crate) fn dispatch_job_run_message(&mut self, msg: RunMsg) -> Task<Message> {
        match msg {
            RunMsg::Validated { run, outcome } => {
                self.job_run_validated(run, Arc::unwrap_or_clone(outcome))
            }
            RunMsg::OutputBuilt { run, node, result } => {
                self.job_output_built(run, node, Arc::unwrap_or_clone(result))
            }
            RunMsg::Packaged { run, result } => {
                self.job_run_packaged(run, Arc::unwrap_or_clone(result))
            }
            RunMsg::Recorded { job, result } => {
                if let Err(why) = result {
                    crate::diagnostics::log_warning(format!(
                        "Export Job {}: the run was not kept in its history: {why}",
                        job.display()
                    ));
                }
                Task::none()
            }
        }
    }
}
