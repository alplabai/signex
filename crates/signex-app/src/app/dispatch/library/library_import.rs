//! Library import (#3113, #3114): start a converter run on a worker, then
//! mount what it wrote. The Import Library dialog calls
//! [`Signex::start_library_import`]; the process, git and the global list
//! write all happen in `crate::library_import`, off the update thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use iced::futures::SinkExt as _;
use signex_i18n::{Translatable, ids};

use super::*;
use crate::app::state::ErrorNotice;
use crate::ignore::IgnoreResult as _;
use crate::library::import_dialog::result::{Mounted, RequiredLibrary};
use crate::library::messages::LibraryImportMsg;
use crate::library::mount::{MountIntent, MountRequest, spawn_pending_mounts};
use crate::library::refresh::{RefreshFollowUp, refresh_task};
use crate::library_import::run_state::{ImportFinished, ImportRun, MountTarget};
use crate::library_import::{
    ExitKind, ImportError, ImportJob, ImportJobOutcome, MountScope, RunFailure, RunMode,
};

/// Progress messages queued before the oldest is dropped; progress is
/// display only, the result is never dropped.
const PROGRESS_QUEUE: usize = 64;

fn import_message(msg: LibraryImportMsg) -> Message {
    Message::Library(LibraryMessage::Import(msg))
}

/// Record `file` in `global_libraries.toml` on a worker.
fn record_global_task(file: std::path::PathBuf) -> Task<Message> {
    Task::perform(
        async move {
            let path = file.clone();
            let result = tokio::task::spawn_blocking(move || {
                crate::panels::components_panel::global_prefs::add_path(path).map(|_| ())
            })
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
            (file, result)
        },
        |(file, result)| import_message(LibraryImportMsg::GlobalRecorded { file, result }),
    )
}

impl Signex {
    /// Start `job`, mounting the result for `scope`. Refused while another
    /// import runs. `ThisProject` with no project open mounts for all
    /// projects instead, and says so.
    pub(crate) fn start_library_import(
        &mut self,
        job: ImportJob,
        scope: MountScope,
    ) -> Task<Message> {
        if self.refuse_library_import(&job) {
            return Task::none();
        }
        let target = self.mount_target_for(scope);
        self.run_library_import(job, target)
    }

    /// Start `job`, mounting the result at `target` (a project the Import
    /// Library dialog was opened for). Refused as [`Self::start_library_import`].
    pub(crate) fn start_library_import_into(
        &mut self,
        job: ImportJob,
        target: MountTarget,
    ) -> Task<Message> {
        if self.refuse_library_import(&job) {
            return Task::none();
        }
        self.run_library_import(job, target)
    }

    /// Refuse `job` with a notice while another import runs, or for an
    /// update while the library has unsaved work in the app.
    fn refuse_library_import(&mut self, job: &ImportJob) -> bool {
        if let Some(running) = &self.library.import.running {
            self.document_view.error_notice = Some(ErrorNotice::new(
                ids::LIBRARY_IMPORT_FAILED_TITLE,
                Translatable::new(ids::LIBRARY_IMPORT_RUNNING)
                    .arg("path", running.output.display().to_string()),
            ));
            return true;
        }
        if job.request.mode == RunMode::Update
            && let Some(what) = self.unsaved_in_library(&job.request.output)
        {
            self.document_view.error_notice = Some(ErrorNotice::new(
                ids::LIBRARY_IMPORT_FAILED_TITLE,
                ImportError::UnsavedInApp { what }.to_translatable(),
            ));
            return true;
        }
        false
    }

