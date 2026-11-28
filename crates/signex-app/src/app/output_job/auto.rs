//! Automation (#1299, mockup row 114, "Automation"): a profile runs without
//! a click when a file it watches is saved (after its wait, so a burst of
//! saves runs once), keeps its outputs up to date by running only the ones
//! made from the saved file, or runs before its project closes. Auto runs
//! read the saved job file and work in the background; they never ask, and
//! a headless run starts none. Rules: docs/OUTPUT_JOBS.md, "Automation".

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;

use iced::Task;
use signex_types::output_job::{BeforeClose, OutputJob, OutputKind, RunTrigger};

use super::OutputJobMsg;
use super::document::list_jobs;
use super::pipeline::StartRun;
use super::run::RunAsk;
use crate::app::output_job::file::read_output_job;
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

/// The shortest wait before a run that found its job busy tries again.
const BUSY_RETRY_SECONDS: u64 = 5;

/// What automation knows: the saved content of the jobs read this session,
/// the waits after saves, and the outputs out of date since their last run.
#[derive(Default)]
pub struct AutoRuns {
    known: BTreeMap<PathBuf, OutputJob>,
    waits: HashMap<(PathBuf, String), u64>,
    generation: u64,
    stale: BTreeMap<PathBuf, BTreeSet<String>>,
}

impl AutoRuns {
    /// Keep `job` as the saved content of the file at `path`.
    pub(crate) fn remember(&mut self, path: &Path, job: &OutputJob) {
        self.known.insert(path.to_path_buf(), job.clone());
    }

    /// How many of `among` are out of date.
    pub fn stale_count(&self, job: &Path, among: &[String]) -> usize {
        self.stale.get(job).map_or(0, |stale| {
            among.iter().filter(|n| stale.contains(*n)).count()
        })
    }

    fn arm(&mut self, job: &Path, profile: &str) -> u64 {
        self.generation += 1;
        self.waits
            .insert((job.to_path_buf(), profile.to_string()), self.generation);
        self.generation
    }

    fn is_newest(&self, job: &Path, profile: &str, generation: u64) -> bool {
        self.waits
            .get(&(job.to_path_buf(), profile.to_string()))
            .is_some_and(|newest| *newest == generation)
    }
}

/// What a saved file feeds: a board feeds the board outputs, a sheet the
/// schematic ones.
fn feeds(saved: &Path, kind: OutputKind) -> bool {
    let ext = saved
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    if ext.eq_ignore_ascii_case("snxpcb") {
        kind.is_board()
    } else if ext.eq_ignore_ascii_case("snxsch") {
        !kind.is_board()
    } else {
        false
    }
}

/// What happened to automation, namespaced under `OutputJobMsg::Auto`.
#[derive(Debug, Clone)]
pub enum AutoMsg {
    /// The jobs beside a saved document were read.
    JobsScanned {
        saved: PathBuf,
        jobs: Vec<(PathBuf, Result<OutputJob, String>)>,
    },
    /// The wait after a save of a watched file ended.
    WaitEnded {
        job: PathBuf,
        profile: String,
        generation: u64,
        among: Option<Vec<String>>,
        trigger: RunTrigger,
    },
}

impl From<AutoMsg> for Message {
    fn from(msg: AutoMsg) -> Self {
        Message::from(OutputJobMsg::Auto(msg))
    }
}

/// The `.snxout` files of `folder`, each read. Blocking.
fn scan(folder: &Path) -> Vec<(PathBuf, Result<OutputJob, String>)> {
    list_jobs(folder)
        .unwrap_or_default()
        .into_iter()
        .map(|path| {
            let read = read_output_job(&path).map_err(|e| e.to_string());
            (path, read)
        })
        .collect()
}

fn wait_then(seconds: u64, msg: AutoMsg) -> Task<Message> {
    if seconds == 0 {
        return Task::done(Message::from(msg));
    }
    Task::perform(
        async move { tokio::time::sleep(Duration::from_secs(seconds)).await },
        move |()| Message::from(msg),
    )
}

