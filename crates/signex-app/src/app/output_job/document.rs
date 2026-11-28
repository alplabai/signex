//! An Export Job as a document (#1299, mockup row 114): the tab opens on a
//! draft read off the UI thread, an edit marks the job's path dirty (the
//! tab's dot, Save All and the quit gate read it), and Ctrl+S writes the
//! draft crash-safely. One write of a job is in flight at a time; an edit
//! made meanwhile is written after it. A closed tab with unsaved edits keeps
//! its draft until it is saved or discarded.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use iced::Task;
use signex_types::output_job::{OUTPUT_JOB_EXTENSION, OutputJob, encode_output_job};

use super::OutputJobMsg;
use crate::app::agent::WaitingCommand;
use crate::app::command::CommandOutcome;
use crate::app::output_job::file::{read_output_job, write_output_job};
use crate::app::{Message, Signex, TabInfo, TabKind};

/// One job's write in flight, and whether the draft changed since.
#[derive(Debug, Default)]
pub(crate) struct SaveSlot {
    pub(crate) in_flight: bool,
    pub(crate) again: bool,
    /// Calls answered once the write lands.
    pub(crate) waiting: Vec<WaitingCommand>,
}

pub(crate) type SaveSlots = HashMap<PathBuf, SaveSlot>;

/// Why a job's files are listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListPurpose {
    /// View ▸ Panels ▸ Export Jobs: open the project's job.
    Open,
    /// New Export Job: pick a free name.
    New,
}

/// What happened to a job file, namespaced under `OutputJobMsg::Document`.
#[derive(Debug, Clone)]
pub enum JobDocMsg {
    /// A job file was read for its tab.
    Loaded {
        path: PathBuf,
        result: Result<OutputJob, String>,
    },
    /// A draft write finished: what was written, or why not.
    Written {
        path: PathBuf,
        saved: Box<OutputJob>,
        result: Result<(), String>,
        hold: Option<crate::autosave::SaveHold>,
    },
    /// The `.snxout` files of a project folder were listed.
    Listed {
        folder: PathBuf,
        stem: String,
        purpose: ListPurpose,
        result: Result<Vec<PathBuf>, String>,
    },
    /// A command's read or write of a job file finished: its answer.
    CommandDone {
        job: PathBuf,
        waiting: Box<WaitingCommand>,
        result: Result<serde_json::Value, String>,
    },
}

impl From<JobDocMsg> for Message {
    fn from(msg: JobDocMsg) -> Self {
        Message::from(OutputJobMsg::Document(msg))
    }
}

/// The `.snxout` files directly in `folder`, sorted. Blocking.
pub(crate) fn list_jobs(folder: &Path) -> Result<Vec<PathBuf>, String> {
    let mut jobs = Vec::new();
    let entries = std::fs::read_dir(folder)
        .map_err(|e| format!("could not list {}: {e}", folder.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let is_job = path.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case(OUTPUT_JOB_EXTENSION));
        if is_job {
            jobs.push(path);
        }
    }
    jobs.sort();
    Ok(jobs)
}

