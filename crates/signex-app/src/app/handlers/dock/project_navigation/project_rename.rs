//! Rename Project (the project-tree dialog): rename the `.snxprj`, and the
//! project folder with it when the folder is named after the project
//! (#1221), then record the move in the project's version control (#1219).
//!
//! A rename that keeps the folder moves one small file and stays inside the
//! `update`, like a sheet rename. A folder move runs on the blocking pool and
//! lands in [`Signex::handle_project_folder_renamed`].

use std::path::{Path, PathBuf};

use super::project_rename_fs::{
    carry_closed_copies, folder_collides, folder_target, journal_closed_copies,
    rename_project_on_disk, same_path_on_disk, unjournal_closed_copies,
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
            if let Some(library) = self.library_under_project_folder(idx) {
                self.set_rename_error(&format!(
                    "Cannot rename the project folder while the library {} inside it is in use \
                     (open, mounted, installed or built in). Close or remove it first, then \
                     rename the project.",
                    library.display()
                ));
                return Task::none();
            }
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

        // A write of the project file runs only after `update` returns, so
        // waiting for it here would hang the app, and letting it land after
        // the move would recreate the old file: refuse instead.
        if self
            .document_state
            .document_saves
            .is_saving(&state.target_path)
        {
            self.refuse_rename_while_project_saving();
            return Task::none();
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
            .any(|(other, p)| {
                other != idx
                    && crate::app::state::scope::path_is_under(&p.path, dir, project.case_fold)
            })
    }

    /// A library the app holds (mounted, Global, Installed, browser tab or
    /// mount in flight) that lies under the folder of the project at `idx`.
    /// The move does not re-key libraries, so one in there would be left
    /// pointing at a missing path.
    fn library_under_project_folder(&self, idx: usize) -> Option<PathBuf> {
        let project = self.document_state.projects.get(idx)?;
        let tabs = self
            .document_state
            .tabs
            .iter()
            .filter_map(|t| match &t.kind {
                crate::app::TabKind::LibraryBrowser(p) => Some(p.as_path()),
                _ => None,
            });
        self.library
            .held_library_paths()
            .chain(tabs)
            .find(|p| crate::app::state::scope::path_is_under(p, project.dir(), project.case_fold))
            .map(Path::to_path_buf)
    }

    /// Whether `path` lies in a project whose folder is being moved right
    /// now. A write or open there races the move: it can land in the old
    /// folder before the re-key, and the file then never reaches the new
    /// folder.
    pub(crate) fn path_is_in_moving_project(&self, path: &Path) -> bool {
        let ds = &self.document_state;
        !ds.project_folder_renames.is_empty()
            && ds.projects.iter().any(|p| {
                ds.project_folder_renames.contains(&p.id)
                    && crate::app::state::scope::path_is_under(path, p.dir(), p.case_fold)
            })
    }

    /// A background write to `target` starts; a rename of a folder holding
    /// it is refused until [`Self::end_project_write`].
    pub(crate) fn begin_project_write(&mut self, target: &Path) {
        self.document_state
            .project_writes
            .push(target.to_path_buf());
    }

    /// The background write to `target` finished, whatever its result.
    pub(crate) fn end_project_write(&mut self, target: &Path) {
        let writes = &mut self.document_state.project_writes;
        if let Some(i) = writes.iter().position(|w| w == target) {
            writes.remove(i);
        }
    }

    /// `Err` with a user-facing reason while `path` is in a moving project,
    /// or in a project folder a rename already moved away (a save dialog
    /// opened before the move and answered after it): the write would
    /// recreate the old folder.
    pub(crate) fn refuse_while_project_moves(&self, path: &Path, what: &str) -> anyhow::Result<()> {
        if self.path_is_in_moving_project(path) {
            anyhow::bail!(
                "Cannot {what} {} while its project folder is being renamed. Try again once the rename has finished.",
                path.display()
            );
        }
        if let Some(moved) = self.moved_project_dir_holding(path) {
            anyhow::bail!(
                "Cannot {what} {}: its project folder was renamed to {}. Choose the location again.",
                path.display(),
                moved.new.display()
            );
        }
        Ok(())
    }

    /// [`Self::refuse_while_project_moves`], with the refusal shown as
    /// `notice` and logged to the Messages panel. True when refused.
    pub(crate) fn refuse_with_notice_while_project_moves(
        &mut self,
        path: &Path,
        what: &str,
        notice: fn(String) -> crate::app::state::ErrorNotice,
    ) -> bool {
        let Err(e) = self.refuse_while_project_moves(path, what) else {
            return false;
        };
        crate::diagnostics::log_warning(e.to_string());
        self.document_state.error_notice = Some(notice(e.to_string()));
        true
    }

    /// The moved-away project folder `path` lies in, while it is still gone.
    /// Once it exists again (re-created by hand, a New Project there) or an
    /// open project lives there, writes and opens there are the user's own.
    fn moved_project_dir_holding(
        &self,
        path: &Path,
    ) -> Option<&crate::app::state::MovedProjectDir> {
        use crate::app::state::scope::path_is_under;
        let ds = &self.document_state;
        ds.moved_project_dirs
            .iter()
            .filter(|m| path_is_under(path, &m.old, m.fold))
            .find(|m| {
                !ds.projects
                    .iter()
                    .any(|p| path_is_under(p.dir(), &m.old, m.fold))
                    // One stat, cheap enough for the update thread.
                    && !m.old.is_dir()
            })
    }

    /// Remember that `job` moved its folder, for
    /// [`Self::moved_project_dir_holding`].
    fn record_moved_project_dir(
        &mut self,
        job: &ProjectRenameJob,
        fold: crate::app::state::CaseFold,
    ) {
        let moved = &mut self.document_state.moved_project_dirs;
        // Moved back to an earlier name: that folder exists again.
        moved.retain(|m| !crate::app::state::scope::path_is_under(&job.new_dir, &m.old, m.fold));
        moved.push(crate::app::state::MovedProjectDir {
            old: job.old_dir.clone(),
            new: job.new_dir.clone(),
            fold,
        });
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
        let fold = ds
            .projects
            .iter()
            .find(|p| p.id == job.project_id)
            .map(|p| p.case_fold)
            .unwrap_or_default();
        let under = |p: &PathBuf| crate::app::state::scope::path_is_under(p, &job.old_dir, fold);
        if ds.project_writes.iter().any(under) {
            return Some(
                "A file is still being written into the project. Rename the project once it has finished.",
            );
        }
        // A recovery check still running for a file under the folder would
        // report for the old path and lose the copy it found.
        if ds.pending_opens.paths().any(under)
            || ds.recovery_check_due.iter().any(under)
            || ds.autosave.checking_paths().any(under)
        {
            return Some("A file is still opening. Rename the project once it has opened.");
        }
        if ds.pending_restores.paths().any(under) {
            return Some(
                "A History restore is still running. Rename the project once it has finished.",
            );
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
            crate::diagnostics::log_warning(format!(
                "Rename of project {} refused: {reason}",
                job.old_stem
            ));
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
        let recovery = self.document_state.autosave.dir().map(Path::to_path_buf);
        let open = self.open_document_paths();
        let fold = self.fold_for_path(&job.old_dir);
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let journalled = recovery.as_deref().map(|recovery| {
                        journal_closed_copies(recovery, &run.old_dir, &run.new_dir, &open, fold)
                    });
                    if let Err(why) = rename_project_on_disk(&run) {
                        if let (Some(recovery), Some(journalled)) = (&recovery, &journalled) {
                            unjournal_closed_copies(recovery, journalled);
                        }
                        return Err(why);
                    }
                    if let Some(recovery) = recovery {
                        carry_closed_copies(&recovery, &run.old_dir, &run.new_dir, &open, fold);
                    }
                    Ok(())
                })
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

    /// Every document open in a tab or an editor (a parked engine too). Their
    /// recovery copies follow through the re-key; the rest are carried on
    /// disk by [`carry_closed_copies`].
    fn open_document_paths(&self) -> std::collections::HashSet<PathBuf> {
        let ds = &self.document_state;
        ds.tabs
            .iter()
            .map(|t| t.path.clone())
            .chain(ds.engines.keys().cloned())
            .chain(ds.symbol_editors.keys().cloned())
            .chain(ds.footprint_editors.keys().cloned())
            .collect()
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
            // Move failed AND putting the `.snxprj` back failed: the disk
            // has it under its new name in the old folder. Point the
            // in-memory project there so it does not reference a missing file.
            let old_file = job.old_project_file();
            let renamed_in_old_dir = job.old_dir.join(format!("{}.snxprj", job.new_stem));
            if !old_file.exists()
                && renamed_in_old_dir.exists()
                && let Some(idx) = self
                    .document_state
                    .projects
                    .iter()
                    .position(|p| p.id == job.project_id)
            {
                self.rekey_renamed_project_file(idx, &old_file, &renamed_in_old_dir, &job.new_stem);
                self.replace_recent_project(&job.recent_key, &renamed_in_old_dir);
                self.queue_project_rename_commit(
                    &old_file,
                    &renamed_in_old_dir,
                    &job.old_stem,
                    &job.new_stem,
                );
                self.refresh_panel_ctx();
            }
            self.run_deferred_git_commits(job, None);
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
            let fold = self.document_state.projects[idx].case_fold;
            self.rekey_project_folder(idx, job);
            self.record_moved_project_dir(job, fold);
            self.queue_project_rename_commit(
                &job.new_dir.join(format!("{}.snxprj", job.old_stem)),
                &new_file,
                &job.old_stem,
                &job.new_stem,
            );
            self.run_deferred_git_commits(job, Some(fold));
        } else {
            self.run_deferred_git_commits(job, None);
        }
        self.replace_recent_project(&job.recent_key, &new_file);
        self.ui_state.rename_dialog = None;
        self.refresh_panel_ctx();
        self.autosave_flush_moves()
    }

    /// Update the in-memory project and any tab / engine that referenced
    /// the `.snxprj` path itself after the file moved from `from` to `to`.
    fn rekey_renamed_project_file(&mut self, idx: usize, from: &Path, to: &Path, new_stem: &str) {
        if let Some(loaded) = self.document_state.projects.get_mut(idx) {
            loaded.path = to.to_path_buf();
            loaded.data.name = new_stem.to_string();
        }
        // A failure or queued save names the file by its old path.
        self.document_state
            .document_saves
            .forget_where(|p| p == from);
        crate::diagnostics::log_info(format!(
            "[rename] Project file {} renamed to {}",
            from.display(),
            to.display()
        ));
        for tab in self.document_state.tabs.iter_mut() {
            if tab.path == from {
                tab.path = to.to_path_buf();
                tab.title = if tab.kind.as_project().is_some() {
                    crate::app::project_tab::project_tab_title(to)
                } else {
                    new_stem.to_string()
                };
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
        self.rekey_undocked_windows(from, to);
    }

    /// Move every path-keyed record under the old project folder to the new
    /// one: open documents (tabs, engines, editors, dirty markers, page
    /// setup, undocked windows) and the project's own directory, which is
    /// the git root.
    fn rekey_project_folder(&mut self, idx: usize, job: &ProjectRenameJob) {
        let fold = self
            .document_state
            .projects
            .get(idx)
            .map(|p| p.case_fold)
            .unwrap_or_default();
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
            .filter(|p| crate::app::state::scope::path_is_under(p, &job.old_dir, fold))
            .collect();
        under.sort();
        under.dedup();
        for old in under {
            let new = crate::app::state::scope::rebase_under(&old, &job.old_dir, &job.new_dir);
            self.rekey_open_document(&old, &new);
        }
        if let Some(loaded) = self.document_state.projects.get_mut(idx) {
            loaded.path = job.new_project_file();
            loaded.data.dir = job.new_dir.to_string_lossy().into_owned();
        }
    }

    /// Run the commits asked for while `job` moved the folder. `moved` is
    /// the project's case rule when the folder now lives at `job.new_dir`,
    /// so paths under the old folder are rebased there first.
    fn run_deferred_git_commits(
        &mut self,
        job: &ProjectRenameJob,
        moved: Option<crate::app::state::CaseFold>,
    ) {
        let (run, keep): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.document_state.deferred_git_commits)
                .into_iter()
                .partition(|c| c.project == job.project_id);
        self.document_state.deferred_git_commits = keep;
        let rebase = |p: PathBuf| match moved {
            Some(fold) if crate::app::state::scope::path_is_under(&p, &job.old_dir, fold) => {
                crate::app::state::scope::rebase_under(&p, &job.old_dir, &job.new_dir)
            }
            _ => p,
        };
        for commit in run {
            self.queue_project_git_commit(
                &rebase(commit.file_path),
                &commit.message,
                commit.replace_message,
                commit.removed_path.map(&rebase).as_deref(),
            );
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
