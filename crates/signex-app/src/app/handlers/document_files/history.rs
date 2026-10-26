//! History restore + reload-from-disk handlers. Split from `handlers/document_files.rs`.
//!
//! The restore itself (git rewrite plus re-reading the file) runs on a worker
//! (#859, `restore_job`); this file starts it and applies its result.

use iced::Task;

use super::super::super::*;
use crate::app::document_read::{Finished, ReadRequest};
use crate::app::restore_job::{
    EditStamp, RestoreFailure, RestoreJob, RestoreKind, RestoredDocument,
};

impl Signex {
    /// v0.22 Phase 8.5 — Resolve the active tab's path and project, then
    /// restore the user-picked SHA on a worker. Per kind: a schematic ends
    /// dirty, so the next Ctrl+S captures the restored content; a PCB ends
    /// clean and commits "Restore X"; so does a footprint or symbol. A
    /// restore whose file cannot be re-read drops the stale copy so a save
    /// cannot undo it.
    ///
    /// Best-effort: failures log a warning and surface as an error card;
    /// nothing destructive runs (the working tree changes only on a
    /// successful blob read + atomic write). The result is applied by
    /// [`Self::handle_history_restore_finished`].
    pub(crate) fn handle_history_restore_clicked(&mut self, sha: &str) -> Task<Message> {
        self.sync_editor_dirty_paths();
        let Some(active) = self.document_state.tabs.get(self.document_state.active_tab) else {
            return Task::none();
        };
        let full_path = active.path.clone();
        let kind = restore_kind_of(&active.kind);
        if full_path.as_os_str().is_empty() {
            return Task::none();
        }

        // Restoring replaces the in-memory document and its undo stack, so
        // unsaved edits would be gone for good: ask first (#110). The first
        // click arms the row's confirmation, the second one restores.
        if self.primitive_draft_is_dirty(&full_path)
            && self.document_state.history.pending_restore.as_deref() != Some(sha)
        {
            self.document_state.history.pending_restore = Some(sha.to_owned());
            self.document_state.panel_ctx.history = self.document_state.history.clone();
            return Task::none();
        }
        self.document_state.history.pending_restore = None;
        self.document_state.panel_ctx.history = self.document_state.history.clone();

        let Some(project_root) = self.owning_project_root(&full_path) else {
            self.report_restore_failure(format!(
                "{} does not belong to an open project, so it has no history to restore.",
                full_path.display()
            ));
            return Task::none();
        };
        // A save's commit reads the file when its worker starts. Restoring
        // before then would commit the restored bytes under the save's
        // label, so refuse until every commit for this repo has finished
        // (the status bar's "Saving…" pill shows when that is).
        if self
            .document_state
            .inflight_git_commits
            .iter()
            .any(|(root, _)| *root == project_root)
        {
            self.report_restore_failure(format!(
                "{} cannot be restored yet: a save is still being committed to version control. Try again in a moment.",
                full_path.display()
            ));
            return Task::none();
        }
        let Ok(rel_path) = full_path
            .strip_prefix(&project_root)
            .map(|p| p.to_path_buf())
        else {
            return Task::none();
        };
        // A second click while this document's restore is running does
        // nothing; the first one's completion is the only one applied.
        let Some(generation) = self
            .document_state
            .pending_restores
            .begin(full_path.clone())
        else {
            crate::diagnostics::log_warning(format!(
                "[git] restore: A History restore of {} is still running",
                file_label(&full_path)
            ));
            return Task::none();
        };
        let request = ReadRequest {
            path: full_path.clone(),
            generation,
        };
        let edit_stamp = self.edit_stamp(&full_path);
        let job = RestoreJob {
            project_root,
            rel_path,
            full_path,
            sha: sha.to_owned(),
            kind,
            saves: self.document_state.document_saves.barrier(),
        };
        let sha = sha.to_owned();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || job.run())
                    .await
                    .unwrap_or_else(|e| {
                        Err(RestoreFailure::NotRestored(format!(
                            "The restore worker failed: {e}"
                        )))
                    })
            },
            move |result| {
                Message::File(FileMsg::HistoryRestoreFinished {
                    request,
                    sha,
                    edit_stamp,
                    result,
                })
            },
        )
    }

    /// Refuse a save of a document whose History restore is still running:
    /// the write would race the restore's rewrite and re-read, and the
    /// loser's bytes would be installed or committed as the other's.
    pub(crate) fn refuse_save_while_restoring(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if !self.document_state.pending_restores.contains(path) {
            return Ok(());
        }
        anyhow::bail!(
            "A History restore of {} is still running, so the save was refused",
            file_label(path)
        )
    }

    /// Re-read the active tab's file and apply it as a restore would. Tests
    /// only: the app itself reads on a worker.
    #[cfg(test)]
    pub(crate) fn reload_active_tab_from_disk(&mut self) {
        let idx = self.document_state.active_tab;
        let Some(tab) = self.document_state.tabs.get(idx) else {
            return;
        };
        let outcome = crate::app::restore_job::read_back(restore_kind_of(&tab.kind), &tab.path);
        self.apply_reloaded(idx, outcome);
    }

    /// The root directory of the open project that contains `path`.
    fn owning_project_root(&self, path: &std::path::Path) -> Option<std::path::PathBuf> {
        self.document_state
            .projects
            .iter()
            .map(|p| std::path::PathBuf::from(&p.data.dir))
            .find(|dir| path.starts_with(dir))
    }

    /// Apply a finished restore - unless it went stale while it ran (the
    /// project or all documents were closed, or it was superseded). A tab
    /// the user switched away from is activated first: the in-memory copy
    /// must follow the restored file, or the next save would undo it.
    pub(crate) fn handle_history_restore_finished(
        &mut self,
        request: ReadRequest,
        sha: &str,
        edit_stamp: EditStamp,
        result: Result<Box<RestoredDocument>, RestoreFailure>,
    ) -> Task<Message> {
        let path = request.path;
        // Settle lazily-tracked editor edits once for every check below.
        self.sync_editor_dirty_paths();
        let finished = self
            .document_state
            .pending_restores
            .finish_claim(&path, request.generation);
        if finished == Finished::Cancelled {
            log_cancelled_restore_finished(&path, sha, &result);
        }
        let tab_idx = self.document_state.tabs.iter().position(|t| t.path == path);
        let wrote = !matches!(result, Err(RestoreFailure::NotRestored(_)));
        // A cancelled restore may meet a tab reopened for the same file. If
        // that tab holds no edits, the restored file is what it should show:
        // apply the result to it. An edited one keeps its edits.
        let reopened_clean = finished == Finished::Cancelled
            && wrote
            && tab_idx.is_some_and(|i| self.tab_holds_no_edits(i));
        let Some(idx) = tab_idx.filter(|_| finished == Finished::Current || reopened_clean) else {
            // Cancelled (tab, project or all documents closed): the file may
            // still have been rewritten, so say so.
            tracing::debug!(path = %path.display(), "stale history restore");
            // Edits survive a closed tab as a parked draft, so unsaved work
            // is judged by the dirty marker, not only by an open tab.
            let has_edits =
                wrote && (tab_idx.is_some() || self.document_state.dirty_paths.contains(&path));
            self.report_restore_without_tab(&path, sha, &result, has_edits);
            return Task::none();
        };
        if finished == Finished::Current && wrote && self.edit_stamp(&path) != edit_stamp {
            self.report_restore_over_edits(&path, sha);
            return Task::none();
        }
        // `apply_reloaded` installs by tab index and path, so the user's
        // active tab, tool and half-finished input stay as they are.
        match result {
            Err(RestoreFailure::NotRestored(message)) => self.report_restore_failure(message),
            Err(failure) => {
                self.note_restored(&path, sha);
                self.apply_reloaded(idx, Err(failure));
                self.refresh_panel_ctx();
            }
            Ok(document) => {
                self.note_restored(&path, sha);
                self.apply_reloaded(idx, Ok(*document));
                self.refresh_panel_ctx();
            }
        }
        // The file was rewritten and the document replaced, so a recovery
        // copy this run wrote holds the edits the restore discarded. (An
        // earlier run's copy, deferred or not, is left for the next open.)
        // The replacement engine has its own revision, so the next tick
        // writes the restored document without waiting on the old stamp.
        if wrote {
            return self.autosave_after_disk_write(&path);
        }
        Task::none()
    }

    /// Whether the tab at `idx` has nothing unsaved.
    fn tab_holds_no_edits(&self, idx: usize) -> bool {
        let Some(tab) = self.document_state.tabs.get(idx) else {
            return false;
        };
        !tab.dirty && !self.document_state.dirty_paths.contains(&tab.path)
    }

    /// The file is back on disk: mark it dirty (the reload clears that where
    /// the loaded document matches disk) and log it.
    fn note_restored(&mut self, path: &std::path::Path, sha: &str) {
        self.document_state.dirty_paths.insert(path.to_path_buf());
        let shown = self
            .owning_project_root(path)
            .and_then(|root| path.strip_prefix(root).ok().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| path.to_path_buf());
        crate::diagnostics::log_info(format!("[git] restored {} to {sha}", shown.display()));
    }

    /// The tab was edited while the restore ran. The restored file is on
    /// disk; the unsaved edits stay in the tab rather than being replaced, and
    /// nothing is committed. The user chooses by saving or closing.
    fn report_restore_over_edits(&mut self, path: &std::path::Path, sha: &str) {
        self.document_state.dirty_paths.insert(path.to_path_buf());
        self.report_restore_failure(format!(
            "{} was edited while its restore to {sha} ran, so the unsaved edits were kept and \
             the restored file was not loaded or committed. The file on disk is the restored \
             revision: saving will overwrite it, closing without saving keeps it.",
            path.display()
        ));
    }

    /// The restore's result was not applied: its tab (or project) closed
    /// while it ran, or the document has unsaved edits (`has_edits`: a
    /// reopened tab, or a draft parked when its tab closed). Nothing in memory follows the file, so say what the file
    /// is and what a save would do.
    fn report_restore_without_tab(
        &self,
        path: &std::path::Path,
        sha: &str,
        result: &Result<Box<RestoredDocument>, RestoreFailure>,
        has_edits: bool,
    ) {
        let name = file_label(path);
        let text = match result {
            Err(RestoreFailure::NotRestored(message)) => message.clone(),
            _ if has_edits => format!(
                "{name} has unsaved edits but its file is now at {sha}; saving overwrites that restore"
            ),
            _ => format!(
                "{} was restored to {sha} on disk, but its tab or project was closed before the restore \
                 finished. The restored file was not committed to version control.",
                path.display()
            ),
        };
        crate::diagnostics::log_warning(format!("[git] restore: {text}"));
    }

    /// A History restore did not happen: log it and raise the error card
    /// so the click is not silently ignored (#110).
    fn report_restore_failure(&mut self, message: String) {
        crate::diagnostics::log_warning(format!("[git] restore: {message}"));
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::restore(message));
    }

    /// Install the restored file into the tab at `idx` (the active tab), or
    /// drop the stale copy when it could not be loaded.
    ///
    /// Per-tab-kind:
    /// - **Schematic**: replace the engine's sheet without resetting the
    ///   camera (the user's pan/zoom is preserved across a restore).
    /// - **PCB**: replace the tab's cached board, refresh the renderer
    ///   snapshot, and commit the on-disk bytes as they are.
    /// - **FootprintEditor / SymbolEditor**: replace the editor state and
    ///   clear the canvas cache.
    /// - **LibraryBrowser / ComponentEditor**: reload from the mounted
    ///   library adapter (still on the UI thread; it needs the adapter).
    pub(crate) fn apply_reloaded(
        &mut self,
        idx: usize,
        outcome: Result<RestoredDocument, RestoreFailure>,
    ) {
        let Some(tab) = self.document_state.tabs.get(idx) else {
            return;
        };
        let path = tab.path.clone();
        let kind = tab.kind.clone();
        match outcome {
            Err(RestoreFailure::NotReloaded { why, unreadable }) => match kind {
                crate::app::TabKind::Schematic => {
                    self.fail_schematic_reload(&path, idx, &why, unreadable)
                }
                crate::app::TabKind::Pcb => self.fail_pcb_reload(&path, idx, &why, unreadable),
                _ => self.fail_primitive_reload(&path, idx, &why, unreadable),
            },
            Err(RestoreFailure::NotRestored(message)) => self.report_restore_failure(message),
            Ok(RestoredDocument::Schematic(sheet)) => {
                crate::app::load_gateway::report_unrecognized_sheet_size(&path, &sheet.paper_size);
                if idx == self.document_state.active_tab {
                    self.discard_state_of_replaced_engine();
                    self.apply_loaded_schematic(Some(*sheet), true, false, false);
                    // Undocked windows on this sheet were redrawn by that;
                    // their selections point into the old engine too.
                    self.refresh_slots_from_engine(
                        &path,
                        false,
                        crate::schematic_runtime::RenderInvalidation::FULL,
                        true,
                    );
                } else {
                    self.install_schematic_at(&path, *sheet);
                }
            }
            Ok(RestoredDocument::Pcb(board)) => {
                crate::app::load_gateway::report_unusable_zone_holes(&path, &board);
                self.document_state.set_pcb_board(idx, *board);
                if idx == self.document_state.active_tab {
                    // Refresh canvas; preserve camera (don't fit-to-board).
                    self.apply_loaded_pcb_document(false, false);
                }
                // The tab now matches disk. Leaving it dirty would let Ctrl+S
                // re-encode the board and drop the restored file's comments
                // and formatting, so commit the on-disk bytes as they are.
                self.set_reloaded_dirty(&path, idx, false);
                self.commit_restore(&path);
            }
            Ok(RestoredDocument::Footprint(file)) => {
                let snap_disabled = !self.ui_state.snap_enabled;
                let state = crate::app::FootprintEditorState::reloaded(path.clone(), *file)
                    .with_global_snap_disabled(snap_disabled);
                // A load repair makes the reloaded editor differ from disk, so
                // it is dirty; otherwise it matches disk and the stale marker
                // must go. Either way the tab and `dirty_paths` follow the
                // editor, or quit would not offer to save the repair (or would
                // ask about a file that matches disk).
                let dirty = state.dirty;
                self.document_state
                    .footprint_editors
                    .insert(path.clone(), state);
                self.set_reloaded_dirty(&path, idx, dirty);
                if let Some(editor) = self.document_state.footprint_editors.get_mut(&path) {
                    editor.canvas_cache.clear();
                }
                self.commit_restore(&path);
            }
            Ok(RestoredDocument::Symbol(file)) => {
                let state = crate::app::SymbolEditorState::new(path.clone(), *file);
                self.document_state
                    .symbol_editors
                    .insert(path.clone(), state);
                // The reloaded editor is clean, so the stale dirty marker must
                // go or quit asks about a file that matches disk.
                self.set_reloaded_dirty(&path, idx, false);
                self.commit_restore(&path);
            }
            Ok(RestoredDocument::Library) => self.reload_library_tab(&kind),
        }
    }

    /// v0.23 — reload-after-restore for `.snxlib` tabs. Re-opens a fresh
    /// `LocalGitAdapter` against the on-disk root (the mounted adapter's git2
    /// caches are stale after the restore) and re-runs `reload_tables` so the
    /// table view picks up the restored TSVs. Bails early if no library is
    /// open at this path; failures log and leave the cache as it was.
    fn reload_library_tab(&mut self, kind: &crate::app::TabKind) {
        let lib_path = match kind {
            crate::app::TabKind::LibraryBrowser(p) => p.clone(),
            crate::app::TabKind::ComponentEditor(c) => c.library_path.clone(),
            _ => return,
        };
        if self.library.library_at(&lib_path).is_none() {
            crate::diagnostics::log_warning(format!(
                "[reload] LibraryBrowser tab {} has no open library to refresh",
                lib_path.display()
            ));
            return;
        }
        if let Some(open_lib) = self.library.library_at_mut(&lib_path) {
            match signex_library::LocalGitAdapter::open(&lib_path) {
                Ok(fresh_adapter) => {
                    if let Err(e) = open_lib.reload_tables(&fresh_adapter) {
                        crate::diagnostics::log_warning(format!(
                            "[reload] reload_tables({}) failed: {e}",
                            lib_path.display()
                        ));
                    } else {
                        self.library.sweep_dangling_refs();
                        crate::diagnostics::log_info(format!(
                            "[reload] LibraryBrowser tab {} refreshed",
                            lib_path.display()
                        ));
                    }
                }
                Err(e) => {
                    crate::diagnostics::log_warning(format!(
                        "[reload] LocalGitAdapter::open({}) failed: {e}",
                        lib_path.display()
                    ));
                }
            }
        }
        self.refresh_panel_ctx();
    }

    /// Queue the project git commit for the restored bytes already on disk,
    /// so a clean tab does not leave them uncommitted. An enqueue while a
    /// commit of this file is in flight is re-queued (see
    /// `commit_save_to_project_git`).
    fn commit_restore(&mut self, path: &std::path::Path) {
        let label = path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        self.commit_save_to_project_git(path, &format!("Restore {label}"));
    }

    /// A restored footprint/symbol could not be loaded into its editor. The
    /// editor in memory is the pre-restore one, so drop it (a save then
    /// finds nothing to write and cannot overwrite the restored bytes), clear
    /// the dirty marker set for the restore, and raise the error card.
    /// `unreadable` is true for an IO error, where reopening can help.
    fn fail_primitive_reload(
        &mut self,
        path: &std::path::Path,
        active_idx: usize,
        why: &str,
        unreadable: bool,
    ) {
        self.document_state.footprint_editors.remove(path);
        self.document_state.symbol_editors.remove(path);
        self.set_reloaded_dirty(path, active_idx, false);
        self.report_restore_failure(restore_reload_card(path, why, unreadable));
    }

    /// Same as [`Self::fail_primitive_reload`] for a schematic: the engine
    /// still holds the pre-restore sheet and is what Ctrl+S serialises, so
    /// drop it. Saving then has no open schematic to write.
    fn fail_schematic_reload(
        &mut self,
        path: &std::path::Path,
        active_idx: usize,
        why: &str,
        unreadable: bool,
    ) {
        if active_idx == self.document_state.active_tab {
            self.discard_state_of_replaced_engine();
        }
        self.document_state.engines.remove(path);
        self.set_reloaded_dirty(path, active_idx, false);
        self.report_restore_failure(restore_reload_card(path, why, unreadable));
    }

    /// Same for a PCB: the tab's cached board is the pre-restore one and
    /// is what Ctrl+S would encode, so drop it (a save then refuses) and
    /// stop the canvas drawing it.
    fn fail_pcb_reload(
        &mut self,
        path: &std::path::Path,
        active_idx: usize,
        why: &str,
        unreadable: bool,
    ) {
        self.document_state.drop_pcb_board(active_idx);
        self.set_reloaded_dirty(path, active_idx, false);
        if active_idx == self.document_state.active_tab {
            self.apply_loaded_empty_document(false);
        }
        self.report_restore_failure(restore_reload_card(path, why, unreadable));
    }

    /// Records whether a document just reloaded from disk is dirty, in
    /// both places that track it. Every reload arm goes through here so
    /// a stale marker cannot outlive the reload (#780).
    fn set_reloaded_dirty(&mut self, path: &std::path::Path, active_idx: usize, dirty: bool) {
        if dirty {
            self.document_state.dirty_paths.insert(path.to_path_buf());
        } else {
            self.document_state.dirty_paths.remove(path);
        }
        if let Some(tab) = self.document_state.tabs.get_mut(active_idx) {
            tab.dirty = dirty;
        }
    }
}

