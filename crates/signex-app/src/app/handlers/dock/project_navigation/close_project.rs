//! Close-Project + app-quit lifecycle for the project-navigation dock.
//!
//! Extracted verbatim from the project-navigation dock handlers
//! (`handlers/dock/project_navigation`); pure code motion, zero
//! behaviour change.

use super::*;
use crate::app::state::ProjectId;
use crate::app::state::sheet_refs::{SheetRefs, StaleRefs};
use crate::app::state::volume_case::directory_ignores_case;

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
    ///
    /// Attributing a sibling project's child sheets reads sheet files, so
    /// this only starts that read on a worker thread; the gate itself runs
    /// in [`handle_close_refs_resolved`](Self::handle_close_refs_resolved)
    /// when the result comes back.
    pub(super) fn close_project_at_tree_path(&mut self, tree_path: &[usize]) -> Task<Message> {
        let Some(&project_idx) = tree_path.first() else {
            return Task::none();
        };
        let Some(project_id) = self.document_state.projects.get(project_idx).map(|p| p.id) else {
            return Task::none();
        };
        self.spawn_close_refs_resolve(project_id)
    }

    /// Start the off-thread child-sheet read for closing `project_id`. Used by
    /// the click and again when the open-project set changed under a result
    /// already in hand (a project opened meanwhile has unresolved children).
    fn spawn_close_refs_resolve(&self, project_id: ProjectId) -> Task<Message> {
        let inputs = self.document_state.sheet_ref_inputs();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    Box::new(inputs.resolve(directory_ignores_case))
                })
                .await
                .map_err(|e| format!("sheet-reference worker failed: {e}"))
            },
            move |result| Message::Project(ProjectMsg::CloseRefsResolved { project_id, result }),
        )
    }

    /// Everything the Close Project task reads, resolved on the calling
    /// thread. The body the task runs off-thread, exposed so a regression
    /// test can complete the task without an async runtime.
    #[doc(hidden)]
    pub fn resolve_sheet_refs_blocking(&self) -> Result<Box<SheetRefs>, String> {
        Ok(Box::new(
            self.document_state
                .sheet_ref_inputs()
                .resolve(directory_ignores_case),
        ))
    }

    /// The Close Project gate, run once the sheet references are resolved:
    /// open the confirm modal when the project owns unsaved work, otherwise
    /// close it.
    ///
    /// Every dirty document of the project ends up in exactly one place: the
    /// modal's save / discard list, or the kept list (another open project
    /// claims it) which the modal and the Messages panel name and which stays
    /// open when the project closes.
    pub(crate) fn handle_close_refs_resolved(
        &mut self,
        project_id: ProjectId,
        result: Result<Box<SheetRefs>, String>,
    ) -> Task<Message> {
        let sheet_refs = match result {
            Ok(refs) => *refs,
            Err(reason) => {
                crate::diagnostics::log_error(
                    "Close Project cancelled: child-sheet ownership could not be resolved",
                    &anyhow::anyhow!(reason),
                );
                self.refresh_panel_ctx();
                return Task::none();
            }
        };
        let Some(project_idx) = self
            .document_state
            .projects
            .iter()
            .position(|p| p.id == project_id)
        else {
            return Task::none();
        };
        // The gate reads `dirty_paths`; do not rely on every edit site
        // having refreshed the panel context since its last editor edit.
        self.sync_editor_dirty_paths();
        // Engines are re-read here (no IO), so an edit made while the Task ran
        // is seen. Evidence that no longer describes the workspace is read
        // again instead of acted on.
        if !self.document_state.refs_are_current(&sheet_refs) {
            return self.spawn_close_refs_resolve(project_id);
        }
        let Some(project) = self.document_state.projects.get(project_idx) else {
            return Task::none();
        };
        let project_name = project.data.name.clone();
        // The project's dirty paths, split into the ones it saves / discards
        // and the ones another open project claims. Primitive editors live
        // under `<project>/<lib>.snxlib/symbols|footprints/<file>`, so the
        // scan walks ancestors rather than the immediate parent, and the
        // project's own `.snxprj` sits at the root and is picked up too.
        // Component Preview rows are keyed by row, not file, so they are
        // not in `dirty_paths`; the scan places them too (#666): a row is
        // the project's only when its library is the project's own.
        let scan = self.current_close_scan(project_idx, &sheet_refs);
        if let Some(line) = scan.warning(&project_name) {
            crate::diagnostics::log_warning(line);
            self.refresh_panel_ctx();
        }

        if !scan.mine.is_empty() || !scan.mine_rows.is_empty() {
            if let Some(reason) = sheet_refs.discard_blocked_reason() {
                crate::diagnostics::log_warning(format!(
                    "Close Project '{project_name}': {reason}"
                ));
                self.refresh_panel_ctx();
            }
            self.ui_state.project_close_confirm = Some(crate::app::ProjectCloseConfirmState {
                project_id,
                project_name,
                dirty_paths: scan.mine,
                dirty_rows: scan.mine_rows,
                kept_paths: scan.kept,
                sheet_refs,
            });
            return Task::none();
        }

        self.execute_close_project(project_id, &sheet_refs, &scan.kept)
    }

    /// The closing project's dirty documents split by the strict rule, from
    /// the engines and library editors as they are now.
    pub(super) fn current_close_scan(
        &self,
        project_idx: usize,
        sheet_refs: &SheetRefs,
    ) -> crate::app::state::sheet_refs::CloseScan {
        self.document_state
            .close_scan(sheet_refs, project_idx, &self.library.dirty_editors())
    }

    /// Inner close-project flow that actually drops tabs + the
    /// project entry. Called either directly (clean project) or via
    /// the project-close confirm modal once the user picks Save All
    /// or Discard All. Trusts that `dirty_paths` for this project is
    /// already empty apart from `kept` (callers ensure this).
    ///
    /// `kept` are dirty documents another open project claims: their tabs
    /// stay open (re-homed to no project) and their engines are never swept.
    pub(super) fn execute_close_project(
        &mut self,
        project_id: ProjectId,
        sheet_refs: &SheetRefs,
        kept: &[std::path::PathBuf],
    ) -> Task<Message> {
        let Some(project_idx) = self
            .document_state
            .projects
            .iter()
            .position(|p| p.id == project_id)
        else {
            crate::diagnostics::log_warning(
                "Close Project cancelled: the project is no longer open",
            );
            self.refresh_panel_ctx();
            return Task::none();
        };
        let target_id = project_id;

        // A kept document's tab outlives the project it was opened under.
        for tab in &mut self.document_state.tabs {
            if tab.project_id == Some(target_id) && kept.contains(&tab.path) {
                tab.project_id = None;
            }
        }

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
        if let Some(project) = self.document_state.projects.get(project_idx) {
            let dir = project.dir().to_path_buf();
            let reach = self.document_state.reach_sets(sheet_refs);
            let sweep: Vec<std::path::PathBuf> = self
                .document_state
                .engines
                .keys()
                .filter(|p| {
                    (p.parent() == Some(&dir) || reach.reached_by(project_idx, p))
                        && !reach.reached_by_other(project_idx, p)
                        && !kept.contains(p)
                })
                .cloned()
                .collect();
            for path in sweep {
                self.forget_net_colors_of(&path);
                self.document_state.engines.remove(&path);
            }
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
        if choice == ProjectCloseChoice::Cancel {
            return Task::none();
        }
        // Look the project up again by id: the list may have shifted while
        // the modal was up.
        if !self
            .document_state
            .projects
            .iter()
            .any(|p| p.id == state.project_id)
        {
            crate::diagnostics::log_warning(format!(
                "Close Project '{}' aborted: the project is no longer open",
                state.project_name
            ));
            self.refresh_panel_ctx();
            return Task::none();
        }
        // A project opened since the references were read has disk-only
        // children nobody resolved; read again and re-prompt rather than act
        // on stale ownership.
        self.sync_editor_dirty_paths();
        let stale = self.document_state.stale_causes(&state.sheet_refs);
        if !stale.is_empty() {
            crate::diagnostics::log_warning(format!(
                "Close Project '{}': {}, so the sheet references were re-read; confirm again",
                state.project_name,
                StaleRefs::describe_all(&stale),
            ));
            self.refresh_panel_ctx();
            return self.spawn_close_refs_resolve(state.project_id);
        }
        // An edit since the prompt (an engine reference, a new dirty
        // document) can move a document between `mine` and `kept`. Never act
        // on the stale lists: run the gate again so the user sees the new ones.
        if let Some(idx) = self
            .document_state
            .projects
            .iter()
            .position(|p| p.id == state.project_id)
        {
            let scan = self.current_close_scan(idx, &state.sheet_refs);
            if scan.mine != state.dirty_paths
                || scan.kept != state.kept_paths
                || scan.mine_rows != state.dirty_rows
            {
                crate::diagnostics::log_warning(format!(
                    "Close Project '{}': the unsaved documents changed since the prompt; review the list and confirm again",
                    state.project_name
                ));
                return self
                    .handle_close_refs_resolved(state.project_id, Ok(Box::new(state.sheet_refs)));
            }
        }
        if choice == ProjectCloseChoice::DiscardAll
            && let Some(reason) = state.sheet_refs.discard_blocked_reason()
        {
            // The dialog does not offer Discard All here; a forced message is
            // refused, logged, and the modal stays up for Save All / Cancel.
            crate::diagnostics::log_warning(format!(
                "Close Project '{}': Discard All refused. {reason}",
                state.project_name
            ));
            self.ui_state.project_close_confirm = Some(state);
            self.refresh_panel_ctx();
            return Task::none();
        }
        match choice {
            ProjectCloseChoice::Cancel => Task::none(),
            ProjectCloseChoice::SaveAll => {
                // Start every dirty file's write off the UI thread; the
                // project closes once all of them completed, or stays open
                // with an error modal naming each file AND why it did not
                // save (`handle_save_all_settled`).
                self.begin_save_all(
                    "Save All",
                    &state.dirty_paths,
                    &state.dirty_rows,
                    crate::app::contracts::SaveAllNext::CloseProject {
                        project_id: state.project_id,
                        sheet_refs: Box::new(state.sheet_refs),
                        kept_paths: state.kept_paths,
                    },
                )
            }
            ProjectCloseChoice::DiscardAll => {
                // Drop dirty engines + clear the dirty flags. The
                // engines map is also swept by
                // `execute_close_project` afterwards,
                // but doing it here too keeps the flow symmetric
                // with SaveAll (after this match the project's
                // dirty_paths entries are gone either way).
                let mut tasks: Vec<Task<Message>> = Vec::new();
                for path in &state.dirty_paths {
                    self.forget_net_colors_of(path);
                    self.document_state
                        .document_saves
                        .forget_where(|p| p == path);
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
                    tasks.push(self.discard_primitive_editor(path));
                }
                // Component Preview rows outlive the project's tabs, so
                // discard their drafts explicitly.
                for address in &state.dirty_rows {
                    tasks.push(self.close_component_preview(address));
                }
                tasks.push(self.execute_close_project(
                    state.project_id,
                    &state.sheet_refs,
                    &state.kept_paths,
                ));
                Task::batch(tasks)
            }
        }
    }

    /// Drop the symbol/footprint editor behind `path`, and its tab, so a
    /// discarded draft cannot linger as a clean-looking editor that a
    /// later edit would re-dirty and Ctrl+S would write. Such a tab can
    /// have `project_id: None` (a library nested under the project), so
    /// the project-close sweep by `project_id` would never reach it.
    fn discard_primitive_editor(&mut self, path: &std::path::Path) -> Task<Message> {
        self.document_state.symbol_editors.remove(path);
        self.document_state.footprint_editors.remove(path);
        let tab_idx = self.document_state.tabs.iter().position(|t| {
            t.path == path
                && matches!(
                    t.kind,
                    crate::app::TabKind::SymbolEditor(_) | crate::app::TabKind::FootprintEditor(_)
                )
        });
        match tab_idx {
            Some(idx) => self.close_tab_now(idx),
            None => Task::none(),
        }
    }

    /// Entry point for every app-exit request — chrome ✕, File ▸ Exit,
    /// and OS close (Alt+F4) all funnel here. If any document in the
    /// workspace has unsaved edits (`dirty_paths` non-empty, or a dirty
    /// Component Preview row), opens the
    /// app-quit confirmation modal instead of exiting; otherwise closes
    /// the main window, which the daemon turns into process exit via
    /// `SecondaryWindowClosed`.
    pub(crate) fn handle_app_quit_requested(&mut self) -> Task<Message> {
        // Same hardening as `close_project_at_tree_path`: the quit gate
        // must see footprint and symbol edits even if no panel refresh ran after them.
        self.sync_editor_dirty_paths();
        // Guard against a second close request stacking a duplicate
        // modal while the first is still up.
        if self.ui_state.app_quit_confirm.is_some() {
            return Task::none();
        }
        let dirty_rows = self.library.dirty_editors();
        if self.document_state.dirty_paths.is_empty() && dirty_rows.is_empty() {
            return self.close_main_window_now();
        }
        let mut dirty: Vec<std::path::PathBuf> =
            self.document_state.dirty_paths.iter().cloned().collect();
        dirty.sort();
        self.ui_state.app_quit_confirm = Some(crate::app::AppQuitConfirmState {
            dirty_paths: dirty,
            dirty_rows,
        });
        Task::none()
    }

    /// Actually close the main window. In `iced::daemon` this fires a
    /// `Closed` event → `SecondaryWindowClosed(main)` → `iced::exit()`,
    /// so all shutdown bookkeeping stays on the existing path.
    pub(super) fn close_main_window_now(&mut self) -> Task<Message> {
        // Flush a debounced dock-layout write first so the last layout is
        // never lost to the quit (#692).
        let close = match self.ui_state.main_window_id {
            Some(id) => iced::window::close(id),
            None => iced::exit(),
        };
        self.flush_quit_writes_then(close)
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
                // Start every dirty file's write off the UI thread; Signex
                // closes once all of them completed, and stays open with a
                // reason for anything still unsaved so nothing is lost.
                self.begin_save_all(
                    "Save All (exit)",
                    &state.dirty_paths,
                    &state.dirty_rows,
                    crate::app::contracts::SaveAllNext::Quit,
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::app::Signex;

    /// An app with one loaded project at `dir` and a footprint editor
    /// flagged dirty WITHOUT any `refresh_panel_ctx` since — the state an
    /// edit site that forgot the refresh leaves behind.
    fn app_with_unsynced_dirty_footprint(dir: &std::path::Path) -> (Signex, PathBuf) {
        let (mut app, _task) = Signex::new();
        let id = app.document_state.mint_project_id();
        app.document_state.projects.push(crate::app::LoadedProject {
            id,
            path: dir.join("p.snxprj"),
            data: signex_types::project::ProjectData {
                name: "p".into(),
                dir: dir.to_string_lossy().into_owned(),
                schematic_root: None,
                pcb_file: None,
                sheets: Vec::new(),
                variant_definitions: Vec::new(),
                active_variant: None,
                libraries: Vec::new(),
                enable_git: false,
                unknown: Default::default(),
            },
            pending_libraries: std::collections::HashMap::new(),
            case_fold: Default::default(),
            case_probed: false,
        });
        let path = dir.join("lib.snxlib").join("footprints").join("g.snxfpt");
        let file =
            signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("fp"));
        let mut editor = crate::app::FootprintEditorState::new(path.clone(), file);
        editor.dirty = true;
        app.document_state
            .footprint_editors
            .insert(path.clone(), editor);
        (app, path)
    }

    #[test]
    fn quit_gate_sees_a_dirty_footprint_without_a_panel_refresh() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, path) = app_with_unsynced_dirty_footprint(dir.path());

        let _ = app.handle_app_quit_requested();

        let modal = app
            .ui_state
            .app_quit_confirm
            .as_ref()
            .expect("quit must prompt");
        assert!(modal.dirty_paths.contains(&path));
    }

    #[test]
    fn project_close_gate_keeps_a_dirty_footprint_without_a_panel_refresh() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, path) = app_with_unsynced_dirty_footprint(dir.path());

        let project_id = app.document_state.projects[0].id;
        let task = app.close_project_at_tree_path(&[0]);
        assert_eq!(task.units(), 1, "the disk read must run off-thread");
        let result = app.resolve_sheet_refs_blocking();
        let _ = app.handle_close_refs_resolved(project_id, result);

        // The draft is not reached by any sheet walk, so it is kept (no
        // prompt, editor stays open) rather than discarded. It only gets that
        // far when the gate saw it despite the missing panel refresh.
        assert!(app.ui_state.project_close_confirm.is_none());
        assert!(app.document_state.dirty_paths.contains(&path));
        assert!(app.document_state.footprint_editors.contains_key(&path));
    }
}
