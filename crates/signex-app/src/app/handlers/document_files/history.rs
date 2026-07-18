//! History restore + reload-from-disk handlers. Split from `handlers/document_files.rs`.



use super::super::super::*;

impl Signex {
    /// v0.22 Phase 8.5 — Resolve the active tab's path and project,
    /// then call `LocalGitProjectAdapter::restore_at` with the user-
    /// picked SHA. Marks the file dirty so the next Ctrl+S captures
    /// the restored content.
    ///
    /// Best-effort: failures log a warning and surface as a status
    /// message; nothing destructive runs (the working tree changes
    /// only on a successful blob read + atomic write).
    pub(crate) fn handle_history_restore_clicked(&mut self, sha: &str) {
        let active = match self.document_state.tabs.get(self.document_state.active_tab) {
            Some(t) => t,
            None => return,
        };
        let full_path = active.path.clone();
        if full_path.as_os_str().is_empty() {
            return;
        }

        // Find the owning project by directory prefix.
        let owning = self.document_state.projects.iter().find(|p| {
            let dir = std::path::Path::new(&p.data.dir);
            full_path.starts_with(dir)
        });
        let Some(project) = owning else {
            crate::diagnostics::log_warning(format!(
                "[git] restore: no owning project for {}",
                full_path.display()
            ));
            return;
        };
        let project_root = std::path::PathBuf::from(&project.data.dir);
        let rel_path = match full_path.strip_prefix(&project_root) {
            Ok(p) => p.to_path_buf(),
            Err(_) => return,
        };

        let adapter =
            match signex_library::adapters::local_git_project::LocalGitProjectAdapter::open_or_init(
                project_root.clone(),
            ) {
                Ok(a) => a,
                Err(e) => {
                    crate::diagnostics::log_warning(format!(
                        "[git] restore: open_or_init({}) failed: {e}",
                        project_root.display()
                    ));
                    return;
                }
            };
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
                crate::diagnostics::log_warning(format!(
                    "[git] restore_at({}, {}) failed: {e}",
                    rel_path.display(),
                    sha
                ));
            }
        }
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
                        crate::diagnostics::log_warning(format!(
                            "[reload] read {} failed: {e}",
                            path.display()
                        ));
                        return;
                    }
                };
                let parsed = match signex_types::format::SnxSchematic::parse(&text) {
                    Ok(p) => p.sheet,
                    Err(e) => {
                        crate::diagnostics::log_warning(format!(
                            "[reload] parse {} failed: {e}",
                            path.display()
                        ));
                        return;
                    }
                };
                // Replace the engine's schematic without resetting
                // the camera (don't fit-to-paper — the user's pan/zoom
                // is preserved across a restore).
                self.apply_loaded_schematic(Some(parsed), true, false, false, false);
            }
            crate::app::TabKind::Pcb => {
                let text = match std::fs::read_to_string(&path) {
                    Ok(t) => t,
                    Err(e) => {
                        crate::diagnostics::log_warning(format!(
                            "[reload] read {} failed: {e}",
                            path.display()
                        ));
                        return;
                    }
                };
                let board = match signex_types::format::SnxPcb::parse(&text) {
                    Ok(p) => p.board,
                    Err(e) => {
                        crate::diagnostics::log_warning(format!(
                            "[reload] parse {} failed: {e}",
                            path.display()
                        ));
                        return;
                    }
                };
                if let Some(t) = self.document_state.tabs.get_mut(active_idx) {
                    t.cached_document = Some(crate::app::TabDocument::Pcb(board));
                }
                // Refresh canvas; preserve camera (don't fit-to-board).
                self.apply_loaded_pcb_document(false, false);
            }
            crate::app::TabKind::FootprintEditor(p) => {
                let bytes = match std::fs::read_to_string(&p) {
                    Ok(s) => s,
                    Err(e) => {
                        crate::diagnostics::log_warning(format!(
                            "[reload] read {} failed: {e}",
                            p.display()
                        ));
                        return;
                    }
                };
                match signex_library::FootprintFile::from_toml_str(&bytes) {
                    Ok(file) if !file.footprints.is_empty() => {
                        let snap_disabled = !self.ui_state.snap_enabled;
                        let state = crate::app::FootprintEditorState::new(p.clone(), file)
                            .with_global_snap_disabled(snap_disabled);
                        self.document_state
                            .footprint_editors
                            .insert(p.clone(), state);
                        if let Some(editor) = self.document_state.footprint_editors.get_mut(&p) {
                            editor.canvas_cache.clear();
                        }
                    }
                    Ok(_) => {
                        crate::diagnostics::log_warning(format!(
                            "[reload] {} contains zero footprints",
                            p.display()
                        ));
                    }
                    Err(e) => {
                        crate::diagnostics::log_warning(format!(
                            "[reload] parse {} failed: {e}",
                            p.display()
                        ));
                    }
                }
            }
            crate::app::TabKind::SymbolEditor(p) => {
                let bytes = match std::fs::read(&p) {
                    Ok(b) => b,
                    Err(e) => {
                        crate::diagnostics::log_warning(format!(
                            "[reload] read {} failed: {e}",
                            p.display()
                        ));
                        return;
                    }
                };
                match signex_library::SymbolFile::from_bytes(&bytes) {
                    Ok(file) if !file.symbols.is_empty() => {
                        let state = crate::app::SymbolEditorState::new(p.clone(), file);
                        self.document_state.symbol_editors.insert(p.clone(), state);
                    }
                    Ok(_) => {
                        crate::diagnostics::log_warning(format!(
                            "[reload] {} contains zero symbols",
                            p.display()
                        ));
                    }
                    Err(e) => {
                        crate::diagnostics::log_warning(format!(
                            "[reload] parse {} failed: {e}",
                            p.display()
                        ));
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

}
