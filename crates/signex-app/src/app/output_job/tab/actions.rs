//! The tab's actions (mockup rows 114 and 193b): Run profile and Run
//! Selected (the job is saved first, so a run reads what its file holds),
//! the question a stopped Validate asks, Show in DRC / ERC, and the
//! summary's Open folder, Copy path, the command line for CI and View in
//! Gerber Viewer.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_types::output_job::{OutputJob, RunResult, RunTrigger};

use super::JobTabMsg;
use crate::app::gerber_viewer::RunOrigin;
use crate::app::output_job::pipeline::StartRun;
use crate::app::output_job::run::RunAsk;
use crate::app::{Message, Signex};
use crate::menu_bar::MenuMessage;

/// What a run wrote that the summary acts on: its folder, the files the
/// Gerber Viewer reads and where it came from.
pub(crate) struct RunPlace {
    pub folder: PathBuf,
    pub viewable: Vec<String>,
    pub origin: RunOrigin,
    pub complete: bool,
}

/// The command line that runs `profile` of the job at `job` with no window
/// (mockup row 114, "Command line, for CI"): the sheets and board the run
/// reads are opened first, all paths relative to the project folder. It runs
/// `--headless`: the agent channel takes no file path.
pub(crate) fn command_line(
    project_file: &str,
    opens: &[String],
    job: &str,
    profile: Option<&str>,
) -> String {
    let mut lines: Vec<String> = opens
        .iter()
        .map(|path| serde_json::json!({"id": "open_document", "args": {"path": path}}).to_string())
        .collect();
    let mut args = serde_json::json!({"path": job});
    if let Some(profile) = profile {
        args["profile"] = serde_json::Value::String(profile.to_string());
    }
    lines.push(serde_json::json!({"id": "run_output_job", "args": args}).to_string());
    let quoted: Vec<String> = lines
        .iter()
        .map(|line| format!("'{}'", line.replace('\'', "'\\''")))
        .collect();
    format!(
        "printf '%s\\n' {} | signex-cli run {project_file} --headless --allow export",
        quoted.join(" ")
    )
}

impl Signex {
    /// Run profile or Run Selected: a job with unsaved edits is saved first
    /// and run when the write lands.
    pub(super) fn run_job_from_tab(&mut self, job: &Path, selected: bool) -> Task<Message> {
        if !self.document_state.output_jobs.contains_key(job) {
            return Task::none();
        }
        if self.document_state.is_document_dirty(job) {
            if let Some(ui) = self.output_jobs_mut().tabs.get_mut(job) {
                ui.run_after_save = Some(selected);
            }
            return self.write_output_job_draft(job, None);
        }
        self.start_tab_run(job, selected)
    }

    /// Start the run of the tab's shown profile from its saved draft.
    pub(crate) fn start_tab_run(&mut self, job: &Path, selected: bool) -> Task<Message> {
        let Some(draft) = self.document_state.output_jobs.get(job).cloned() else {
            return Task::none();
        };
        if let Ok(scope) = self.output_job_scope(job) {
            self.commit_drafts_in_scope_of(Some(&scope.source));
        }
        let trigger = match selected {
            true => RunTrigger::Selected,
            false => RunTrigger::Manual,
        };
        let ask = RunAsk {
            profile: self.shown_profile_name(job),
            selected,
            trigger: Some(trigger),
            ..RunAsk::default()
        };
        self.start_job_run(StartRun {
            job_path: job.to_path_buf(),
            job: draft,
            ask,
            trigger,
            waiting: None,
        })
    }

    /// The person answered the question of the run going now.
    pub(super) fn answer_job_from_tab(&mut self, job: &Path, go_on: bool) -> Task<Message> {
        let Some(run) = self.output_jobs().runs.running(job).map(|run| run.id) else {
            return Task::none();
        };
        self.answer_job_run(run, go_on)
    }

    /// Show in DRC (or ERC): the check that stopped the run opens on its
    /// document, as Run DRC / Run ERC show it.
    pub(super) fn show_job_check(&mut self, job: &Path) -> Task<Message> {
        let drc = self
            .output_jobs()
            .runs
            .latest(job)
            .and_then(|run| run.checks.as_ref())
            .is_some_and(|checks| checks.failing.contains(&"drc"));
        let target = match drc {
            true => self
                .document_state
                .document_project(job)
                .and_then(Self::project_board),
            false => self.output_job_scope(job).ok().map(|scope| scope.source),
        };
        if let Some(path) = target {
            self.activate_tab_for_path(&path);
        }
        let command = if drc { "run_drc" } else { "run_erc" };
        Task::done(Message::Menu(MenuMessage::command(command)))
    }

