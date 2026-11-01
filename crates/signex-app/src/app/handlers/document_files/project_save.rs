//! Writing a project's `.snxprj` (#1202, #859).
//!
//! Every save is planned in `update`, written by one worker job and applied
//! by [`Signex::handle_project_file_saved`]; no filesystem or git work runs
//! in `update`. The job creates the pending libraries first and then writes
//! the project listing them (`persist::project_save`). Every write goes
//! through the document-save ledger, so a later write always wins over an
//! earlier one, and a save asked for while one runs queues one follow-up
//! instead of starting a second writer. No save path renames the file: the
//! name is the file stem, changed only by the Rename flow. The
//! concurrent-writer rule is in
//! `docs/superpowers/specs/2026-10-26-project-tab-shell-design.md` §3.

use std::path::Path;

use anyhow::{Context, Result};
use iced::Task;

use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, Signex};
use crate::library::mount::{MountIntent, MountRequest, spawn_pending_mounts};
use crate::persist::{
    LibraryCreated, ProjectSaveDone, ProjectWritePlan, SaveOutcome, run_project_write,
};

/// The follow-up ledger's per-editor stamp; a project has one writer.
const PROJECT_FOLLOW_UP: u64 = 0;

impl Signex {
    /// Save the project's `.snxprj` off the UI thread. A clean project is
    /// not rewritten. While a write of the file runs, one follow-up save is
    /// queued for when it completes and no task is returned.
    pub(crate) fn begin_project_save(&mut self, project_id: ProjectId) -> Result<Task<Message>> {
        let project = self
            .document_state
            .project_by_id(project_id)
            .with_context(|| format!("project {project_id:?} is no longer loaded"))?;
        let path = project.path.clone();
        if !self.document_state.is_document_dirty(&path) {
            return Ok(Task::none());
        }
        if self.document_state.document_saves.is_saving(&path) {
            self.document_state
                .document_saves
                .queue_follow_up(path.clone(), PROJECT_FOLLOW_UP);
            crate::diagnostics::log_info(format!(
                "[save] {} is still being written; it saves again when that write finishes",
                path.display()
            ));
            return Ok(Task::none());
        }
        let plan = self.plan_project_write(project_id)?;
        let job = self
            .document_state
            .document_saves
            .begin(path.clone(), Vec::new());
        let sequence = job.sequence();
        Ok(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || run_project_write(plan, job))
                    .await
                    .unwrap_or_else(|join_error| ProjectSaveDone {
                        project_id,
                        path,
                        sequence,
                        bytes: Vec::new(),
                        libraries: Vec::new(),
                        result: Err(format!("the save task failed: {join_error}")),
                    })
            },
            |done| Message::Project(ProjectMsg::FileSaved(Box::new(done))),
        ))
    }

    /// Everything a project write needs from the model, taken before the
    /// worker starts. Also ends the Project tab's typing run, so what is
    /// typed after a save is its own undo step.
    fn plan_project_write(&mut self, project_id: ProjectId) -> Result<ProjectWritePlan> {
        // The write would race a project folder move: it can land in the old
        // folder before the re-key and never reach the new one.
        let project_path = self
            .document_state
            .project_by_id(project_id)
            .map(|project| project.path.clone())
            .with_context(|| format!("project {project_id:?} is no longer loaded"))?;
        self.refuse_while_project_moves(&project_path, "save")?;
        self.refuse_while_path_op(&project_path, "saved")?;
        self.refuse_project_while_delete_pending(project_id, "saved")?;
        let project = self
            .document_state
            .project_by_id(project_id)
            .with_context(|| format!("project {project_id:?} vanished before write"))?;
        for spec in project.pending_libraries.values() {
            self.refuse_while_path_op(&spec.lib_path, "created")?;
        }
        if let Some(tab) = self.document_state.project_tabs.get_mut(&project_id) {
            tab.general_history.close_run();
        }
        let project = self
            .document_state
            .project_by_id(project_id)
            .with_context(|| format!("project {project_id:?} vanished before write"))?;
        let mut pending: Vec<_> = project
            .pending_libraries
            .iter()
            .map(|(id, spec)| (*id, spec.clone()))
            .collect();
        // A stable order, so the entries land in the file the same way on
        // every save.
        pending.sort_by(|a, b| a.1.lib_path.cmp(&b.1.lib_path));
        Ok(ProjectWritePlan {
            project_id,
            path: project.path.clone(),
            snapshot: project.data.clone(),
            fold: project.case_fold,
            pending,
            defaults: self.library.new_library_defaults.clone(),
        })
    }

    /// An off-thread `.snxprj` write finished: apply what the worker did,
    /// then start the follow-up save that was asked for meanwhile.
    pub(crate) fn handle_project_file_saved(&mut self, done: ProjectSaveDone) -> Task<Message> {
        let (project_id, path) = (done.project_id, done.path.clone());
        let mounts = self.apply_project_save_done(done);
        let follow_up = self.start_project_follow_up(project_id, &path);
        Task::batch([mounts, follow_up])
    }

    fn apply_project_save_done(&mut self, done: ProjectSaveDone) -> Task<Message> {
        let ProjectSaveDone {
            project_id,
            path,
            sequence,
            bytes,
            libraries,
            result,
        } = done;
        self.document_state.document_saves.finish(&path);
        // First, whatever the write's own result: a library the worker
        // created is on disk, so the project must list it and drop it from
        // the pending set, or the next save would fail on "already exists".
        let (mounts, failed) = self.apply_created_libraries(project_id, &path, libraries);
        self.settle_project_write(project_id, &path, sequence, bytes, result);
        if !failed.is_empty() {
            let error = unmaterialised_error(&path, &failed);
            // Save All names this reason for the file it kept unsaved.
            self.document_state
                .document_saves
                .record_failure(path, format!("{error:#}"));
            self.report_save_failure("Failed to save project file", &error);
        }
        self.refresh_panel_ctx();
        mounts
    }

    /// Register and mount each library the worker created, if the project is
    /// still the one that asked for it. Returns the mount task and the
    /// libraries that could not be created (they stay pending).
    fn apply_created_libraries(
        &mut self,
        project_id: ProjectId,
        path: &Path,
        libraries: Vec<LibraryCreated>,
    ) -> (Task<Message>, Vec<String>) {
        let mut failed = Vec::new();
        let mut to_mount = Vec::new();
        for created in libraries {
            let LibraryCreated {
                library_id,
                spec,
                result,
            } = created;
            if let Err(error) = result {
                tracing::warn!(
                    target: "signex::library",
                    project = %path.display(),
                    library = %spec.lib_path.display(),
                    library_id = %library_id,
                    error = %error,
                    "pending library not created; keeping entry pending for retry"
                );
                failed.push(format!("{} ({error})", spec.display_name));
                continue;
            }
            if !self.register_created_library(project_id, path, library_id, &spec) {
                continue;
            }
            let request = self
                .library
                .request_mount(&spec.lib_path, MountIntent::Silent);
            if matches!(request, MountRequest::Spawn) {
                to_mount.push(spec.lib_path);
            }
        }
        (spawn_pending_mounts(to_mount), failed)
    }

    /// List a created library on the project that still has it pending and
    /// drop the pending entry. False, with a Messages warning, when that
    /// project is gone or no longer wants it.
    fn register_created_library(
        &mut self,
        project_id: ProjectId,
        path: &Path,
        library_id: uuid::Uuid,
        spec: &crate::library::commands::PendingLibrarySpec,
    ) -> bool {
        let registered = self
            .document_state
            .project_by_id_mut(project_id)
            .filter(|loaded| loaded.path == path)
            .and_then(|loaded| {
                loaded.pending_libraries.remove(&library_id)?;
                crate::library::commands::register_library_entry(
                    &mut loaded.data,
                    &spec.lib_path,
                    library_id,
                    loaded.case_fold,
                );
                Some(())
            });
        if registered.is_none() {
            crate::diagnostics::log_warning(format!(
                "[save] The library {} was created, but project {} no longer asks for it; \
                 it is on disk and not in the open project",
                spec.lib_path.display(),
                path.display()
            ));
            return false;
        }
        tracing::info!(
            target: "signex::library",
            project = %path.display(),
            library = %spec.lib_path.display(),
            library_id = %library_id,
            "materialised pending library"
        );
        true
    }

    /// Decide what the write's result means for the project: failures reach
    /// the Messages panel, and the dirty marker clears only for the newest
    /// write begun, of the same loaded project, whose bytes are exactly what
    /// the project encodes to now (design doc §3).
    fn settle_project_write(
        &mut self,
        project_id: ProjectId,
        path: &Path,
        sequence: u64,
        bytes: Vec<u8>,
        result: Result<SaveOutcome, String>,
    ) {
        if !self.note_project_write_result(path, sequence, result) {
            return;
        }
        let newest = self
            .document_state
            .document_saves
            .is_newest_begun(path, sequence);
        let unchanged = newest
            && self
                .document_state
                .project_by_id(project_id)
                .is_some_and(|project| {
                    project.path == path
                        && project.pending_libraries.is_empty()
                        && signex_types::project::encode_project(&project.data)
                            .is_ok_and(|now| now == bytes)
                });
        if unchanged {
            let saved = crate::app::state::CommitBytes {
                bytes: std::sync::Arc::new(bytes),
                sequence,
            };
            self.finish_clean_project_save(path, saved);
        } else if self.document_state.is_document_dirty(path) {
            crate::diagnostics::log_info(format!(
                "[save] {} changed while it was being written; it stays modified",
                path.display()
            ));
        }
    }

    /// Record the write's outcome. True when the file was written, so the
    /// caller may go on to decide whether the project is clean.
    fn note_project_write_result(
        &mut self,
        path: &Path,
        sequence: u64,
        result: Result<SaveOutcome, String>,
    ) -> bool {
        let newest = self
            .document_state
            .document_saves
            .is_newest_begun(path, sequence);
        match result {
            Ok(SaveOutcome::Written) => {}
            // Defensive: one writer per file, so no newer write can have
            // landed first. If one ever did, its completion decides.
            Ok(SaveOutcome::Superseded) => return false,
            // Defensive: a project write carries no disk expectation.
            Ok(SaveOutcome::DiskChanged(_)) => return false,
            // Defensive, same reason: only the newest write's failure counts.
            Err(reason) if !newest => {
                crate::diagnostics::log_warning(format!(
                    "[save] An earlier write of project {} failed after a newer one began: {reason}",
                    path.display()
                ));
                return false;
            }
            Err(reason) => {
                self.document_state
                    .document_saves
                    .record_failure(path.to_path_buf(), reason.clone());
                self.report_save_failure("Failed to save project file", &anyhow::anyhow!(reason));
                return false;
            }
        }
        // The file on disk changed behind every engine (#1401).
        self.document_state.note_documents_changed();
        self.document_state.document_saves.clear_failure(path);
        crate::diagnostics::log_info(format!("[save] Wrote project {}", path.display()));
        true
    }

    /// Run the save asked for while the write of `path` was running, if the
    /// project is still loaded there and still unsaved.
    fn start_project_follow_up(&mut self, project_id: ProjectId, path: &Path) -> Task<Message> {
        if self.document_state.document_saves.is_saving(path) {
            return Task::none();
        }
        let asked = self.document_state.document_saves.take_follow_ups(path);
        let still_loaded = self
            .document_state
            .project_by_id(project_id)
            .is_some_and(|project| project.path == path);
        if asked.is_empty() || !still_loaded {
            return Task::none();
        }
        match self.begin_project_save(project_id) {
            Ok(task) => task,
            Err(error) => {
                self.report_save_failure("Failed to save project file", &error);
                Task::none()
            }
        }
    }

    /// The project's bytes on disk match it: clear the marker and queue the
    /// project git commit of exactly `saved`. The caller refreshes the panels.
    fn finish_clean_project_save(&mut self, path: &Path, saved: crate::app::state::CommitBytes) {
        self.document_state.mark_document_clean(path);
        let label = path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        self.commit_bytes_to_project_git(path, &format!("Save {label}"), saved);
    }
}

/// The error for a project written without some of its new libraries.
fn unmaterialised_error(path: &Path, missing: &[String]) -> anyhow::Error {
    anyhow::anyhow!(
        "{} was written without {} new librar{} that could not be created: {}",
        path.display(),
        missing.len(),
        if missing.len() == 1 { "y" } else { "ies" },
        missing.join("; ")
    )
}
