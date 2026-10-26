//! Rename Project (the project-tree dialog): rename the `.snxprj`, and the
//! project folder with it when the folder is named after the project
//! (#1221), then record the move in the project's version control (#1219).
//!
//! A rename that keeps the folder moves one small file and stays inside the
//! `update`, like a sheet rename. A folder move runs on the blocking pool and
//! lands in [`Signex::handle_project_folder_renamed`].

use std::path::{Path, PathBuf};

use super::project_rename_fs::{
    folder_collides, folder_target, rename_project_on_disk, same_path_on_disk,
};
use super::*;

impl Signex {
    /// Project-root rename — `state.target_path` is the project's
    /// `.snxprj`; the buffer is the new *stem*. Companion schematic / pcb
    /// files keep their filenames (they may be shared or referenced by
    /// version control under those names); the `.snxprj`'s `schematic_root` /
    /// `pcb_file` strings keep pointing at them. When the folder holding the
    /// project is named after it, the folder is renamed too.
    pub(super) fn handle_project_rename_submit(
        &mut self,
        state: &crate::app::RenameDialogState,
    ) -> Task<Message> {
        let new_stem = state.buffer.trim();
        if new_stem.is_empty() {
            self.set_rename_error("Name cannot be empty.");
            return Task::none();
        }
        if new_stem.contains(['/', '\\', '.']) {
            self.set_rename_error("Name cannot contain '/', '\\', or '.'.");
            return Task::none();
        }
        let dir = match state.target_path.parent() {
            Some(d) => d.to_path_buf(),
            None => {
                self.set_rename_error("Project file has no parent directory.");
                return Task::none();
            }
        };
        let old_stem = match state.target_path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => {
                self.set_rename_error("Project file has no stem.");
                return Task::none();
            }
        };
        if new_stem == old_stem {
            self.ui_state.rename_dialog = None;
            return Task::none();
        }
        let new_prj = dir.join(format!("{new_stem}.snxprj"));
        // A case-only rename on a case-insensitive volume finds the project
        // itself at the new name; that is not a collision.
        if new_prj.exists() && !same_path_on_disk(&state.target_path, &new_prj) {
            self.set_rename_error("A project with that name already exists.");
            return Task::none();
        }
        let Some(idx) = state.tree_path.first().copied() else {
            return Task::none();
        };
        let Some(project) = self.document_state.projects.get(idx) else {
            return Task::none();
        };
        let project_id = project.id;
        let recent_key = crate::recent_projects::RecentProjects::spelling_of(&state.target_path);

        if let Some(new_dir) = folder_target(&dir, &old_stem, new_stem)
            && self.project_folder_can_move(idx)
        {
            let job = ProjectRenameJob {
                project_id,
                old_dir: dir,
                new_dir,
                old_stem,
                new_stem: new_stem.to_string(),
                recent_key,
            };
            return self.begin_project_folder_rename(job);
        }

