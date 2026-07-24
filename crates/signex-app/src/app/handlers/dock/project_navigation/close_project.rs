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

    /// Attempt to save a single dirty path during a Save-All flow
    /// (project close or app exit), routing by the kind of document the
    /// path backs:
    ///
    /// * a live schematic **engine** → `engine.save()`;
    /// * an open **symbol/footprint primitive editor** → `save_primitive_tab_at`;
    /// * the project's own **`.snxprj`** → `save_project_at_path`.
    ///
    /// Both Save-All loops previously handled *only* engines, so a dirty
    /// `.snxsym`/`.snxfpt` draft (e.g. a freshly-added symbol library
    /// like `SymbolLibrary5.snxsym`) or a dirty `.snxprj` always fell
    /// through to the failure list and blocked the close with a bogus
    /// "Could not save …" — the file was never actually attempted (#104).
    ///
    /// On success the path's dirty markers are cleared: the primitive
    /// and project savers clear their own; the engine leg clears them
    /// here. Returns the failure reason so the caller can tell the user
    /// WHY a file couldn't be saved instead of only naming it.
    fn try_save_dirty_path(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        use anyhow::Context as _;
        if let Some(engine) = self.document_state.engines.get_mut(path) {
            engine
                .save()
                .with_context(|| format!("save schematic {}", path.display()))?;
            self.document_state.dirty_paths.remove(path);
            if let Some(tab) = self
                .document_state
                .tabs
                .iter_mut()
                .find(|t| t.path == *path)
            {
                tab.dirty = false;
            }
            // Parity with the Ctrl+S schematic path (`save_active_document`):
            // auto-commit the saved sheet into the owning project's git
            // repo. Best-effort, no-op when `enable_git` is off — the
            // primitive and project legs already do the equivalent
            // internally, so the three routes stay uniform.
            let label = path
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            self.commit_save_to_project_git(path, &format!("Save {label}"));
            return Ok(());
        }
        if self.document_state.symbol_editors.contains_key(path)
            || self.document_state.footprint_editors.contains_key(path)
        {
            return self.save_primitive_tab_at(path);
        }
        if self.document_state.projects.iter().any(|p| p.path == path) {
            return self.save_project_at_path(path);
        }
        anyhow::bail!(
            "no open editor, engine, or project backs this file — it can't be saved automatically"
        )
    }

    /// Format the per-file failure lines (`name — reason`) for a
    /// Save-All error modal so the user sees *why* each file couldn't
    /// be saved, not just its name.
    fn save_all_failure_listing(failed: &[(std::path::PathBuf, String)]) -> String {
        failed
            .iter()
            .map(|(p, reason)| {
                let name = p
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("(unnamed file)");
                format!("{name} — {reason}")
            })
            .collect::<Vec<_>>()
            .join("\n  ")
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
                // Save every dirty file, routing each path to the right
                // saver (schematic engine / symbol/footprint editor /
                // project `.snxprj`) via `try_save_dirty_path`. If ANY
                // save fails the project stays open and the user gets an
                // error modal naming the file AND the reason it couldn't
                // be saved.
                let mut failed: Vec<(std::path::PathBuf, String)> = Vec::new();
                for path in &state.dirty_paths {
                    if let Err(err) = self.try_save_dirty_path(path) {
                        crate::diagnostics::log_error(
                            &format!("Save All: could not save {}", path.display()),
                            &err,
                        );
                        failed.push((path.clone(), format!("{err:#}")));
                    }
                }
                // Drop dirty dots for the paths that DID save (the engine
                // leg clears markers but doesn't refresh; the full-success
                // close path refreshes via `execute_close_project_at_tree_path`,
                // but the partial-failure path below would otherwise leave
                // stale red dots on already-saved sheets).
                self.refresh_panel_ctx();
                if !failed.is_empty() {
                    self.document_state.export_error = Some(format!(
                        "Could not save {} file(s) — project not closed:\n  {}",
                        failed.len(),
                        Self::save_all_failure_listing(&failed)
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
                // Save every dirty file, routing each path to the right
                // saver (schematic engine / symbol/footprint editor /
                // project `.snxprj`) via `try_save_dirty_path`. A dirty
                // symbol/footprint draft (e.g. a freshly-added
                // `SymbolLibrary5.snxsym`) or a dirty `.snxprj` used to
                // have no route here and always tripped a bogus "Export
                // Failed" (#104); they are now saved like any other.
                // Anything still un-saveable keeps Signex open with a
                // reason so nothing is lost.
                let mut failed: Vec<(std::path::PathBuf, String)> = Vec::new();
                for path in &state.dirty_paths {
                    if let Err(err) = self.try_save_dirty_path(path) {
                        crate::diagnostics::log_error(
                            &format!("Save All (exit): could not save {}", path.display()),
                            &err,
                        );
                        failed.push((path.clone(), format!("{err:#}")));
                    }
                }
                // Refresh so already-saved sheets drop their dirty dots on
                // the partial-failure path (the engine leg clears markers
                // but doesn't refresh); a no-op on the clean-exit path.
                self.refresh_panel_ctx();
                if failed.is_empty() {
                    self.close_main_window_now()
                } else {
                    self.document_state.export_error = Some(format!(
                        "Could not save {} file(s) — Signex stayed open so nothing is lost. \
                         Save them manually, or choose Discard All to exit anyway:\n  {}",
                        failed.len(),
                        Self::save_all_failure_listing(&failed)
                    ));
                    Task::none()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::app::{Signex, SymbolEditorState};

    /// Build a minimal app carrying one dirty, open symbol-library
    /// editor keyed at `path` — mirrors the state right after
    /// `Add New ▸ Symbol Library` followed by an edit (the exact repro
    /// for the "Export Failed — SymbolLibrary5.snxsym" close bug).
    fn app_with_dirty_symbol(path: &std::path::Path) -> Signex {
        let (mut app, _task) = Signex::new();
        let symbol = signex_library::Symbol::empty("Sym1");
        let file = signex_library::SymbolFile::from_symbol(symbol);
        app.document_state.symbol_editors.insert(
            path.to_path_buf(),
            SymbolEditorState::new(path.to_path_buf(), file),
        );
        app.document_state.dirty_paths.insert(path.to_path_buf());
        app
    }

    #[test]
    fn save_all_writes_a_dirty_symbol_library_instead_of_failing() {
        // Regression (#104): the Save-All loops routed only through
        // `engines`, so a dirty `.snxsym` editor draft — which has no
        // engine — always fell into the failure list and tripped a
        // bogus "Could not save … SymbolLibrary5.snxsym" instead of
        // ever being written.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SymbolLibrary5.snxsym");
        let mut app = app_with_dirty_symbol(&path);

        let result = app.try_save_dirty_path(&path);

        assert!(result.is_ok(), "symbol draft should save, got {result:?}");
        assert!(path.exists(), "the .snxsym must be written to disk");
        assert!(
            !app.document_state.dirty_paths.contains(&path),
            "dirty marker must clear once the symbol is saved",
        );
    }

    #[test]
    fn save_all_reports_a_reason_for_an_unbacked_path() {
        // A dirty path backing no engine / editor / project must
        // surface a descriptive error (not a silent failure) so the
        // Save-All dialog can tell the user WHY it couldn't save.
        let (mut app, _task) = Signex::new();
        let path = PathBuf::from("/nonexistent/Ghost.snxsym");

        let err = app.try_save_dirty_path(&path).unwrap_err();

        assert!(
            format!("{err:#}").contains("can't be saved automatically"),
            "unexpected error: {err:#}",
        );
    }
}
