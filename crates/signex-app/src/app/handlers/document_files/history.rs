//! History restore + reload-from-disk handlers. Split from `handlers/document_files.rs`.

use super::super::super::*;

impl Signex {
    /// v0.22 Phase 8.5 — Resolve the active tab's path and project,
    /// then call `LocalGitProjectAdapter::restore_at` with the user-
    /// picked SHA. Per kind: a schematic ends dirty, so the next Ctrl+S
    /// captures the restored content; a PCB ends clean and commits
    /// "Restore X"; so does a footprint or symbol. A restore whose file
    /// cannot be re-read drops the stale copy so a save cannot undo it.
    ///
    /// Best-effort: failures log a warning and surface as a status
    /// message; nothing destructive runs (the working tree changes
    /// only on a successful blob read + atomic write).
    pub(crate) fn handle_history_restore_clicked(&mut self, sha: &str) {
        self.sync_editor_dirty_paths();
        let active = match self.document_state.tabs.get(self.document_state.active_tab) {
            Some(t) => t,
            None => return,
        };
        let full_path = active.path.clone();
        if full_path.as_os_str().is_empty() {
            return;
        }

        // Restoring replaces the in-memory document and its undo stack, so
        // unsaved edits would be gone for good: ask first (#110). The first
        // click arms the row's confirmation, the second one restores.
        if self.primitive_draft_is_dirty(&full_path)
            && self.document_state.history.pending_restore.as_deref() != Some(sha)
        {
            self.document_state.history.pending_restore = Some(sha.to_owned());
            self.document_state.panel_ctx.history = self.document_state.history.clone();
            return;
        }
        self.document_state.history.pending_restore = None;
        self.document_state.panel_ctx.history = self.document_state.history.clone();

        // Find the owning project by directory prefix.
        let owning = self.document_state.projects.iter().find(|p| {
            let dir = std::path::Path::new(&p.data.dir);
            full_path.starts_with(dir)
        });
        let Some(project) = owning else {
            self.report_restore_failure(format!(
                "{} does not belong to an open project, so it has no history to restore.",
                full_path.display()
            ));
            return;
        };
        let project_root = std::path::PathBuf::from(&project.data.dir);
        let rel_path = match full_path.strip_prefix(&project_root) {
            Ok(p) => p.to_path_buf(),
            Err(_) => return,
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
            return;
        }

        let adapter =
            match signex_library::adapters::local_git_project::LocalGitProjectAdapter::open_or_init(
                project_root.clone(),
            ) {
                Ok(a) => a,
                Err(e) => {
                    self.report_restore_failure(format!(
                        "Could not open the version-control repository at {}: {e}",
                        project_root.display()
                    ));
                    return;
                }
            };
        // A write still running for this file would land after the restore
        // and put the pre-restore content back on disk; let it finish first
        // (same bounded wait as Save All).
        self.document_state.document_saves.barrier().wait();
        match adapter.restore_at_from_sha(&rel_path, sha) {
            Ok(()) => {
                self.document_state.dirty_paths.insert(full_path.clone());
                crate::diagnostics::log_info(format!(
                    "[git] restored {} to {}",
                    rel_path.display(),
                    sha
                ));
                // v0.22 — live-reload the active tab from disk so the
                // user sees the restored content immediately. Without
                // this, the working tree changed but the in-memory
                // engine still holds the pre-restore state until the
                // user re-opens.
                self.reload_active_tab_from_disk();
                self.refresh_panel_ctx();
            }
            Err(e) => {
                self.report_restore_failure(format!(
                    "Could not restore {} to {sha}: {e}",
                    rel_path.display()
                ));
            }
        }
    }