    fn run_library_import(&mut self, job: ImportJob, target: MountTarget) -> Task<Message> {
        let cancel = Arc::new(AtomicBool::new(false));
        self.library.import.running = Some(ImportRun {
            output: job.request.output.clone(),
            mode: job.request.mode,
            target: target.clone(),
            cancel: cancel.clone(),
            group: job.group.clone(),
            progress: None,
        });
        log::info!(
            "Library import: {} into {}",
            job.converter,
            job.request.output.display()
        );
        let stream = iced::stream::channel(PROGRESS_QUEUE, async move |mut out| {
            let mut progress = out.clone();
            let failed_target = target.clone();
            let worker = tokio::task::spawn_blocking(move || {
                let result = job.run(&cancel, &mut |event| {
                    progress
                        .try_send(import_message(LibraryImportMsg::ProgressReported(
                            event.clone(),
                        )))
                        .ignore("progress is display only; a full queue skips an update");
                });
                ImportFinished { result, target }
            });
            let finished = worker.await.unwrap_or_else(|e| ImportFinished {
                result: Err(RunFailure::new(
                    crate::library_import::ImportError::WaitFailed {
                        detail: e.to_string(),
                    },
                )),
                target: failed_target,
            });
            out.send(import_message(LibraryImportMsg::Finished(Box::new(
                finished,
            ))))
            .await
            .ignore("the runtime stopped listening; nothing is left to tell");
        });
        Task::run(stream, |message| message)
    }

    /// The first unsaved editor or document inside the library folder
    /// `dir`, named for the refusal; an update would overwrite it.
    pub(super) fn unsaved_in_library(&self, dir: &std::path::Path) -> Option<String> {
        use crate::app::state::scope::{CaseFold, path_is_under};
        let fold = CaseFold::host_default();
        if let Some(editor) = self
            .library
            .dirty_editors_matching(|root| path_is_under(root, dir, fold))
            .first()
        {
            return Some(self.library.editor_label(editor));
        }
        self.unsaved_documents(dir, fold)
            .first()
            .map(|p| p.display().to_string())
    }

    /// Settle where an import is mounted while the active project is known.
    fn mount_target_for(&mut self, scope: MountScope) -> MountTarget {
        match scope {
            MountScope::AllProjects => MountTarget::Global,
            MountScope::ThisProject => {
                let project = self
                    .document_state
                    .active_project
                    .and_then(|id| self.document_state.project_by_id(id))
                    .map(|p| p.path.clone());
                project.map_or_else(
                    || {
                        crate::diagnostics::log_warning(
                            Translatable::new(ids::LIBRARY_IMPORT_NO_PROJECT).render_en_us(),
                        );
                        MountTarget::Global
                    },
                    MountTarget::Project,
                )
            }
        }
    }

    pub(super) fn handle_library_import(&mut self, msg: LibraryImportMsg) -> Task<Message> {
        match msg {
            LibraryImportMsg::ProgressReported(event) => {
                let previous = self
                    .library
                    .import
                    .running
                    .as_mut()
                    .map(|run| run.progress.replace(event.clone()));
                if let Some(previous) = previous {
                    self.import_dialog_progress(previous.as_ref(), &event);
                }
                Task::none()
            }
            LibraryImportMsg::Dialog(msg) => self.handle_import_dialog(msg),
            LibraryImportMsg::CancelRequested => {
                if let Some(run) = &self.library.import.running {
                    run.cancel.store(true, Ordering::Relaxed);
                    log::info!("Library import: cancelling {}", run.output.display());
                }
                Task::none()
            }
            LibraryImportMsg::Finished(finished) => self.finish_library_import(*finished),
            LibraryImportMsg::GlobalRecorded { file, result } => {
                match result {
                    Ok(()) => crate::panels::components_panel::global_prefs::merge_path(
                        &mut self.library.global_libraries,
                        file,
                    ),
                    Err(reason) => self.report_global_libraries_save_failure(
                        "record the imported library",
                        &reason,
                    ),
                }
                Task::none()
            }
        }
    }

