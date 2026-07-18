//! Close-Project + app-quit lifecycle for the project-navigation dock.
//!
//! Extracted verbatim from the project-navigation dock handlers
//! (`handlers/dock/project_navigation`); pure code motion, zero
//! behaviour change.

use super::*;

impl Signex {
    /// Close every tab backed by the project whose root is at
    /// `tree_path[0]`, then drop the project from the workspace and
    /// promote a sibling (or `None`) to active. Mirrors Altium's
    /// Projects-panel right-click → Close Project.
    ///
    /// If any file in the project's directory has unsaved edits
    /// (`dirty_paths` intersects the project dir), opens the
    /// `ProjectCloseConfirm` modal first instead of closing
    /// immediately. The modal's choice handler
    /// (`handle_project_close_confirm`) calls back into this method
    /// once the user picks Save All or Discard All.
    pub(super) fn close_project_at_tree_path(&mut self, tree_path: &[usize]) -> Task<Message> {
        let Some(&project_idx) = tree_path.first() else {
            return Task::none();
        };
        let Some(project) = self.document_state.projects.get(project_idx) else {
            return Task::none();
        };
        let project_dir = project.path.parent().map(|p| p.to_path_buf());
        // Collect dirty paths inside this project's directory tree.
        // `dirty_paths` is project-scoped by ancestor check —
        // primitive editors live under `<project>/<lib>.snxlib/
        // symbols|footprints/<file>` so the immediate parent dir is
        // never the project dir itself; a strict `parent() == dir`
        // check would miss every nested primitive draft (which is
        // why Add New ▸ Symbol drafts weren't surfacing in the
        // close-project prompt). Walk ancestors instead. The
        // project's own `.snxprj` file (when added to dirty_paths
        // by the auto-attach-library flow) sits exactly at the
        // project root and is also picked up here.
        let dirty: Vec<std::path::PathBuf> = if let Some(dir) = project_dir.as_deref() {
            let mut v: Vec<std::path::PathBuf> = self
                .document_state
                .dirty_paths
                .iter()
                .filter(|p| p.starts_with(dir))
                .cloned()
                .collect();
            v.sort();
            v
        } else {
            Vec::new()
        };

        if !dirty.is_empty() {
            self.ui_state.project_close_confirm = Some(crate::app::ProjectCloseConfirmState {
                tree_path: tree_path.to_vec(),
                project_name: project.data.name.clone(),
                dirty_paths: dirty,
            });
            return Task::none();
        }

        self.execute_close_project_at_tree_path(tree_path)
    }

    /// Inner close-project flow that actually drops tabs + the
    /// project entry. Called either directly (clean project) or via
    /// the project-close confirm modal once the user picks Save All
    /// or Discard All. Trusts that `dirty_paths` for this project is
    /// already empty (callers ensure this).
    fn execute_close_project_at_tree_path(&mut self, tree_path: &[usize]) -> Task<Message> {
        let Some(&project_idx) = tree_path.first() else {
            return Task::none();
        };
        let Some(target_id) = self.document_state.projects.get(project_idx).map(|p| p.id) else {
            return Task::none();
        };

        // Collect tab indices owned by this project, highest first —
        // `close_tab_now` removes by index, so descending order keeps
        // untouched indices valid as we iterate.
        let mut indices: Vec<usize> = self
            .document_state
            .tabs
            .iter()
            .enumerate()
            .filter_map(|(i, t)| (t.project_id == Some(target_id)).then_some(i))
            .collect();
        indices.sort_unstable_by(|a, b| b.cmp(a));
        let mut tasks: Vec<Task<Message>> = Vec::with_capacity(indices.len());
        for idx in indices {
            tasks.push(self.close_tab_now(idx));
        }

        // Drop the project's parked engines too — without this, an
        // engine that was kept alive by `dirty_paths` would leak
        // through project close. Save All / Discard All have already
        // pruned `dirty_paths` for the project, so this loop just
        // sweeps the engine map for any path under this project's
        // directory.
        if let Some(project) = self.document_state.projects.get(project_idx)
            && let Some(dir) = project.path.parent()
        {
            let dir = dir.to_path_buf();
            self.document_state
                .engines
                .retain(|p, _| p.parent() != Some(&dir));
        }

        // Drop the project from the workspace, then pick a new active
        // project — the one that now sits in the same slot (was
        // immediately after the closed one), else the previous slot,
        // else None when the workspace is empty.
        self.document_state.projects.retain(|p| p.id != target_id);
        self.document_state.active_project = self
            .document_state
            .projects
            .get(project_idx)
            .or_else(|| {
                project_idx
                    .checked_sub(1)
                    .and_then(|i| self.document_state.projects.get(i))
            })
            .map(|p| p.id);

        self.refresh_panel_ctx();

        if tasks.is_empty() {
            Task::none()
        } else {
            Task::batch(tasks)
        }
    }

