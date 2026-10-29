//! Close-Project + app-quit lifecycle for the project-navigation dock.

use super::*;
use crate::app::handlers::document_files::FoldAnswer;
use crate::app::state::ProjectId;
use crate::app::state::project_case::fold_of;
use crate::app::state::scope::{CaseFold, path_is_under, path_key_with};
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

        // Reads and restores still running for this project's files must not
        // open or rewrite anything once it is gone (#859).
        if let Some(project) = self.document_state.projects.get(project_idx) {
            let dir = project.dir().to_path_buf();
            self.document_state.pending_opens.cancel_under(&dir);
            if self
                .ui_state
                .pending_create_sheet
                .as_ref()
                .is_some_and(|(parent, _)| parent.starts_with(&dir))
            {
                self.ui_state.pending_create_sheet = None;
            }
            self.document_state
                .pending_restores
                .cancel_under_retaining(&dir);
        }

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
        // sweeps the engine map for paths directly in this project's
        // directory or reached through its sheet hierarchy.
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
                self.document_state.remove_engine(&path);
            }
            self.cancel_placement_session_if_sheet_gone();
        }

        // Forget that this project was noted, so reopening it says it again.
        if let Some(closed) = self
            .document_state
            .projects
            .iter()
            .find(|p| p.id == target_id)
        {
            let closed_path = closed.path.clone();
            self.document_state
                .paused_git_projects
                .remove(std::path::Path::new(&closed.data.dir));
            self.document_state
                .instance_note_scopes
                .remove(&closed_path);
        }

        // A pricing walk keeps running: closing a project does not unmount its
        // libraries, and another open project may use the same one.
        // Drop the project from the workspace, then pick a new active
        // project — the one that now sits in the same slot (was
        // immediately after the closed one), else the previous slot,
        // else None when the workspace is empty.
        self.document_state.note_documents_changed();
        self.cancel_replace_job("the project was closed");
        self.document_state.projects.retain(|p| p.id != target_id);
        self.document_state.net_class_history.remove(&target_id);
        self.document_state.project_tabs.remove(&target_id);
        self.forget_net_class_picks_of(target_id);
        self.document_state.rules_history.remove(&target_id);
        self.document_state.profile_drift_noted.remove(&target_id);
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
    #[cfg(test)]
    pub(crate) fn try_save_dirty_path(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        self.try_save_dirty_path_with(path)
    }

    /// The project whose directory holds `path`; the deepest one when
    /// directories nest (the inner volume is the one the file lives on). An
    /// empty dir would claim every path, so it is skipped.
    pub(crate) fn project_owning(&self, path: &std::path::Path) -> Option<&LoadedProject> {
        self.document_state
            .projects
            .iter()
            .filter(|p| {
                !p.dir().as_os_str().is_empty() && path_is_under(path, p.dir(), p.case_fold)
            })
            .max_by_key(|p| p.dir().components().count())
    }

    /// The volume rule of the project that owns `path`. A loose document has
    /// no probe, so it keys by the host default.
    pub(crate) fn fold_for_path(&self, path: &std::path::Path) -> CaseFold {
        fold_of(self.project_owning(path))
    }

    pub(super) fn try_save_dirty_path_with(
        &mut self,
        path: &std::path::Path,
    ) -> anyhow::Result<()> {
        // The maps are keyed by the exact spelling a document was opened
        // under. Whether two spellings are ONE file is the identity rule
        // open, Ctrl+S and Save All use. If one file, the second write would
        // silently replace the first's edits: refuse unless every variant is
        // an engine encoding to identical bytes; keep every marker and say so.
        let FoldAnswer { fold, decided } = self.identity_decision(path);
        let held = self.held_spellings(path, fold);
        if held.len() > 1 {
            self.ensure_variants_agree(&held)?;
        }
        let target = self.write_target(&held).clone();
        self.save_held_path(&target)?;
        if !decided {
            // The spellings may be two files: the others were not written.
            return match self.unsaved_spellings(&held, &target) {
                Some((_, message)) => Err(anyhow::Error::new(SpellingsKept(message))),
                None => Ok(()),
            };
        }
        let key = path_key_with(path, fold);
        for variant in held.iter().filter(|p| **p != target) {
            if let Some(tab) = self
                .document_state
                .tabs
                .iter_mut()
                .find(|t| t.path == *variant)
            {
                tab.dirty = false;
            }
        }
        self.document_state
            .dirty_paths
            .retain(|p| path_key_with(p, fold) != key);
        Ok(())
    }

    /// The other spellings of a file still dirty after only `target` was
    /// written under an ASSUMED fold, with the Messages line that says so
    /// (advice first, paths last). `None` when none of them is dirty.
    pub(crate) fn unsaved_spellings(
        &self,
        held: &[std::path::PathBuf],
        target: &std::path::Path,
    ) -> Option<(Vec<std::path::PathBuf>, String)> {
        let kept: Vec<_> = held
            .iter()
            .filter(|p| p.as_path() != target && self.document_state.dirty_paths.contains(*p))
            .cloned()
            .collect();
        if kept.is_empty() {
            return None;
        }
        let names = kept
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(" and ");
        let count = kept.len();
        let noun = if count == 1 { "spelling" } else { "spellings" };
        let message = format!(
            "Save each spelling separately, folder case rule unknown, {count} {noun} kept unsaved: {names}"
        );
        Some((kept, message))
    }

    /// Ok only when every spelling is a schematic engine and all of them
    /// encode to the same bytes (so one write loses nothing). Otherwise errs
    /// with the advice first and the paths last, because Messages clips an
    /// entry; the caller logs it, so a refusal is one line.
    pub(crate) fn ensure_variants_agree(&self, held: &[std::path::PathBuf]) -> anyhow::Result<()> {
        let encode = |p: &std::path::PathBuf| -> Option<String> {
            let engine = self.document_state.engines.get(p)?;
            signex_types::format::SnxSchematic::new(engine.document().clone())
                .write_string()
                .ok()
        };
        let first = encode(&held[0]);
        if first.is_some() && held[1..].iter().all(|p| encode(p) == first) {
            return Ok(());
        }
        let names = held
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(" and ");
        anyhow::bail!(
            "Nothing was saved, no edits overwritten: one file is open under different \
             spellings with different contents, close one and retry: {names}"
        )
    }

    /// Every spelling an engine / editor / project is held under that matches
    /// `path` under `fold` (sorted, so the save order is deterministic), or
    /// `path` itself when nothing is held for it.
    pub(crate) fn held_spellings(
        &self,
        path: &std::path::Path,
        fold: CaseFold,
    ) -> Vec<std::path::PathBuf> {
        let key = path_key_with(path, fold);
        let doc = &self.document_state;
        let mut held: Vec<std::path::PathBuf> = doc
            .engines
            .keys()
            .chain(doc.symbol_editors.keys())
            .chain(doc.footprint_editors.keys())
            .chain(doc.projects.iter().map(|p| &p.path))
            .filter(|held| path_key_with(held, fold) == key)
            .cloned()
            .collect();
        held.sort();
        held.dedup();
        if held.is_empty() {
            held.push(path.to_path_buf());
        }
        held
    }

    /// The save legs of [`Self::try_save_dirty_path`], for a path already
    /// spelled the way the document is held.
    fn save_held_path(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        use anyhow::Context as _;
        // Reported as this file's failure; the rest of the batch still saves.
        self.refuse_save_while_restoring(path)?;
        // A write still running for this sheet could land after the one
        // below and put older content on disk; let it finish first.
        self.document_state.document_saves.barrier().wait();
        if let Some(engine) = self.document_state.engines.get_mut(path) {
            engine
                .save_with_net_colors(&self.ui_state.wire_color_overrides)
                .with_context(|| format!("save schematic {}", path.display()))?;
            // This save never reaches the async completion that removes
            // the recovery copy, so a write still running is stale here;
            // callers chain `autosave_sweep` to remove the copy on disk.
            self.document_state.autosave.supersede_in_flight(path);
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
            return self.save_primitive_tab_blocking(path);
        }
        if self.document_state.projects.iter().any(|p| p.path == path) {
            return self.save_project_at_path(path);
        }
        anyhow::bail!(
            "no open editor, engine, or project backs this file — it can't be saved automatically"
        )
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
                    self.document_state.remove_engine(path);
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
                tasks.push(self.autosave_sweep());
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
        self.warn_if_cascade_owed(path);
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
            // A second quit request raises the open confirm; it never stacks
            // another or exits.
            return self
                .detached_window_of(crate::app::state::ModalId::AppQuitConfirm)
                .map_or_else(Task::none, iced::window::gain_focus);
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
        self.ui_state.quit_confirm_refreshed = false;
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
            ProjectCloseChoice::DiscardAll => {
                // Discard only what the user was shown. An edit made since
                // (a window that was not locked, a route nobody thought of)
                // is NOT in the list: refresh the confirm, keep it open, and
                // leave every recovery copy alone.
                self.sync_editor_dirty_paths();
                let mut live: Vec<std::path::PathBuf> =
                    self.document_state.dirty_paths.iter().cloned().collect();
                live.sort();
                let live_rows = self.library.dirty_editors();
                let unseen = live.iter().any(|p| !state.dirty_paths.contains(p))
                    || live_rows.iter().any(|r| !state.dirty_rows.contains(r));
                if unseen {
                    crate::diagnostics::log_warning(
                        "Exit: unsaved edits changed since the prompt; review the list and \
                         confirm again. Nothing was discarded",
                    );
                    self.ui_state.app_quit_confirm = Some(crate::app::AppQuitConfirmState {
                        dirty_paths: live,
                        dirty_rows: live_rows,
                    });
                    self.ui_state.quit_confirm_refreshed = true;
                    return Task::none();
                }
                // The user declined to keep these edits, so their
                // recovery copies go too, and are gone before the exit.
                let cleanup = self.autosave_discard_all();
                cleanup.chain(self.close_main_window_now())
            }
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

/// A save wrote one spelling of a file whose other spellings, still dirty,
/// may be separate files under an assumed fold.
#[derive(Debug)]
pub(crate) struct SpellingsKept(String);

impl std::fmt::Display for SpellingsKept {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SpellingsKept {}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::app::{Signex, SymbolDocument};

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
            SymbolDocument::new(path.to_path_buf(), file),
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
    fn save_finds_a_document_held_under_a_case_variant_spelling() {
        let dir = tempfile::tempdir().unwrap();
        let held = dir.path().join("SymbolLibrary5.snxsym");
        let mut app = app_with_dirty_symbol(&held);
        let variant = dir.path().join("symbollibrary5.SNXSYM");

        let result = app.try_save_dirty_path_with(&variant);

        assert!(result.is_ok(), "case-variant save failed: {result:?}");
        assert!(held.exists(), "the held spelling must be the one written");
        assert!(app.document_state.dirty_paths.is_empty());
    }

    /// Two engines for one file, held as `Lib`/`lib`; the second carries a
    /// different edit (paper size) when `edit_second` is set.
    fn app_with_two_engine_variants(
        dir: &std::path::Path,
        edit_second: bool,
    ) -> (Signex, PathBuf, PathBuf) {
        let upper = dir.join("Lib.snxsch");
        let lower = dir.join("lib.snxsch");
        let (mut app, _task) = Signex::new();
        let second_paper = if edit_second { "A3" } else { "A4" };
        for (path, paper) in [(&upper, "A4"), (&lower, second_paper)] {
            let sheet = signex_types::schematic::SchematicSheet {
                paper_size: paper.to_string(),
                ..Default::default()
            };
            let mut engine = signex_engine::Engine::new(sheet).expect("engine");
            engine.set_path(Some(path.clone()));
            app.document_state.engines.insert(path.clone(), engine);
            app.document_state.dirty_paths.insert(path.clone());
        }
        (app, upper, lower)
    }

    #[test]
    fn differing_case_variants_are_refused_and_nothing_is_written() {
        // On a folding host the two spellings are ONE file; writing both
        // would let the second silently replace the first's edits.
        let dir = tempfile::tempdir().unwrap();
        let (mut app, upper, lower) = app_with_two_engine_variants(dir.path(), true);
        std::fs::write(&upper, b"on-disk").unwrap();

        let err = app.try_save_dirty_path_with(&upper).unwrap_err();

        let text = format!("{err:#}");
        assert!(
            text.contains("Lib.snxsch") && text.contains("lib.snxsch"),
            "{text}"
        );
        assert_eq!(std::fs::read(&upper).unwrap(), b"on-disk");
        assert!(app.document_state.dirty_paths.contains(&upper));
        assert!(app.document_state.dirty_paths.contains(&lower));
    }

    fn decide_dir_folds(app: &mut Signex, dir: &std::path::Path) {
        use crate::app::state::project_case::CaseProbeOutcome;
        app.document_state
            .dir_case
            .record(dir, CaseProbeOutcome::Decided(true));
    }

    #[test]
    fn identical_variants_in_an_undecided_folder_save_one_and_keep_the_other_dirty() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, upper, lower) = app_with_two_engine_variants(dir.path(), false);

        let err = app.try_save_dirty_path_with(&upper).unwrap_err();

        assert!(format!("{err:#}").starts_with("Save each spelling separately"));
        assert!(!app.document_state.dirty_paths.contains(&upper));
        assert!(app.document_state.dirty_paths.contains(&lower));
        // The retry writes the spelling that is still dirty and succeeds.
        assert!(app.try_save_dirty_path_with(&lower).is_ok());
        assert!(app.document_state.dirty_paths.is_empty());
    }

    #[test]
    fn identical_case_variants_save_once_and_clear_every_marker() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, upper, _lower) = app_with_two_engine_variants(dir.path(), false);

        decide_dir_folds(&mut app, dir.path());

        let result = app.try_save_dirty_path_with(&upper);

        assert!(result.is_ok(), "identical variants must save: {result:?}");
        assert!(app.document_state.dirty_paths.is_empty());
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
                description: None,
                name: "p".into(),
                dir: dir.to_string_lossy().into_owned(),
                schematic_root: None,
                pcb_file: None,
                sheets: Vec::new(),
                variant_definitions: Vec::new(),
                active_variant: None,
                libraries: Vec::new(),
                enable_git: false,
                net_classes: Vec::new(),
                rules: None,
                annotation: None,
                parameters: Default::default(),
                unknown: Default::default(),
            },
            pending_libraries: std::collections::HashMap::new(),
            case_fold: Default::default(),
            case_probed: false,
            case_decided: false,
        });
        let path = dir.join("lib.snxlib").join("footprints").join("g.snxfpt");
        let file =
            signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("fp"));
        let mut editor = crate::app::FootprintDocument::new(path.clone(), file);
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

    #[test]
    fn closing_a_project_forgets_that_its_designator_note_was_shown() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _path) = app_with_unsynced_dirty_footprint(dir.path());
        // A clean workspace, so the close goes straight through.
        app.document_state.footprint_editors.clear();
        app.document_state.dirty_paths.clear();
        let project = &app.document_state.projects[0];
        let (project_id, project_path) = (project.id, project.path.clone());
        app.document_state
            .instance_note_scopes
            .insert(project_path.clone());

        let _ = app.close_project_at_tree_path(&[0]);
        let result = app.resolve_sheet_refs_blocking();
        let _ = app.handle_close_refs_resolved(project_id, result);

        assert!(
            app.document_state
                .projects
                .iter()
                .all(|p| p.id != project_id),
            "the project closed"
        );
        assert!(
            !app.document_state
                .instance_note_scopes
                .contains(&project_path)
        );
    }
}