    /// A History restore did not happen: log it and raise the error card
    /// so the click is not silently ignored (#110).
    fn report_restore_failure(&mut self, message: String) {
        crate::diagnostics::log_warning(format!("[git] restore: {message}"));
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::restore(message));
    }

    /// v0.22 — Re-parse the active tab's on-disk file and replace
    /// the in-memory engine/editor state with the fresh content.
    /// Used after a `restore_at` to make the rewind visible without
    /// requiring the user to close and reopen the tab.
    ///
    /// Per-tab-kind dispatch:
    /// - **Schematic**: re-parse `.snxsch` via `SnxSchematic::parse`,
    ///   replace the engine via `sync_engine_from_schematic`, refresh
    ///   the canvas.
    /// - **PCB**: re-parse `.snxpcb` via `SnxPcb::parse`, replace
    ///   the tab's cached_document, refresh the renderer snapshot.
    /// - **FootprintEditor**: re-parse the `.snxfpt` JSON, replace
    ///   the entry in `document_state.footprint_editors`, clear the
    ///   canvas cache.
    /// - **SymbolEditor**: re-parse the `.snxsym` JSON, replace the
    ///   entry in `document_state.symbol_editors`, clear the canvas
    ///   cache.
    /// - **LibraryBrowser / ComponentEditor**: deferred — these tabs
    ///   read from the library adapter which has its own refresh
    ///   path; restore-to-historical-version on these would need a
    ///   library-side reload.
    pub(crate) fn reload_active_tab_from_disk(&mut self) {
        let active_idx = self.document_state.active_tab;
        let Some(tab) = self.document_state.tabs.get(active_idx) else {
            return;
        };
        let path = tab.path.clone();
        let kind = tab.kind.clone();

        match kind {
            crate::app::TabKind::Schematic => {
                let text = match std::fs::read_to_string(&path) {
                    Ok(t) => t,
                    Err(e) => {
                        self.fail_schematic_reload(
                            &path,
                            active_idx,
                            &format!("could not be read: {e}"),
                            true,
                        );
                        return;
                    }
                };
                let parsed = match signex_types::format::SnxSchematic::parse(&text) {
                    Ok(p) => p.sheet,
                    Err(e) => {
                        self.fail_schematic_reload(
                            &path,
                            active_idx,
                            &format!("could not be parsed: {e}"),
                            false,
                        );
                        return;
                    }
                };
                // Replace the engine's schematic without resetting
                // the camera (don't fit-to-paper — the user's pan/zoom
                // is preserved across a restore).
                crate::app::load_gateway::report_unrecognized_sheet_size(&path, &parsed.paper_size);
                self.apply_loaded_schematic(Some(parsed), true, false, false);
            }
            crate::app::TabKind::Pcb => {
                let board = match std::fs::read_to_string(&path)
                    .map_err(|e| (format!("could not be read: {e}"), true))
                    .and_then(|text| {
                        signex_types::format::SnxPcb::parse(&text)
                            .map_err(|e| (format!("could not be parsed: {e}"), false))
                    }) {
                    Ok(p) => p.board,
                    Err((why, unreadable)) => {
                        self.fail_pcb_reload(&path, active_idx, &why, unreadable);
                        return;
                    }
                };
                crate::app::load_gateway::report_unusable_zone_holes(&path, &board);
                self.document_state.set_pcb_board(active_idx, board);
                // Refresh canvas; preserve camera (don't fit-to-board).
                self.apply_loaded_pcb_document(false, false);
                // The tab now matches disk. Leaving it dirty would let Ctrl+S
                // re-encode the board and drop the restored file's comments
                // and formatting, so commit the on-disk bytes as they are.
                self.set_reloaded_dirty(&path, active_idx, false);
                self.commit_restore(&path);
            }
            crate::app::TabKind::FootprintEditor(p) => {
                let bytes = match std::fs::read_to_string(&p) {
                    Ok(s) => s,
                    Err(e) => {
                        self.fail_primitive_reload(
                            &p,
                            active_idx,
                            &format!("could not be read: {e}"),
                            true,
                        );
                        return;
                    }
                };
                match signex_library::FootprintFile::from_toml_str(&bytes) {
                    Ok(file) if !file.footprints.is_empty() => {
                        let snap_disabled = !self.ui_state.snap_enabled;
                        let state = crate::app::FootprintEditorState::reloaded(p.clone(), file)
                            .with_global_snap_disabled(snap_disabled);
                        // A load repair makes the reloaded editor differ from
                        // disk, so it is dirty; otherwise it matches disk and
                        // the stale marker must go. Either way the tab and
                        // `dirty_paths` follow the editor, or quit would not
                        // offer to save the repair (or would ask about a
                        // file that matches disk).
                        let dirty = state.dirty;
                        self.document_state
                            .footprint_editors
                            .insert(p.clone(), state);
                        self.set_reloaded_dirty(&p, active_idx, dirty);
                        if let Some(editor) = self.document_state.footprint_editors.get_mut(&p) {
                            editor.canvas_cache.clear();
                        }
                        self.commit_restore(&p);
                    }
                    Ok(_) => {
                        self.fail_primitive_reload(
                            &p,
                            active_idx,
                            "contains zero footprints",
                            false,
                        );
                    }
                    Err(e) => {
                        self.fail_primitive_reload(
                            &p,
                            active_idx,
                            &format!("could not be parsed: {e}"),
                            false,
                        );
                    }
                }
            }
            crate::app::TabKind::SymbolEditor(p) => {
                let bytes = match std::fs::read(&p) {
                    Ok(b) => b,
                    Err(e) => {
                        self.fail_primitive_reload(
                            &p,
                            active_idx,
                            &format!("could not be read: {e}"),
                            true,
                        );
                        return;
                    }
                };
                match signex_library::SymbolFile::from_bytes(&bytes) {
                    Ok(file) if !file.symbols.is_empty() => {
                        let state = crate::app::SymbolEditorState::new(p.clone(), file);
                        self.document_state.symbol_editors.insert(p.clone(), state);
                        // The reloaded editor is clean, so the stale
                        // dirty marker must go or quit asks about a
                        // file that matches disk.
                        self.set_reloaded_dirty(&p, active_idx, false);
                        self.commit_restore(&p);
                    }
                    Ok(_) => {
                        self.fail_primitive_reload(&p, active_idx, "contains zero symbols", false);
                    }
                    Err(e) => {
                        self.fail_primitive_reload(
                            &p,
                            active_idx,
                            &format!("could not be parsed: {e}"),
                            false,
                        );
                    }
                }
            }
            crate::app::TabKind::LibraryBrowser(_) | crate::app::TabKind::ComponentEditor(_) => {
                let lib_path = match &kind {
                    crate::app::TabKind::LibraryBrowser(p) => p.clone(),
                    crate::app::TabKind::ComponentEditor(c) => c.library_path.clone(),
                    _ => unreachable!(),
                };
                // v0.23 — reload-after-restore for `.snxlib` tabs.
                // Re-opens a fresh `LocalGitAdapter` against the
                // on-disk root (the mounted adapter's git2 caches
                // are stale after `restore_at`) and re-runs
                // `reload_tables` so the table view picks up the
                // restored TSVs. Bails early if no library is
                // currently open at this path; failures log + drop
                // into an empty cache.
                if self.library.library_at(&lib_path).is_none() {
                    crate::diagnostics::log_warning(format!(
                        "[reload] LibraryBrowser tab {} has no open library to refresh",
                        lib_path.display()
                    ));
                    return;
                }
                if let Some(open_lib) = self.library.library_at_mut(&lib_path) {
                    // Re-open the on-disk adapter rather than reusing
                    // the mounted one — `restore_at` rewrote the
                    // working tree, and the adapter's internal
                    // caches (e.g. git2 index) are stale.
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
                // Refresh the panel so the Library Browser tab picks
                // up the new table set on the next render.
                self.refresh_panel_ctx();
            }
        }
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
        self.apply_loaded_empty_document(false);
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