        // Only the file moves. Nothing else is written; the project's
        // in-memory record follows.
        if let Err(e) = std::fs::rename(&state.target_path, &new_prj) {
            self.set_rename_error(&format!("Rename failed: {e}"));
            return Task::none();
        }
        self.rekey_renamed_project_file(idx, &state.target_path, &new_prj, new_stem);
        self.replace_recent_project(&recent_key, &new_prj);
        self.queue_project_rename_commit(&state.target_path, &new_prj, &old_stem, new_stem);
        self.ui_state.rename_dialog = None;
        self.refresh_panel_ctx();
        Task::none()
    }

    /// Whether the folder of the project at `idx` may follow a rename:
    /// nothing in it is in the middle of a write, and nothing else points
    /// into it that this rename does not re-key (another open project, a
    /// mounted project library). In the second case the folder keeps its name
    /// and only the file is renamed, as before.
    fn project_folder_can_move(&self, idx: usize) -> bool {
        let Some(project) = self.document_state.projects.get(idx) else {
            return false;
        };
        let dir = project.dir();
        if !project.data.libraries.is_empty() || !project.pending_libraries.is_empty() {
            return false;
        }
        !self
            .document_state
            .projects
            .iter()
            .enumerate()
            .any(|(other, p)| other != idx && p.path.starts_with(dir))
    }

    /// Whether `path` lies in a project whose folder is being moved right
    /// now. A write or open there would hit the old folder, which is about
    /// to vanish (or be recreated empty by the write).
    pub(crate) fn path_is_in_moving_project(&self, path: &Path) -> bool {
        let ds = &self.document_state;
        !ds.project_folder_renames.is_empty()
            && ds.projects.iter().any(|p| {
                ds.project_folder_renames.contains(&p.id)
                    && crate::app::state::scope::path_is_under(path, p.dir(), p.case_fold)
            })
    }

    /// `Err` with a user-facing reason while `path` is in a moving project.
    pub(crate) fn refuse_while_project_moves(&self, path: &Path, what: &str) -> anyhow::Result<()> {
        if self.path_is_in_moving_project(path) {
            anyhow::bail!(
                "Cannot {what} {} while its project folder is being renamed. Try again once the rename has finished.",
                path.display()
            );
        }
        Ok(())
    }

    /// The reason a folder rename must not start now: a write into the
    /// project that is running or queued would land on a path that is about
    /// to vanish.
    fn project_folder_busy_reason(&self, job: &ProjectRenameJob) -> Option<&'static str> {
        let ds = &self.document_state;
        if ds.project_folder_renames.contains(&job.project_id) {
            return Some("This project is already being renamed.");
        }
        if ds.document_saves.is_any_saving() {
            return Some("A save is still running. Rename the project once it has finished.");
        }
        let under = |p: &PathBuf| p.starts_with(&job.old_dir);
        if ds.pending_opens.iter().any(under) {
            return Some("A file is still opening. Rename the project once it has opened.");
        }
        let git_busy = ds
            .pending_git_commits
            .iter()
            .any(|c| under(&c.project_root))
            || ds.inflight_git_commits.iter().any(|(root, _)| under(root))
            || ds.active_git_repos.iter().any(under);
        git_busy.then_some(
            "A version-control commit is still running. Rename the project once it has finished.",
        )
    }

    fn begin_project_folder_rename(&mut self, job: ProjectRenameJob) -> Task<Message> {
        if let Some(reason) = self.project_folder_busy_reason(&job) {
            self.set_rename_error(reason);
            return Task::none();
        }
        if folder_collides(&job.old_dir, &job.new_dir) {
            self.set_rename_error("A folder with that name already exists.");
            return Task::none();
        }
        self.document_state
            .project_folder_renames
            .insert(job.project_id);
        let run = job.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || rename_project_on_disk(&run))
                    .await
                    .unwrap_or_else(|e| Err(format!("spawn_blocking: {e}")))
            },
            move |result| {
                Message::Project(ProjectMsg::ProjectFolderRenamed {
                    job: job.clone(),
                    result,
                })
            },
        )
    }

    /// The folder move finished. On success every record keyed by a path
    /// under the old folder moves to the new one; on failure the disk is as
    /// it was and the dialog says why.
    pub(crate) fn handle_project_folder_renamed(
        &mut self,
        job: &ProjectRenameJob,
        result: Result<(), String>,
    ) -> Task<Message> {
        self.document_state
            .project_folder_renames
            .remove(&job.project_id);
        if let Err(reason) = result {
            crate::diagnostics::log_warning(format!(
                "Rename of project {} cancelled: {reason}",
                job.old_stem
            ));
            self.set_rename_error(&reason);
            return Task::none();
        }
        let old_file = job.old_project_file();
        let new_file = job.new_project_file();
        // In the old folder the file already has its new name.
        let renamed_in_old_dir = job.old_dir.join(format!("{}.snxprj", job.new_stem));
        if let Some(idx) = self
            .document_state
            .projects
            .iter()
            .position(|p| p.id == job.project_id)
        {
            self.rekey_renamed_project_file(idx, &old_file, &renamed_in_old_dir, &job.new_stem);
            self.rekey_project_folder(idx, job);
            self.queue_project_rename_commit(
                &job.new_dir.join(format!("{}.snxprj", job.old_stem)),
                &new_file,
                &job.old_stem,
                &job.new_stem,
            );
        }
        self.replace_recent_project(&job.recent_key, &new_file);
        self.ui_state.rename_dialog = None;
        self.refresh_panel_ctx();
        Task::none()
    }

    /// Update the in-memory project and any tab / engine that referenced
    /// the `.snxprj` path itself after the file moved from `from` to `to`.
    fn rekey_renamed_project_file(&mut self, idx: usize, from: &Path, to: &Path, new_stem: &str) {
        if let Some(loaded) = self.document_state.projects.get_mut(idx) {
            loaded.path = to.to_path_buf();
            loaded.data.name = new_stem.to_string();
        }
        for tab in self.document_state.tabs.iter_mut() {
            if tab.path == from {
                tab.path = to.to_path_buf();
                tab.title = new_stem.to_string();
            }
        }
        if let Some(engine) = self.document_state.engines.remove(from) {
            self.document_state.engines.insert(to.to_path_buf(), engine);
        }
        if self.document_state.active_path.as_deref() == Some(from) {
            self.document_state.active_path = Some(to.to_path_buf());
        }
        if self.document_state.dirty_paths.remove(from) {
            self.document_state.dirty_paths.insert(to.to_path_buf());
        }
        self.rekey_page_setup(from, to);
    }

    /// Move every path-keyed record under the old project folder to the new
    /// one: open documents (tabs, engines, editors, dirty markers, page
    /// setup, undocked windows) and the project's own directory, which is
    /// the git root.
    fn rekey_project_folder(&mut self, idx: usize, job: &ProjectRenameJob) {
        let ds = &self.document_state;
        let mut under: Vec<PathBuf> = ds
            .tabs
            .iter()
            .map(|t| t.path.clone())
            .chain(ds.engines.keys().cloned())
            .chain(ds.symbol_editors.keys().cloned())
            .chain(ds.footprint_editors.keys().cloned())
            .chain(ds.dirty_paths.iter().cloned())
            .chain(ds.pcb_revisions.keys().cloned())
            .chain(ds.active_path.iter().cloned())
            .filter(|p| p.starts_with(&job.old_dir))
            .collect();
        under.sort();
        under.dedup();
        for old in under {
            let Ok(rel) = old.strip_prefix(&job.old_dir) else {
                continue;
            };
            let new = job.new_dir.join(rel);
            self.rekey_open_document(&old, &new);
        }
        if let Some(loaded) = self.document_state.projects.get_mut(idx) {
            loaded.path = job.new_project_file();
            loaded.data.dir = job.new_dir.to_string_lossy().into_owned();
        }
    }

    /// Queue the commit that records the rename: the old `.snxprj` removed,
    /// the new one added, in the project's repository (#1219). `from` and
    /// `to` lie under the project's directory as it is named now.
    fn queue_project_rename_commit(&mut self, from: &Path, to: &Path, old: &str, new: &str) {
        self.commit_project_rename_to_git(
            from,
            to,
            &format!("Rename project {old} \u{2192} {new}"),
        );
    }
}