/// The first of `<stem>.snxout`, `<stem>-2.snxout`, ... in `folder` that
/// neither `taken` nor `drafts` names, compared without case.
pub(crate) fn free_job_path(
    folder: &Path,
    stem: &str,
    taken: &[PathBuf],
    drafts: &[PathBuf],
) -> PathBuf {
    let used = |candidate: &Path| {
        taken.iter().chain(drafts).any(|p| {
            p.file_name()
                .zip(candidate.file_name())
                .is_some_and(|(a, b)| {
                    a.to_string_lossy()
                        .eq_ignore_ascii_case(&b.to_string_lossy())
                })
                && p.parent() == candidate.parent()
        })
    };
    let mut n = 1;
    loop {
        let name = match n {
            1 => format!("{stem}.{OUTPUT_JOB_EXTENSION}"),
            n => format!("{stem}-{n}.{OUTPUT_JOB_EXTENSION}"),
        };
        let candidate = folder.join(name);
        if !used(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

impl Signex {
    /// Open `job` in its tab, or focus the tab that shows it. The file is
    /// read off the UI thread unless its draft is held already.
    pub(crate) fn open_output_job_tab(&mut self, job: PathBuf) -> Task<Message> {
        if let Some(idx) = self
            .document_state
            .tabs
            .iter()
            .position(|tab| tab.kind.is_output_job() && tab.path == job)
        {
            self.focus_tab_at(idx);
            return Task::none();
        }
        let title = self.output_job_tab_title(&job);
        let project_id = self.document_state.project_for_path(&job).map(|p| p.id);
        self.document_state.tabs.push(TabInfo {
            title,
            path: job.clone(),
            project_id,
            kind: TabKind::OutputJob,
        });
        let idx = self.document_state.tabs.len() - 1;
        self.focus_tab_at(idx);
        let held = self.document_state.output_jobs.contains_key(&job);
        let ui = self.output_jobs_mut().tabs.entry(job.clone()).or_default();
        if held {
            return Task::none();
        }
        ui.loading = true;
        let reading = job.clone();
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || {
                read_output_job(&reading).map_err(|e| e.to_string())
            }),
            move |result| {
                Message::from(JobDocMsg::Loaded {
                    path: job,
                    result: result.and_then(|read| read),
                })
            },
        )
    }

    fn focus_tab_at(&mut self, idx: usize) {
        if idx != self.document_state.active_tab {
            self.park_active_schematic_session();
            self.document_state.active_tab = idx;
            self.sync_active_tab();
        }
    }

    /// The tab's title: `fab.snxout — Export Job` (mockup row 167).
    pub(crate) fn output_job_tab_title(&self, job: &Path) -> String {
        let file = job
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.ui_state.i18n.message_plain(
            signex_i18n::ids::OUTPUT_JOB_TAB_TITLE,
            &[("file", signex_i18n::Arg::from(file))],
        )
    }

    /// View ▸ Panels ▸ Export Jobs (mockup row 114): focus the Export Job
    /// tab of the project in view, else open the project's job (the first
    /// by name), else a new one.
    pub(crate) fn open_project_output_job(&mut self) -> Task<Message> {
        let dir = self
            .document_state
            .active_document_project()
            .map(|project| project.dir().to_path_buf());
        let shown = dir.as_deref().and_then(|dir| {
            self.document_state
                .tabs
                .iter()
                .find(|tab| tab.kind.is_output_job() && tab.path.parent() == Some(dir))
                .map(|tab| tab.path.clone())
        });
        match shown {
            Some(job) => self.open_output_job_tab(job),
            None => self.list_project_jobs(ListPurpose::Open),
        }
    }

    /// List the `.snxout` files of the project in view off the UI thread:
    /// for View ▸ Panels ▸ Export Jobs (`Open`) or New Export Job (`New`).
    pub(crate) fn list_project_jobs(&mut self, purpose: ListPurpose) -> Task<Message> {
        let Some(project) = self.document_state.active_document_project() else {
            crate::diagnostics::log_info("Export Jobs: open a project first");
            return Task::none();
        };
        let folder = project.dir().to_path_buf();
        let stem = project
            .path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "outputs".to_string());
        let listing = folder.clone();
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || list_jobs(&listing)),
            move |result| {
                Message::from(JobDocMsg::Listed {
                    folder,
                    stem,
                    purpose,
                    result: result.and_then(|jobs| jobs),
                })
            },
        )
    }

    /// Route a job document message.
    pub(crate) fn dispatch_job_doc_message(&mut self, msg: JobDocMsg) -> Task<Message> {
        match msg {
            JobDocMsg::Loaded { path, result } => {
                self.output_job_loaded(path, result);
                Task::none()
            }
            JobDocMsg::Written {
                path,
                saved,
                result,
                hold: _hold,
            } => self.output_job_written(path, *saved, result),
            JobDocMsg::Listed {
                folder,
                stem,
                purpose,
                result,
            } => self.project_jobs_listed(&folder, &stem, purpose, result),
            JobDocMsg::CommandDone {
                job,
                waiting,
                result,
            } => self.job_command_done(job, *waiting, result),
        }
    }

    fn output_job_loaded(&mut self, path: PathBuf, result: Result<OutputJob, String>) {
        let ui = self.output_jobs_mut().tabs.entry(path.clone()).or_default();
        ui.loading = false;
        match result {
            Ok(job) => {
                ui.read_error = None;
                self.output_jobs_mut().auto.remember(&path, &job);
                self.document_state.output_jobs.entry(path).or_insert(job);
            }
            Err(why) => {
                crate::diagnostics::log_warning(format!(
                    "Export Job {} could not be read: {why}",
                    path.display()
                ));
                ui.read_error = Some(why);
            }
        }
    }

    /// The project's jobs are listed: open the first, or start a new one.
    fn project_jobs_listed(
        &mut self,
        folder: &Path,
        stem: &str,
        purpose: ListPurpose,
        result: Result<Vec<PathBuf>, String>,
    ) -> Task<Message> {
        let jobs = match result {
            Ok(jobs) => jobs,
            Err(why) => {
                crate::diagnostics::log_warning(format!("Export Jobs: {why}"));
                return Task::none();
            }
        };
        if purpose == ListPurpose::Open
            && let Some(first) = jobs.first()
        {
            return self.open_output_job_tab(first.clone());
        }
        let drafts: Vec<PathBuf> = self.document_state.output_jobs.keys().cloned().collect();
        let path = free_job_path(folder, stem, &jobs, &drafts);
        self.new_output_job_draft(path)
    }

    /// A new, empty job at `path`, unsaved, in its tab: Save writes it.
    pub(crate) fn new_output_job_draft(&mut self, path: PathBuf) -> Task<Message> {
        self.document_state
            .output_jobs
            .insert(path.clone(), OutputJob::default());
        self.document_state.mark_document_dirty(&path);
        self.open_output_job_tab(path)
    }

    /// Ctrl+S or Save All on `job`: write its draft. A draft that cannot be
    /// saved (two outputs of one name, ...) is refused and named.
    pub(crate) fn write_output_job_draft(
        &mut self,
        job: &Path,
        waiting: Option<WaitingCommand>,
    ) -> Task<Message> {
        let Some(draft) = self.document_state.output_jobs.get(job).cloned() else {
            return self.answer_job_write(job, waiting, Err("no open Export Job".to_string()));
        };
        if let Err(why) = encode_output_job(&draft) {
            let why = why.to_string();
            self.refuse_output_job(job, &why);
            return self.answer_job_write(job, waiting, Err(why));
        }
        if let Err(why) = self.refuse_while_project_moves(job, "save") {
            return self.answer_job_write(job, waiting, Err(why.to_string()));
        }
        let slot = self
            .output_jobs_mut()
            .saves
            .entry(job.to_path_buf())
            .or_default();
        slot.waiting.extend(waiting);
        if slot.in_flight {
            slot.again = true;
            return Task::none();
        }
        slot.in_flight = true;
        let hold = self.document_state.autosave.hold_for_save();
        let path = job.to_path_buf();
        let writing = path.clone();
        let saved = draft.clone();
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || {
                write_output_job(&writing, &draft).map_err(|e| e.to_string())
            }),
            move |result| {
                Message::from(JobDocMsg::Written {
                    path,
                    saved: Box::new(saved),
                    result: result.and_then(|written| written),
                    hold: Some(hold),
                })
            },
        )
    }

    /// A write landed: the draft reads clean when it still equals what was
    /// written; a draft that changed meanwhile is written again.
    fn output_job_written(
        &mut self,
        path: PathBuf,
        saved: OutputJob,
        result: Result<(), String>,
    ) -> Task<Message> {
        let slot = self
            .output_jobs_mut()
            .saves
            .remove(&path)
            .unwrap_or_default();
        if let Err(why) = &result {
            crate::diagnostics::log_warning(format!(
                "Export Job {} was not saved: {why}",
                path.display()
            ));
            self.refuse_output_job(&path, why);
        } else {
            log::info!("Export Job saved: {}", path.display());
            self.output_jobs_mut().auto.remember(&path, &saved);
            let unchanged = self.document_state.output_jobs.get(&path) == Some(&saved);
            if unchanged {
                self.document_state.mark_document_clean(&path);
            }
            // A clean draft with no tab was loaded for a command only.
            if unchanged && !self.has_output_job_tab(&path) {
                self.document_state.output_jobs.remove(&path);
            }
        }
        let mut tasks: Vec<Task<Message>> = slot
            .waiting
            .into_iter()
            .map(|waiting| {
                let outcome = match &result {
                    Ok(()) => CommandOutcome::Answered(serde_json::json!({
                        "job": path.display().to_string(),
                        "saved": true,
                    })),
                    Err(why) => super::operands::job_refusal(&path, why.clone()),
                };
                self.finish_waiting_command(waiting, &path, move |_| outcome)
            })
            .collect();
        if slot.again && result.is_ok() {
            tasks.push(self.write_output_job_draft(&path, None));
        }
        // A run asked for before the save starts once the last write lands;
        // a failed write drops it.
        if !(slot.again && result.is_ok()) {
            let pending = self
                .output_jobs_mut()
                .tabs
                .get_mut(&path)
                .and_then(|ui| ui.run_after_save.take());
            if let Some(selected) = pending
                && result.is_ok()
            {
                tasks.push(self.start_tab_run(&path, selected));
            }
        }
        Task::batch(tasks)
    }

    fn answer_job_write(
        &mut self,
        job: &Path,
        waiting: Option<WaitingCommand>,
        result: Result<(), String>,
    ) -> Task<Message> {
        let Some(waiting) = waiting else {
            return Task::none();
        };
        let outcome = match result {
            Ok(()) => {
                CommandOutcome::Answered(serde_json::json!({"job": job.display().to_string()}))
            }
            Err(why) => super::operands::job_refusal(job, why),
        };
        self.finish_waiting_command(waiting, job, move |_| outcome)
    }

    /// Close the Export Job tab at `idx`. With `park_dirty`, a draft with
    /// unsaved edits stays held (its dot moves to Save All and the quit
    /// gate); otherwise the draft goes with the tab.
    pub(crate) fn close_output_job_tab(
        &mut self,
        idx: usize,
        park_dirty: bool,
        keep_session: bool,
    ) -> Task<Message> {
        let path = self.document_state.tabs.remove(idx).path;
        self.output_jobs_mut().tabs.remove(&path);
        let dirty = self.document_state.is_document_dirty(&path);
        if !(park_dirty && dirty) {
            self.document_state.output_jobs.remove(&path);
        }
        let len = self.document_state.tabs.len();
        let active = self.document_state.active_tab;
        if active > 0 && (idx < active || active >= len) {
            self.document_state.active_tab = active - 1;
        }
        if keep_session {
            self.settle_document_tiling();
        } else {
            self.sync_active_tab();
        }
        self.refresh_panel_ctx();
        Task::none()
    }

    /// A job file was renamed or saved under a new name: its draft, view
    /// state and dirty mark follow it.
    pub(crate) fn rekey_output_job(&mut self, from: &Path, to: &Path) {
        if let Some(draft) = self.document_state.output_jobs.remove(from) {
            self.document_state
                .output_jobs
                .insert(to.to_path_buf(), draft);
        }
        let ui = &mut self.ui_state.output_jobs;
        if let Some(view) = ui.tabs.remove(from) {
            ui.tabs.insert(to.to_path_buf(), view);
        }
        let title = self.output_job_tab_title(to);
        for tab in &mut self.document_state.tabs {
            if tab.kind.is_output_job() && tab.path == to {
                tab.title = title.clone();
            }
        }
    }
}