impl Signex {
    /// A document was saved: read the jobs of its project off the UI thread,
    /// to mark their outputs out of date and start the profiles watching it.
    pub(crate) fn auto_runs_after_save(&mut self, saved: &Path) -> Task<Message> {
        if self.headless_answers().is_some() {
            return Task::none();
        }
        let Some(project) = self.document_state.document_project(saved) else {
            return Task::none();
        };
        let folder = project.dir().to_path_buf();
        let saved = saved.to_path_buf();
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || scan(&folder)),
            move |jobs| {
                Message::from(AutoMsg::JobsScanned {
                    saved,
                    jobs: jobs.unwrap_or_default(),
                })
            },
        )
    }

    pub(crate) fn dispatch_auto_message(&mut self, msg: AutoMsg) -> Task<Message> {
        match msg {
            AutoMsg::JobsScanned { saved, jobs } => self.auto_jobs_scanned(&saved, jobs),
            AutoMsg::WaitEnded {
                job,
                profile,
                generation,
                among,
                trigger,
            } => self.auto_wait_ended(job, profile, generation, among, trigger),
        }
    }

    fn auto_jobs_scanned(
        &mut self,
        saved: &Path,
        jobs: Vec<(PathBuf, Result<OutputJob, String>)>,
    ) -> Task<Message> {
        let name = saved
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut tasks = Vec::new();
        for (path, read) in jobs {
            let Ok(job) = read else {
                continue;
            };
            let auto = &mut self.output_jobs_mut().auto;
            auto.remember(&path, &job);
            let fed: Vec<String> = job
                .outputs
                .iter()
                .filter(|o| o.known_kind().is_some_and(|kind| feeds(saved, kind)))
                .map(|o| o.name.clone())
                .collect();
            auto.stale
                .entry(path.clone())
                .or_default()
                .extend(fed.iter().cloned());
            for profile in &job.profiles {
                let triggers = &profile.triggers;
                if !triggers.watches(&name) {
                    continue;
                }
                let (among, trigger) = match triggers.on_save {
                    true => (None, RunTrigger::OnSave),
                    false => {
                        let ticked: Vec<String> =
                            fed.iter().filter(|n| profile.ticks(n)).cloned().collect();
                        if ticked.is_empty() {
                            continue;
                        }
                        (Some(ticked), RunTrigger::KeepUpToDate)
                    }
                };
                let generation = auto.arm(&path, &profile.name);
                tasks.push(wait_then(
                    u64::from(triggers.wait_seconds),
                    AutoMsg::WaitEnded {
                        job: path.clone(),
                        profile: profile.name.clone(),
                        generation,
                        among,
                        trigger,
                    },
                ));
            }
        }
        Task::batch(tasks)
    }

    /// The wait ended with no newer save: run the profile from the saved
    /// job. A job whose run is going waits again.
    fn auto_wait_ended(
        &mut self,
        job: PathBuf,
        profile: String,
        generation: u64,
        among: Option<Vec<String>>,
        trigger: RunTrigger,
    ) -> Task<Message> {
        let auto = &self.output_jobs().auto;
        if !auto.is_newest(&job, &profile, generation) {
            return Task::none();
        }
        let Some(saved) = auto.known.get(&job).cloned() else {
            return Task::none();
        };
        if self.output_jobs().runs.running(&job).is_some() {
            let msg = AutoMsg::WaitEnded {
                job,
                profile,
                generation,
                among,
                trigger,
            };
            return wait_then(BUSY_RETRY_SECONDS, msg);
        }
        let ask = RunAsk {
            profile: Some(profile),
            among,
            trigger: Some(trigger),
            ..RunAsk::default()
        };
        self.start_job_run(StartRun {
            job_path: job,
            job: saved,
            ask,
            trigger,
            waiting: None,
        })
    }

    /// A run wrote `written` of `job`: they are up to date.
    pub(crate) fn note_job_outputs_written(&mut self, job: &Path, written: &[String]) {
        if let Some(stale) = self.output_jobs_mut().auto.stale.get_mut(job) {
            for name in written {
                stale.remove(name);
            }
        }
    }

    /// The project `project` closes: each profile set to run before close
    /// with out-of-date ticked outputs runs now (Always), from what the run
    /// captures at its start, or is named in a notice (Ask).
    pub(crate) fn auto_runs_before_close(&mut self, project: ProjectId) -> Task<Message> {
        if self.headless_answers().is_some() {
            return Task::none();
        }
        let Some(dir) = self
            .document_state
            .project_by_id(project)
            .map(|p| p.dir().to_path_buf())
        else {
            return Task::none();
        };
        let auto = &self.output_jobs().auto;
        let mut due: Vec<(PathBuf, OutputJob, String, BeforeClose, usize)> = Vec::new();
        for (path, job) in &auto.known {
            if path.parent() != Some(dir.as_path()) {
                continue;
            }
            for profile in &job.profiles {
                let stale = auto.stale_count(path, &profile.outputs);
                if profile.triggers.before_close != BeforeClose::Off && stale > 0 {
                    due.push((
                        path.clone(),
                        job.clone(),
                        profile.name.clone(),
                        profile.triggers.before_close,
                        stale,
                    ));
                }
            }
        }
        let mut tasks = Vec::new();
        for (path, job, profile, when, stale) in due {
            match when {
                BeforeClose::Always => tasks.push(self.start_job_run(StartRun {
                    job_path: path,
                    job,
                    ask: RunAsk {
                        profile: Some(profile),
                        trigger: Some(RunTrigger::BeforeClose),
                        ..RunAsk::default()
                    },
                    trigger: RunTrigger::BeforeClose,
                    waiting: None,
                })),
                BeforeClose::Ask => self.refuse_export(
                    signex_i18n::Translatable::new(signex_i18n::ids::NOTICE_OUTPUT_JOB_OUT_OF_DATE)
                        .arg("job", path.display().to_string())
                        .arg("profile", profile)
                        .arg("count", stale.to_string()),
                ),
                BeforeClose::Off => {}
            }
        }
        Task::batch(tasks)
    }
}