    /// A run ended. The Import Library dialog shows the result of a run it
    /// waits for (a dry run, a failed import, a finished import with its
    /// report); no notice repeats it.
    fn finish_library_import(&mut self, finished: ImportFinished) -> Task<Message> {
        let run = self.library.import.running.take();
        let mode = run.as_ref().map(|run| run.mode);
        let shown = self.import_dialog_run_finished(&finished, mode);
        // A dry run the dialog stopped waiting for (Back, Close) leaves its
        // temp folder; a failed one was removed by the engine already.
        let leftover = match (&run, shown) {
            (Some(run), false) if run.mode == RunMode::Preview => {
                super::import_dialog::remove_preview_dir(run.output.clone())
            }
            _ => Task::none(),
        };
        let task = match &finished.result {
            Err(failure) if *failure.error == ImportError::Cancelled => {
                log::info!("Library import: cancelled");
                if !shown && mode != Some(RunMode::Preview) {
                    self.report_library_import_failure(failure);
                }
                Task::none()
            }
            Err(failure) if shown || mode == Some(RunMode::Preview) => {
                crate::diagnostics::log_error(
                    "Library import",
                    &anyhow::anyhow!("{}\n{}", failure.error, failure.stderr_tail.trim()),
                );
                Task::none()
            }
            Err(failure) => {
                self.report_library_import_failure(failure);
                Task::none()
            }
            Ok(outcome) => {
                let (task, mounted) = self.mount_imported_library(outcome, &finished, shown);
                if shown && let Some(mounted) = mounted {
                    let required = self.required_libraries(&outcome.requires);
                    self.import_dialog_mounted(mounted, required);
                }
                task
            }
        };
        self.library.import.last = Some(Box::new(finished));
        Task::batch([task, leftover])
    }

    /// Quitting: the worker and its converter would outlive the window, so
    /// the run is told to stop and the converter's process group is killed
    /// now. Synchronous: it only sends signals.
    pub(crate) fn stop_library_import_at_quit(&mut self) {
        if let Some(run) = &self.library.import.running {
            log::info!("Library import: stopped by quit ({})", run.output.display());
            run.cancel.store(true, Ordering::Relaxed);
            run.group.kill();
        }
    }

    fn report_library_import_failure(&mut self, failure: &RunFailure) {
        let error = failure.error.to_translatable();
        let detail = match &failure.restored {
            None => error,
            Some(Ok(())) => {
                Translatable::new(ids::LIBRARY_IMPORT_UPDATE_RESTORED).arg("error", error)
            }
            Some(Err(reason)) => Translatable::new(ids::LIBRARY_IMPORT_UPDATE_NOT_RESTORED)
                .arg("error", error)
                .arg("reason", reason.as_str()),
        };
        crate::diagnostics::log_error(
            "Library import",
            &anyhow::anyhow!("{}\n{}", failure.error, failure.stderr_tail.trim()),
        );
        self.document_view.error_notice =
            Some(ErrorNotice::new(ids::LIBRARY_IMPORT_FAILED_TITLE, detail));
    }

    /// Record and mount a written library; refresh it when it is already
    /// mounted (a re-import). A preview mounts nothing (`None`). A library
    /// whose id another open library has is neither recorded nor mounted;
    /// the notice saying so is left to the dialog when it shows the result
    /// (`shown`).
    fn mount_imported_library(
        &mut self,
        outcome: &ImportJobOutcome,
        finished: &ImportFinished,
        shown: bool,
    ) -> (Task<Message>, Option<Mounted>) {
        let Some(file) = outcome.library_file.clone() else {
            return (Task::none(), None);
        };
        if let Some(other) = self.library_id_clash(&file, outcome.library_id) {
            let detail = Translatable::new(ids::LIBRARY_IMPORT_ID_CLASH)
                .arg("path", file.display().to_string())
                .arg("other", other.display().to_string());
            crate::diagnostics::log_warning(detail.render_en_us());
            if !shown {
                self.document_view.error_notice =
                    Some(ErrorNotice::new(ids::LIBRARY_IMPORT_FAILED_TITLE, detail));
            }
            return (Task::none(), Some(Mounted::Clash(other)));
        }
        let mut tasks = vec![self.mount_required_libraries(&outcome.requires)];
        let mounted = match &finished.target {
            MountTarget::Global => {
                tasks.push(record_global_task(file.clone()));
                Mounted::AllProjects
            }
            MountTarget::Project(project) => self
                .add_imported_library_to_project(project, &file, outcome.library_id)
                .map_or(Mounted::Unrecorded, Mounted::Project),
        };
        self.log_import_result(outcome, &file, shown);
        tasks.push(
            match self.library.request_mount(&file, MountIntent::Silent) {
                MountRequest::Spawn => spawn_pending_mounts(vec![file]),
                MountRequest::AlreadyMounted => {
                    refresh_task(&mut self.library, &file, RefreshFollowUp::Nothing)
                }
                MountRequest::InFlight => Task::none(),
            },
        );
        self.refresh_panel_ctx();
        (Task::batch(tasks), Some(mounted))
    }