    /// Resolve the user's Save All / Discard All / Cancel choice on
    /// the project-close confirmation modal. Owned by this module
    /// because it's a follow-up of `close_project_at_tree_path`.
    pub(crate) fn handle_project_close_confirm(
        &mut self,
        choice: crate::app::ProjectCloseChoice,
    ) -> Task<Message> {
        use crate::app::ProjectCloseChoice;
        let Some(state) = self.ui_state.project_close_confirm.take() else {
            return Task::none();
        };
        match choice {
            ProjectCloseChoice::Cancel => Task::none(),
            ProjectCloseChoice::SaveAll => {
                // Save every dirty file. Files without a live engine
                // shouldn't exist if `dirty_paths` is consistent with
                // `engines` — count those as failures so the user
                // sees something is wrong instead of silently losing
                // the dirty state. If ANY save fails, the project
                // stays open + the user gets an error modal listing
                // the unsaved files.
                let mut failed: Vec<std::path::PathBuf> = Vec::new();
                for path in &state.dirty_paths {
                    if let Some(engine) = self.document_state.engines.get_mut(path) {
                        match engine.save() {
                            Ok(_) => {
                                self.document_state.dirty_paths.remove(path);
                                if let Some(tab) = self
                                    .document_state
                                    .tabs
                                    .iter_mut()
                                    .find(|t| &t.path == path)
                                {
                                    tab.dirty = false;
                                }
                            }
                            Err(err) => {
                                crate::diagnostics::log_error(
                                    "Failed to save during project close",
                                    &anyhow::anyhow!("{err}"),
                                );
                                failed.push(path.clone());
                            }
                        }
                    } else {
                        crate::diagnostics::log_info(format!(
                            "Project close: no live engine for dirty {} — cannot save",
                            path.display()
                        ));
                        failed.push(path.clone());
                    }
                }
                if !failed.is_empty() {
                    let listing: Vec<String> = failed
                        .iter()
                        .filter_map(|p| {
                            p.file_name()
                                .and_then(|s| s.to_str())
                                .map(|s| s.to_string())
                        })
                        .collect();
                    self.document_state.export_error = Some(format!(
                        "Could not save {} file(s) — project not closed:\n  {}",
                        failed.len(),
                        listing.join("\n  ")
                    ));
                    return Task::none();
                }
                self.execute_close_project_at_tree_path(&state.tree_path)
            }
            ProjectCloseChoice::DiscardAll => {
                // Drop dirty engines + clear the dirty flags. The
                // engines map is also swept by
                // `execute_close_project_at_tree_path` afterwards,
                // but doing it here too keeps the flow symmetric
                // with SaveAll (after this match the project's
                // dirty_paths entries are gone either way).
                for path in &state.dirty_paths {
                    self.document_state.engines.remove(path);
                    self.document_state.dirty_paths.remove(path);
                    if let Some(tab) = self
                        .document_state
                        .tabs
                        .iter_mut()
                        .find(|t| &t.path == path)
                    {
                        tab.dirty = false;
                    }
                }
                self.execute_close_project_at_tree_path(&state.tree_path)
            }
        }
    }

