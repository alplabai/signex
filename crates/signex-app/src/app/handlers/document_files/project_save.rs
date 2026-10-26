//! Writing a project's `.snxprj` (#1202).
//!
//! Every write goes through the document-save ledger, so a later write
//! always wins over an earlier one whatever order they finish in. Ctrl+S in
//! a Project tab writes off the UI thread ([`Signex::begin_project_save`]);
//! the other project saves (Ctrl+S in a document tab, Save All on close and
//! quit) still write inline ([`Signex::persist_project_by_id`]).
//!
//! Both first apply a pending rename (an edited project name is the file
//! stem) and materialise pending libraries, then encode the project as it is
//! now. The concurrent-writer rule this relies on is in
//! `docs/superpowers/specs/2026-10-26-project-tab-shell-design.md` §3.

use std::path::PathBuf;

use anyhow::{Context, Result};
use iced::Task;

use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, Signex};
use crate::persist::{ProjectSaveDone, SaveOutcome};

/// A project encoded for one write.
struct ProjectWrite {
    path: PathBuf,
    bytes: Vec<u8>,
    /// Pending libraries that could not be created: the file is written
    /// without them and the project stays dirty.
    unmaterialised: Vec<String>,
}

impl Signex {
    /// Ctrl+S in a Project tab: write the project's `.snxprj` off the UI
    /// thread. A clean project is not rewritten.
    pub(crate) fn begin_project_save(&mut self, project_id: ProjectId) -> Result<Task<Message>> {
        let project = self
            .document_state
            .project_by_id(project_id)
            .with_context(|| format!("project {project_id:?} is no longer loaded"))?;
        if !self.document_state.dirty_paths.contains(&project.path) {
            return Ok(Task::none());
        }
        let write = self.prepare_project_write(project_id)?;
        if let Some(error) = unmaterialised_error(&write) {
            // Reported now; the write still goes ahead, and the completion
            // leaves the project dirty because the library is still pending.
            self.report_save_failure("Failed to save project file", &error);
        }
        let job = self
            .document_state
            .document_saves
            .begin(write.path.clone(), write.bytes.clone());
        let (path, bytes) = (write.path, write.bytes);
        Ok(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || job.run())
                    .await
                    .map_err(|join_error| format!("the save task failed: {join_error}"))
                    .and_then(|result| result)
            },
            move |result| {
                Message::Project(ProjectMsg::FileSaved(Box::new(ProjectSaveDone {
                    project_id,
                    path: path.clone(),
                    bytes: bytes.clone(),
                    result,
                })))
            },
        ))
    }

    /// An off-thread `.snxprj` write finished. The project is marked clean
    /// only if it is still the same loaded project at the same path and
    /// still encodes to exactly the bytes written: another writer that
    /// changed it meanwhile keeps it dirty (design doc §3).
    pub(crate) fn handle_project_file_saved(&mut self, done: ProjectSaveDone) -> Task<Message> {
        let ProjectSaveDone {
            project_id,
            path,
            bytes,
            result,
        } = done;
        self.document_state.document_saves.finish(&path);
        match result {
            Ok(SaveOutcome::Written) => {}
            // A newer write of this file landed first; its own completion
            // (or the inline save that made it) decides.
            Ok(SaveOutcome::Superseded) => return Task::none(),
            Err(reason) => {
                self.document_state
                    .document_saves
                    .record_failure(path, reason.clone());
                self.report_save_failure("Failed to save project file", &anyhow::anyhow!(reason));
                return Task::none();
            }
        }
        self.document_state.document_saves.clear_failure(&path);
        crate::diagnostics::log_info(format!("[save] Wrote project {}", path.display()));
        let unchanged = self
            .document_state
            .project_by_id(project_id)
            .is_some_and(|project| {
                project.path == path
                    && project.pending_libraries.is_empty()
                    && signex_types::project::encode_project(&project.data)
                        .is_ok_and(|now| now == bytes)
            });
        if unchanged {
            self.finish_clean_project_save(&path);
        } else if self.document_state.dirty_paths.contains(&path) {
            crate::diagnostics::log_info(format!(
                "[save] {} changed while it was being written; it stays modified",
                path.display()
            ));
        }
        self.refresh_panel_ctx();
        Task::none()
    }

    /// Write the identified project's `.snxprj` inline, through the save
    /// ledger. On success clears its dirty marker and queues the project
    /// git commit. Errs for a refused rename, a failed write, and when a
    /// pending library could not be created (the file is written without
    /// it, the entry stays pending, and the project stays dirty, so Save All
    /// callers never treat it as saved).
    pub(crate) fn persist_project_by_id(&mut self, project_id: ProjectId) -> Result<()> {
        let write = self.prepare_project_write(project_id)?;
        let job = self
            .document_state
            .document_saves
            .begin(write.path.clone(), write.bytes.clone());
        let outcome = job.run();
        self.document_state.document_saves.finish(&write.path);
        if let Err(reason) = outcome {
            self.document_state
                .document_saves
                .record_failure(write.path.clone(), reason.clone());
            anyhow::bail!("write project {}: {reason}", write.path.display());
        }
        self.document_state
            .document_saves
            .clear_failure(&write.path);
        if let Some(error) = unmaterialised_error(&write) {
            return Err(error);
        }
        crate::diagnostics::log_info(format!("[save] Wrote project {}", write.path.display()));
        self.finish_clean_project_save(&write.path);
        Ok(())
    }

    /// Everything a project write does before the bytes: apply a pending
    /// rename, materialise pending libraries, encode.
    fn prepare_project_write(&mut self, project_id: ProjectId) -> Result<ProjectWrite> {
        self.apply_pending_project_rename(project_id)?;
        let unmaterialised = self.materialise_pending_libraries(project_id);
        let project = self
            .document_state
            .project_by_id(project_id)
            .with_context(|| format!("project {project_id:?} vanished before write"))?;
        let bytes = signex_types::project::encode_project(&project.data)
            .with_context(|| format!("encode project {}", project.path.display()))?;
        Ok(ProjectWrite {
            path: project.path.clone(),
            bytes,
            unmaterialised,
        })
    }

    /// The project name is the `.snxprj` file stem (the loader reads it from
    /// the file name), so a name edited in the Project tab is a rename,
    /// applied here before the write. Refused names fail the save.
    fn apply_pending_project_rename(&mut self, project_id: ProjectId) -> Result<()> {
        let Some(idx) = self
            .document_state
            .projects
            .iter()
            .position(|p| p.id == project_id)
        else {
            anyhow::bail!("project {project_id:?} is no longer loaded");
        };
        let project = &self.document_state.projects[idx];
        let wanted = project.data.name.trim().to_string();
        let current = project
            .path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        if wanted == current {
            // Typed surrounding spaces are not part of the name.
            self.document_state.projects[idx].data.name = wanted;
            return Ok(());
        }
        if let Some(problem) = crate::app::project_tab::project_stem_problem(&wanted) {
            anyhow::bail!("project name \"{wanted}\" refused: {problem}");
        }
        self.move_project_file(idx, &wanted)
            .map(|_| ())
            .map_err(|reason| anyhow::anyhow!("rename the project to \"{wanted}\": {reason}"))
    }

    /// Create the libraries registered through the New Library flow (they
    /// exist only in memory until the user saves) and add their entries to
    /// the project. Returns the ones that could not be created; they stay
    /// pending for the next save.
    fn materialise_pending_libraries(&mut self, project_id: ProjectId) -> Vec<String> {
        let pending: Vec<(uuid::Uuid, crate::library::commands::PendingLibrarySpec)> = self
            .document_state
            .projects
            .iter_mut()
            .find(|p| p.id == project_id)
            .map(|p| p.pending_libraries.drain().collect())
            .unwrap_or_default();
        let mut unmaterialised = Vec::new();
        for (library_id, spec) in pending {
            let Some(loaded) = self
                .document_state
                .projects
                .iter_mut()
                .find(|p| p.id == project_id)
            else {
                continue;
            };
            let project_path = loaded.path.clone();
            let result = crate::library::commands::materialize_pending_library(
                &mut self.library,
                &mut loaded.data,
                library_id,
                &spec,
            );
            match result {
                Ok(()) => tracing::info!(
                    target: "signex::library",
                    project = %project_path.display(),
                    library = %spec.lib_path.display(),
                    library_id = %library_id,
                    "materialised pending library"
                ),
                Err(error) => {
                    tracing::warn!(
                        target: "signex::library",
                        project = %project_path.display(),
                        library = %spec.lib_path.display(),
                        library_id = %library_id,
                        error = %error,
                        "materialise_pending_library failed; keeping entry pending for retry"
                    );
                    unmaterialised.push(format!("{} ({error})", spec.display_name));
                    loaded.pending_libraries.insert(library_id, spec);
                }
            }
        }
        unmaterialised
    }

    /// The project's bytes on disk match it: clear the marker, queue the
    /// project git commit, refresh the tree's dirty dot.
    fn finish_clean_project_save(&mut self, path: &std::path::Path) {
        self.document_state.dirty_paths.remove(path);
        let label = path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        self.commit_save_to_project_git(path, &format!("Save {label}"));
        self.refresh_panel_ctx();
    }
}

/// The error for a project written without some of its new libraries.
fn unmaterialised_error(write: &ProjectWrite) -> Option<anyhow::Error> {
    let missing = &write.unmaterialised;
    (!missing.is_empty()).then(|| {
        anyhow::anyhow!(
            "{} was written without {} new librar{} that could not be created: {}",
            write.path.display(),
            missing.len(),
            if missing.len() == 1 { "y" } else { "ies" },
            missing.join("; ")
        )
    })
}