    /// Where the run the tab's summary shows wrote: the picked history row,
    /// else this session's newest run, else the newest history row.
    pub(crate) fn job_run_place(&self, job: &Path, row: Option<usize>) -> Option<RunPlace> {
        let draft = self.document_state.output_jobs.get(job);
        let job_dir = job.parent().unwrap_or_else(|| Path::new(""));
        let from_record = |draft: &OutputJob, index: usize| {
            let record = draft.history.get(index)?;
            Some(RunPlace {
                folder: job_dir.join(&record.folder),
                viewable: record.viewable.clone(),
                origin: RunOrigin {
                    job: job.to_path_buf(),
                    profile: record.profile.clone(),
                    variant: record.variant.clone(),
                    started: record.started.clone(),
                    folder: job_dir.join(&record.folder),
                },
                complete: record.result == RunResult::Complete,
            })
        };
        if let Some(index) = row {
            return draft.and_then(|draft| from_record(draft, index));
        }
        if let Some(run) = self
            .output_jobs()
            .runs
            .latest(job)
            .filter(|run| !run.is_running())
        {
            let folder = PathBuf::from(&run.report.folder);
            return Some(RunPlace {
                folder: folder.clone(),
                viewable: run.viewable.clone(),
                origin: RunOrigin {
                    job: job.to_path_buf(),
                    profile: run.profile.clone(),
                    variant: run.variant.clone(),
                    started: run.started.clone(),
                    folder,
                },
                complete: run.stopped.is_none(),
            });
        }
        draft.and_then(|draft| from_record(draft, 0))
    }

    fn shown_run_place(&self, job: &Path) -> Option<RunPlace> {
        let row = self
            .output_jobs()
            .tabs
            .get(job)
            .and_then(|ui| ui.history_pick);
        self.job_run_place(job, row)
    }

    /// Open folder: the folder the run wrote, in the file manager, off the
    /// update thread.
    pub(super) fn open_job_run_folder(&mut self, job: &Path) -> Task<Message> {
        let Some(place) = self.shown_run_place(job) else {
            return Task::none();
        };
        let opener = self.ui_state.io.opener;
        let folder = place.folder;
        let target = folder.clone();
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || {
                opener.view(&target).map_err(|err| err.to_string())
            }),
            move |result| match result.and_then(|opened| opened) {
                Ok(()) => Message::noop(),
                Err(why) => Message::from(JobTabMsg::FolderOpenFailed { folder, why }),
            },
        )
    }

    /// Copy path: the run's folder on the clipboard.
    pub(super) fn copy_job_run_path(&mut self, job: &Path) -> Task<Message> {
        match self.shown_run_place(job) {
            Some(place) => iced::clipboard::write(place.folder.display().to_string()).discard(),
            None => Task::none(),
        }
    }

    /// The command line for CI of the shown profile.
    pub(crate) fn job_command_line(&self, job: &Path) -> Option<String> {
        let project = self.document_state.document_project(job)?;
        let dir = project.dir();
        let relative = |path: &Path| {
            path.strip_prefix(dir)
                .unwrap_or(path)
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/")
        };
        let mut opens: Vec<String> = Vec::new();
        if let Ok(scope) = self.output_job_scope(job) {
            opens.push(relative(&scope.source));
        }
        let draft = self.document_state.output_jobs.get(job);
        let has_board_output = draft.is_some_and(|draft| {
            draft
                .outputs
                .iter()
                .any(|output| output.known_kind().is_some_and(|kind| kind.is_board()))
        });
        if has_board_output && let Some(board) = Self::project_board(project) {
            opens.push(relative(&board));
        }
        let file = project
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let profile = self.shown_profile_name(job);
        Some(command_line(
            &file,
            &opens,
            &relative(job),
            profile.as_deref(),
        ))
    }

    pub(super) fn copy_job_command(&mut self, job: &Path) -> Task<Message> {
        match self.job_command_line(job) {
            Some(line) => iced::clipboard::write(line).discard(),
            None => Task::none(),
        }
    }

    /// View in Gerber Viewer (mockup row 193b): the files of the run that
    /// the viewer reads open in its tab, which names the run. A stopped run
    /// wrote nothing, so it has none.
    pub(crate) fn view_job_run_in_gerber(
        &mut self,
        job: &Path,
        row: Option<usize>,
    ) -> Task<Message> {
        let row = row.or_else(|| {
            self.output_jobs()
                .tabs
                .get(job)
                .and_then(|ui| ui.history_pick)
        });
        let Some(place) = self.job_run_place(job, row).filter(|place| place.complete) else {
            crate::diagnostics::log_info("Export Job: this run wrote no files to view");
            return Task::none();
        };
        if place.viewable.is_empty() {
            crate::diagnostics::log_info(
                "Export Job: this run wrote no Gerber or drill file to view",
            );
            return Task::none();
        }
        let paths: Vec<PathBuf> = place
            .viewable
            .iter()
            .map(|file| place.folder.join(file))
            .collect();
        self.open_gerber_paths_from_run(paths, place.origin)
    }
}