    /// Each of `library_ids` with the name of the open library that has
    /// it, if one does.
    fn required_libraries(&self, library_ids: &[uuid::Uuid]) -> Vec<RequiredLibrary> {
        library_ids
            .iter()
            .map(|&id| RequiredLibrary {
                id,
                open_as: self
                    .library
                    .open_libraries
                    .iter()
                    .find(|lib| lib.library_id == id)
                    .map(|lib| lib.display_name.clone()),
            })
            .collect()
    }

    /// Another open library with `library_id`; a re-import of the same
    /// file is no clash.
    fn library_id_clash(
        &self,
        file: &std::path::Path,
        library_id: Option<uuid::Uuid>,
    ) -> Option<std::path::PathBuf> {
        let id = library_id?;
        self.library
            .open_libraries
            .iter()
            .find(|lib| lib.library_id == id && lib.root != file)
            .map(|lib| lib.root.clone())
    }

    /// Put `file` on the project's library list (once), so the next save
    /// records it in the `.snxprj`. The project's name, or `None` when the
    /// library has no readable id or the project closed meanwhile.
    fn add_imported_library_to_project(
        &mut self,
        project: &std::path::Path,
        file: &std::path::Path,
        library_id: Option<uuid::Uuid>,
    ) -> Option<String> {
        let Some(library_id) = library_id else {
            log::warn!("Library import: {} has no readable id", file.display());
            return None;
        };
        let Some(loaded) = self
            .document_state
            .projects
            .iter_mut()
            .find(|p| p.path == project)
        else {
            log::warn!(
                "Library import: project {} closed before the import finished",
                project.display()
            );
            return None;
        };
        let name = loaded.data.name.clone();
        if loaded
            .data
            .libraries
            .iter()
            .any(|entry| entry.library_id == Some(library_id))
        {
            return Some(name);
        }
        let fold = loaded.case_fold;
        crate::library::commands::register_library_entry(&mut loaded.data, file, library_id, fold);
        let project_path = loaded.path.clone();
        self.document_state.mark_document_dirty(&project_path);
        Some(name)
    }

    /// Log a written library; a partial import raises a notice unless the
    /// dialog shows its result (`shown`).
    fn log_import_result(
        &mut self,
        outcome: &ImportJobOutcome,
        file: &std::path::Path,
        shown: bool,
    ) {
        let summary = &outcome.run.summary;
        let name = file
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let done = Translatable::new(ids::LIBRARY_IMPORT_DONE)
            .arg("name", name)
            .arg("path", file.display().to_string())
            .arg("parts", summary.counts.parts)
            .arg("skipped", summary.counts.skipped);
        crate::diagnostics::log_info(done.render_en_us());
        for (message, count) in &summary.warnings {
            crate::diagnostics::log_warning(format!("Library import: {message} (×{count})"));
        }
        if outcome.local_commits > 0 {
            crate::diagnostics::log_warning(
                Translatable::new(ids::LIBRARY_IMPORT_LOCAL_COMMITS)
                    .arg("count", outcome.local_commits)
                    .render_en_us(),
            );
        }
        if outcome.run.exit == ExitKind::Partial && !shown {
            self.document_view.error_notice =
                Some(ErrorNotice::new(ids::LIBRARY_IMPORT_DONE_TITLE, done));
        }
    }

    /// The libraries an imported library's manifest `requires` (contract
    /// §6, step 4), by library id. One already open needs nothing; one that
    /// is not is noted here, named in the dialog's result card, and listed
    /// in Messages once the mount settles (`library::requires`). Finding a
    /// library that is not open by its id is not done: nothing records the
    /// ids of libraries that are not mounted.
    pub(crate) fn mount_required_libraries(&mut self, library_ids: &[uuid::Uuid]) -> Task<Message> {
        for id in library_ids {
            if !self.library.open_libraries.iter().any(|lib| lib.library_id == *id) {
                log::info!("Library import: required library {id} is not open");
            }
        }
        Task::none()
    }
}