/// Error-card text for a restore whose file could not be loaded back.
/// Reopening only helps when the read failed; a parse or empty-content
/// failure would read the same bytes again.
fn restore_reload_card(path: &std::path::Path, why: &str, unreadable: bool) -> String {
    let advice = if unreadable {
        "Close and reopen the tab once the file can be read."
    } else {
        "Reopening would read the same bytes again; restore a different revision instead."
    };
    format!(
        "{} was restored but {why}. The restored file was not committed to version control. {advice}",
        path.display()
    )
}

/// The file's name for a Messages line, or the whole path without one.
fn file_label(path: &std::path::Path) -> String {
    path.file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// A cancelled restore's worker has reported, so saves of the file are
/// allowed again. Short: the Messages panel keeps only the first 157 bytes.
fn log_cancelled_restore_finished(
    path: &std::path::Path,
    sha: &str,
    result: &Result<Box<RestoredDocument>, RestoreFailure>,
) {
    let name = file_label(path);
    let outcome = match result {
        Err(RestoreFailure::NotRestored(_)) => "without writing",
        _ => "after writing",
    };
    crate::diagnostics::log_warning(format!(
        "[git] restore: A cancelled restore of {name} to {sha} finished {outcome}; saves are allowed again"
    ));
}

fn restore_kind_of(kind: &crate::app::TabKind) -> RestoreKind {
    match kind {
        crate::app::TabKind::Schematic => RestoreKind::Schematic,
        crate::app::TabKind::Pcb => RestoreKind::Pcb,
        crate::app::TabKind::FootprintEditor(_) => RestoreKind::Footprint,
        crate::app::TabKind::SymbolEditor(_) => RestoreKind::Symbol,
        crate::app::TabKind::LibraryBrowser(_) | crate::app::TabKind::ComponentEditor(_) => {
            RestoreKind::Library
        }
    }
}