    /// Entry point for every app-exit request — chrome ✕, File ▸ Exit,
    /// and OS close (Alt+F4) all funnel here. If any document in the
    /// workspace has unsaved edits (`dirty_paths` non-empty), opens the
    /// app-quit confirmation modal instead of exiting; otherwise closes
    /// the main window, which the daemon turns into process exit via
    /// `SecondaryWindowClosed`.
    pub(crate) fn handle_app_quit_requested(&mut self) -> Task<Message> {
        // Guard against a second close request stacking a duplicate
        // modal while the first is still up.
        if self.ui_state.app_quit_confirm.is_some() {
            return Task::none();
        }
        if self.document_state.dirty_paths.is_empty() {
            return self.close_main_window_now();
        }
        let mut dirty: Vec<std::path::PathBuf> =
            self.document_state.dirty_paths.iter().cloned().collect();
        dirty.sort();
        self.ui_state.app_quit_confirm =
            Some(crate::app::AppQuitConfirmState { dirty_paths: dirty });
        Task::none()
    }

    /// Actually close the main window. In `iced::daemon` this fires a
    /// `Closed` event → `SecondaryWindowClosed(main)` → `iced::exit()`,
    /// so all shutdown bookkeeping stays on the existing path.
    fn close_main_window_now(&self) -> Task<Message> {
        match self.ui_state.main_window_id {
            Some(id) => iced::window::close(id),
            None => iced::exit(),
        }
    }

    /// Resolve the user's Save All / Discard All / Cancel choice on the
    /// app-quit confirmation modal.
    pub(crate) fn handle_app_quit_confirm(
        &mut self,
        choice: crate::app::ProjectCloseChoice,
    ) -> Task<Message> {
        use crate::app::ProjectCloseChoice;
        let Some(state) = self.ui_state.app_quit_confirm.take() else {
            return Task::none();
        };
        match choice {
            ProjectCloseChoice::Cancel => Task::none(),
            ProjectCloseChoice::DiscardAll => self.close_main_window_now(),
            ProjectCloseChoice::SaveAll => {
                // Save every dirty file that has a live engine. A dirty
                // `.snxprj` or a symbol/footprint editor draft has no
                // engine and cannot be saved through this path yet
                // (tracked in #104). Rather than exit and lose them, we
                // keep Signex open and tell the user which files still
                // need a manual save.
                let mut failed: Vec<std::path::PathBuf> = Vec::new();
                for path in &state.dirty_paths {
                    if let Some(engine) = self.document_state.engines.get_mut(path) {
                        match engine.save() {
                            Ok(_) => {
                                self.document_state.dirty_paths.remove(path);
                                if let Some(tab) = self
                                    .document_state
                                    .tabs
                                    .iter_mut()
                                    .find(|t| &t.path == path)
                                {
                                    tab.dirty = false;
                                }
                            }
                            Err(err) => {
                                crate::diagnostics::log_error(
                                    "Failed to save during app exit",
                                    &anyhow::anyhow!("{err}"),
                                );
                                failed.push(path.clone());
                            }
                        }
                    } else {
                        failed.push(path.clone());
                    }
                }
                if failed.is_empty() {
                    self.close_main_window_now()
                } else {
                    let listing: Vec<String> = failed
                        .iter()
                        .filter_map(|p| {
                            p.file_name()
                                .and_then(|s| s.to_str())
                                .map(|s| s.to_string())
                        })
                        .collect();
                    self.document_state.export_error = Some(format!(
                        "Could not save {} file(s) — Signex stayed open so nothing is lost. \
                         Save them manually, or choose Discard All to exit anyway:\n  {}",
                        failed.len(),
                        listing.join("\n  ")
                    ));
                    Task::none()
                }
            }
        }
    }
}
