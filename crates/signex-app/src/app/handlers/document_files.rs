use std::path::PathBuf;

use anyhow::{Context, Result};

use super::super::*;

impl Signex {
    pub(crate) fn handle_document_file_opened(&mut self, path: Option<PathBuf>) {
        let Some(path) = path else {
            return;
        };

        self.interaction_state.editing_text = None;
        self.interaction_state.context_menu = None;

        if let Err(error) = self.open_document_path(path) {
            crate::diagnostics::log_error("Failed to open document path", &error);
        }
    }

    pub(crate) fn handle_new_project_file(&mut self, path: Option<PathBuf>) {
        let Some(path) = path else { return };

        self.interaction_state.editing_text = None;
        self.interaction_state.context_menu = None;

        if let Err(error) = self.create_new_project(&path) {
            crate::diagnostics::log_error("Failed to create new project", &error);
        }
    }

    /// Create a brand-new `.snxprj` at `path` plus a blank companion
    /// `<stem>.snxsch` in the same directory, then load the project and
    /// open the schematic as a tab. The `.snxprj` is written empty —
    /// `parse_project` is directory-driven and ignores file content.
    fn create_new_project(&mut self, project_path: &std::path::Path) -> Result<()> {
        if project_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("snxprj"))
            != Some(true)
        {
            anyhow::bail!(
                "new project path must end in .snxprj (got {})",
                project_path.display()
            );
        }
        let dir = project_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("project path has no parent directory"))?;
        let stem = project_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("project path has no file stem"))?
            .to_string();

        std::fs::create_dir_all(dir)
            .with_context(|| format!("create project directory {}", dir.display()))?;

        std::fs::write(project_path, b"")
            .with_context(|| format!("write project file {}", project_path.display()))?;

        let sch_path = dir.join(format!("{stem}.snxsch"));
        if !sch_path.exists() {
            let sheet = blank_schematic_sheet();
            let serialised = signex_types::format::SnxSchematic::new(sheet)
                .write_string()
                .context("serialise blank schematic")?;
            std::fs::write(&sch_path, serialised.as_bytes())
                .with_context(|| format!("write blank schematic {}", sch_path.display()))?;
        }

        self.open_document_path(project_path.to_path_buf())?;
        if sch_path.exists() {
            // Best-effort: surface the schematic tab so the new project
            // lands the user on a drawable canvas. Errors here just
            // leave them on the Welcome view.
            if let Err(error) = self.open_document_path(sch_path) {
                crate::diagnostics::log_error("Open new project schematic", &error);
            }
        }
        self.refresh_panel_ctx();
        Ok(())
    }

    pub(crate) fn handle_active_document_save_requested(&mut self) -> iced::Task<Message> {
        match self.save_active_document() {
            Ok(task) => task,
            Err(error) => {
                crate::diagnostics::log_error("Failed to save active document", &error);
                iced::Task::none()
            }
        }
    }

    pub(crate) fn handle_active_document_save_as_requested(&mut self, path: PathBuf) {
        if let Err(error) = self.save_active_document_as(path) {
            crate::diagnostics::log_error("Failed to save active document as", &error);
        }
    }

    fn open_document_path(&mut self, path: PathBuf) -> Result<()> {
        let ext = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("");
        match ext {
            "standard_pro" | "snxprj" => self.open_project_file(path)?,
            "standard_sch" | "snxsch" => self.open_schematic_file(path)?,
            "standard_pcb" | "snxpcb" => self.open_pcb_file(path)?,
            "snxsym" | "snxfpt" => {
                let _ = self.handle_open_primitive(path);
            }
            // `.snxlib/` is a directory package — open it as a Library
            // Browser tab in the main canvas area. The handler mounts
            // the library if not already mounted and pushes the tab
            // (or activates it if a tab for the same library is
            // already open).
            "snxlib" => {
                let _ = self.handle_open_library_browser(path);
            }
            _ => anyhow::bail!("unsupported file type: .{ext}"),
        }

        Ok(())
    }

    fn open_project_file(&mut self, path: PathBuf) -> Result<()> {
        self.load_or_activate_project(&path)?;
        self.refresh_panel_ctx();
        Ok(())
    }

    /// Append a `LoadedProject` for `project_path` to the workspace if
    /// it isn't already loaded, then make it active. De-dupes by path
    /// so re-opening the same project just switches activity. Used by
    /// both `open_project_file` (direct .standard_pro open) and the
    /// companion-project path inside `open_schematic_file` /
    /// `open_pcb_file`. Returns the resolved `ProjectId`.
    fn load_or_activate_project(
        &mut self,
        project_path: &std::path::Path,
    ) -> Result<crate::app::state::ProjectId> {
        if let Some(existing) = self
            .document_state
            .projects
            .iter()
            .find(|p| p.path == project_path)
        {
            let id = existing.id;
            self.document_state.active_project = Some(id);
            return Ok(id);
        }
        let data = signex_types::project::parse_project(project_path)
            .with_context(|| format!("parse project {}", project_path.display()))?;
        let id = self.document_state.mint_project_id();
        // Auto-mount every library referenced by `Project::libraries`
        // so the project tree picks them up before the panel rebuild
        // fires. Errors are logged inside `auto_mount_project_libraries`
        // and never bubble: a missing library shouldn't block the
        // project from loading.
        let mounted =
            crate::library::commands::auto_mount_project_libraries(&mut self.library, &data);
        if mounted > 0 {
            tracing::info!(
                target: "signex::library",
                project = %project_path.display(),
                mounted,
                "auto-mounted project libraries"
            );
        }
        self.document_state
            .projects
            .push(super::super::state::LoadedProject {
                id,
                path: project_path.to_path_buf(),
                data,
                pending_libraries: std::collections::HashMap::new(),
            });
        self.document_state.active_project = Some(id);
        Ok(id)
    }

    fn open_schematic_file(&mut self, path: PathBuf) -> Result<()> {
        // Try to load the companion project so the schematic tab gets
        // a `project_id` via `project_for_path`. Best-effort: a missing
        // or unparseable `.snxprj` doesn't block opening the loose
        // schematic.
        if let Some(dir) = path.parent() {
            let stem = path
                .file_stem()
                .and_then(|segment| segment.to_str())
                .unwrap_or("");
            let companion = dir.join(format!("{stem}.snxprj"));
            if companion.exists()
                && let Err(error) = self.load_or_activate_project(&companion)
            {
                crate::diagnostics::log_error("Failed to parse companion project", &error);
            }
        }
        let title = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "Schematic".to_string());
        // Parked-engine restore — same Altium-parity rule as the
        // project-tree open path. Reparsing from disk would discard
        // edits the user made before closing the tab.
        if self.document_state.engines.contains_key(&path)
            && self.document_state.dirty_paths.contains(&path)
        {
            self.attach_parked_schematic_tab(path, title);
            return Ok(());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read schematic {}", path.display()))?;
        let sheet = signex_types::format::SnxSchematic::parse(&text)
            .with_context(|| format!("parse schematic {}", path.display()))?
            .sheet;
        self.open_schematic_tab(path, title, sheet);
        Ok(())
    }

    fn open_pcb_file(&mut self, path: PathBuf) -> Result<()> {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("read pcb {}", path.display()))?;
        let board = signex_types::format::SnxPcb::parse(&text)
            .with_context(|| format!("parse pcb {}", path.display()))?
            .board;
        // Same companion-project resolution as `open_schematic_file` so
        // the PCB tab can resolve `project_id` for project-scoped
        // handlers.
        if let Some(dir) = path.parent() {
            let stem = path
                .file_stem()
                .and_then(|segment| segment.to_str())
                .unwrap_or("");
            let companion = dir.join(format!("{stem}.snxprj"));
            if companion.exists()
                && let Err(error) = self.load_or_activate_project(&companion)
            {
                crate::diagnostics::log_error("Failed to parse companion project", &error);
            }
        }
        let title = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "PCB".to_string());
        self.open_pcb_tab(path, title, board);
        Ok(())
    }

    /// v0.22 Phase 8.4 — auto-commit a saved file into the owning
    /// project's local Git repo when `enable_git` is on.
    ///
    /// Walks `document_state.projects` looking for the project whose
    /// `data.dir` is a prefix of `file_path`. If found AND
    /// `data.enable_git == true`, opens
    /// [`LocalGitProjectAdapter`] and runs `commit_path`.
    ///
    /// Failure is best-effort: logged + surfaced as a non-modal
    /// status warning, never blocks the save. The user's data is on
    /// disk regardless of whether git captures it.
    ///
    /// v0.23 — Async pipeline. The save-handler synchronously
    /// resolves the owning project + relative path (cheap — just
    /// walks `DocumentState.projects`), then pushes a
    /// [`crate::app::state::PendingGitCommit`] onto
    /// `pending_git_commits` and adds the pair to
    /// `inflight_git_commits` so the status bar's "Saving…" pill
    /// shows immediately. The actual `git2` work runs in
    /// `finish_update`'s [`Self::drain_pending_git_commits`] which
    /// emits one `Task::perform` per queued commit. Result lands as
    /// `Message::ProjectGitCommitDone`; the handler clears the
    /// inflight entry.
    pub fn commit_save_to_project_git(
        &mut self,
        file_path: &std::path::Path,
        default_message: &str,
    ) {
        let owning = self
            .document_state
            .projects
            .iter()
            .find(|p| {
                if !p.data.enable_git {
                    return false;
                }
                let dir = std::path::Path::new(&p.data.dir);
                file_path.starts_with(dir)
            });
        let Some(project) = owning else {
            return;
        };
        let project_root = std::path::PathBuf::from(&project.data.dir);
        let rel_path = match file_path.strip_prefix(&project_root) {
            Ok(p) => p.to_path_buf(),
            Err(_) => return,
        };

        // Idempotent: ignore duplicate enqueues for the same
        // (project_root, rel_path) when the previous one is still
        // inflight. The next save round adds it back if the user
        // saves again after the prior commit completes.
        let key = (project_root.clone(), rel_path.clone());
        if !self.document_state.inflight_git_commits.insert(key) {
            return;
        }
        self.document_state
            .pending_git_commits
            .push(crate::app::state::PendingGitCommit {
                project_root,
                rel_path,
                message: default_message.to_string(),
            });
    }

    /// v0.23 — Drain the pending-commit queue. Returns a
    /// `Task::batch` of `Task::perform` calls that each open the
    /// project's git adapter and run `commit_path` on a tokio
    /// `spawn_blocking`. Each completion routes through
    /// `Message::ProjectGitCommitDone` which clears the matching
    /// `inflight_git_commits` entry. Returns `Task::none()` when the
    /// queue is empty.
    ///
    /// **Ordering note:** Concurrent commits to the same project
    /// repo are serialised by libgit2's `.git/index.lock` (and the
    /// `LocalGitProjectAdapter::git_lock` mutex), but **not** by
    /// this dispatcher. If the user types fast enough to fire two
    /// saves before the first commit completes, the second
    /// `Task::perform` may race the first; in practice both
    /// commits land sequentially with the OS-level lock determining
    /// order. The first commit's blob can therefore reflect
    /// post-second-save content if the rapid sequence overlaps
    /// `index.add_path` with the user's next save. Not data loss —
    /// every save's content is captured by *some* commit — but the
    /// commit-message vs blob-content correspondence is best-effort.
    pub(crate) fn drain_pending_git_commits(&mut self) -> iced::Task<crate::app::Message> {
        if self.document_state.pending_git_commits.is_empty() {
            return iced::Task::none();
        }
        let drained: Vec<crate::app::state::PendingGitCommit> =
            self.document_state.pending_git_commits.drain(..).collect();
        let tasks: Vec<iced::Task<crate::app::Message>> = drained
            .into_iter()
            .map(|pending| {
                let project_root = pending.project_root.clone();
                let rel_path = pending.rel_path.clone();
                let message = pending.message.clone();
                let response_root = project_root.clone();
                let response_rel = rel_path.clone();
                iced::Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let adapter = signex_library::adapters::local_git_project::LocalGitProjectAdapter::open_or_init(
                                project_root.clone(),
                            )
                            .map_err(|e| {
                                format!("open_or_init({}) failed: {e}", project_root.display())
                            })?;
                            adapter
                                .commit_path(&rel_path, &message)
                                .map(|oid| oid.to_string())
                                .map_err(|e| {
                                    format!(
                                        "commit_path({}) failed: {e}",
                                        rel_path.display()
                                    )
                                })
                        })
                        .await
                        .unwrap_or_else(|e| Err(format!("spawn_blocking: {e}")))
                    },
                    move |result| crate::app::Message::ProjectGitCommitDone {
                        project_root: response_root.clone(),
                        rel_path: response_rel.clone(),
                        result,
                    },
                )
            })
            .collect();
        iced::Task::batch(tasks)
    }

    /// v0.23 — Handler for [`Message::ProjectGitCommitDone`]. Clears
    /// the matching `inflight_git_commits` entry and logs the result.
    pub(crate) fn handle_project_git_commit_done(
        &mut self,
        project_root: std::path::PathBuf,
        rel_path: std::path::PathBuf,
        result: Result<String, String>,
    ) {
        self.document_state
            .inflight_git_commits
            .remove(&(project_root.clone(), rel_path.clone()));
        match result {
            Ok(oid) => crate::diagnostics::log_info(format!(
                "[git] committed {} in {} ({})",
                rel_path.display(),
                project_root.display(),
                oid
            )),
            Err(e) => crate::diagnostics::log_warning(format!("[git] {e}")),
        }
    }

    /// v0.22 Phase 8.5 — Resolve the active tab's path and project,
    /// then call `LocalGitProjectAdapter::restore_at` with the user-
    /// picked SHA. Marks the file dirty so the next Ctrl+S captures
    /// the restored content.
    ///
    /// Best-effort: failures log a warning and surface as a status
    /// message; nothing destructive runs (the working tree changes
    /// only on a successful blob read + atomic write).
    pub(crate) fn handle_history_restore_clicked(&mut self, sha: &str) {
        let active = match self
            .document_state
            .tabs
            .get(self.document_state.active_tab)
        {
            Some(t) => t,
            None => return,
        };
        let full_path = active.path.clone();
        if full_path.as_os_str().is_empty() {
            return;
        }

        // Find the owning project by directory prefix.
        let owning = self
            .document_state
            .projects
            .iter()
            .find(|p| {
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

        let adapter = match signex_library::adapters::local_git_project::LocalGitProjectAdapter::open_or_init(
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
                        let state =
                            crate::app::FootprintEditorState::new(p.clone(), file)
                                .with_global_snap_disabled(snap_disabled);
                        self.document_state
                            .footprint_editors
                            .insert(p.clone(), state);
                        if let Some(editor) =
                            self.document_state.footprint_editors.get_mut(&p)
                        {
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
                        self.document_state
                            .symbol_editors
                            .insert(p.clone(), state);
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
            crate::app::TabKind::LibraryBrowser(_)
            | crate::app::TabKind::ComponentEditor(_) => {
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

    fn save_active_document(&mut self) -> Result<iced::Task<Message>> {
        // Standalone `.snxsym` / `.snxfpt` document tabs route Ctrl+S
        // through `save_primitive_tab_at` so JSON persistence happens
        // before the generic schematic-save handler runs (it would
        // no-op for these tabs but the diagnostic log line would be
        // misleading). When the file doesn't exist on disk yet (the
        // tab was minted in-memory by `Add New ▸ Symbol` /
        // `Add New ▸ Footprint`), return a Task that opens an
        // AsyncFileDialog so the user can pick where the new
        // primitive lands — including a global library directory
        // outside the active project. The dialog result dispatches
        // `Message::SavePrimitiveAs { from, to }` which re-keys the
        // editor + tab and writes the file.
        if let Some(active_tab) = self.document_state.tabs.get(self.document_state.active_tab) {
            match &active_tab.kind {
                super::super::TabKind::SymbolEditor(path)
                | super::super::TabKind::FootprintEditor(path) => {
                    let path = path.clone();
                    if !path.exists() {
                        return Ok(spawn_save_as_for_new_primitive(path));
                    }
                    self.save_primitive_tab_at(&path);
                    crate::diagnostics::log_info(format!("[save] Wrote {}", path.display()));
                    // v0.14.2: also save the active project's
                    // `.snxprj` if it's dirty + refresh panel ctx so
                    // the project-root red dot drops. Adding a
                    // `.snxsym` / `.snxfpt` to `Project::libraries`
                    // marks the `.snxprj` dirty separately from the
                    // primitive itself; without these calls the
                    // project-root row stays red after a primitive
                    // save even though the primitive's row is clean.
                    self.save_active_project_if_dirty();
                    self.refresh_panel_ctx();
                    return Ok(iced::Task::none());
                }
                _ => {}
            }
        }
        if let Some(result) = self.with_active_schematic_session_mut(|session| session.save()) {
            result.context("save active schematic session")?;
            let path = self.active_tab_path().unwrap_or_default();
            crate::diagnostics::log_info(format!("[save] Wrote {}", path.display()));
            // v0.22 Phase 8.4 — auto-commit the saved schematic into
            // the owning project's git repo when enable_git is on.
            // Best-effort; failure logged + ignored (user data is on
            // disk regardless).
            if !path.as_os_str().is_empty() {
                let label = path
                    .file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string());
                let msg = format!("Save {label}");
                self.commit_save_to_project_git(&path, &msg);
            }
        }
        // v0.23 — PCB save-hook auto-commit. The PCB editor doesn't
        // own a save-back-to-disk path yet (Pcb tabs render from
        // `cached_document` and never mutate the parsed `.snxpcb`
        // through this dispatcher). When the PCB save path lands as
        // part of future PCB-editor work, wire the same
        // `commit_save_to_project_git` call here so PCB edits get
        // captured by the v0.22 project-git pipeline:
        //
        //     if let TabKind::Pcb = active_tab.kind {
        //         pcb_session.save()?;
        //         let label = path.file_name()…;
        //         self.commit_save_to_project_git(&path, &format!("Save {label}"));
        //     }
        //
        // Tracked in PROJECT_GIT_PLAN.md "Deferred to v0.23" → "PCB
        // save-hook wiring".
        // Save the active project's `.snxprj` if it's dirty (right-
        // click → Add Existing flips the dirty bit). Best-effort: a
        // failure here is logged but doesn't block the schematic save.
        self.save_active_project_if_dirty();
        // Refresh the panel context so the project-tree red dirty dot
        // (which reads `panel_ctx.projects[*].sheets[*].is_dirty`)
        // sees the updated `dirty_paths` set after the save. Without
        // this the tree row stays red even though the title (which
        // reads `dirty_paths.len()` directly each frame) clears.
        self.refresh_panel_ctx();
        Ok(iced::Task::none())
    }

    /// Persist the active project's `.snxprj` JSON when its dirty bit
    /// is set in `dirty_paths`. No-op if no project is active or the
    /// project file isn't dirty.
    ///
    /// Pending libraries (registered via the New Library flow) are
    /// materialised to disk **first** — `commands::materialize_pending_library`
    /// runs `LocalGitAdapter::init` for each entry. Successful
    /// materialisations push their `LibraryEntry` onto
    /// `project.libraries` so the subsequent `.snxprj` write captures
    /// them. Failures (e.g. target path now exists, permission glitch)
    /// keep the entry pending so the user can fix the underlying
    /// problem and retry on the next Save.
    ///
    /// Closes `feedback_no_disk_writes_without_user_save.md`'s "wait
    /// for explicit user save" invariant: modal confirm registered the
    /// pending entry; this Ctrl+S is the explicit save that actually
    /// commits to disk.
    fn save_active_project_if_dirty(&mut self) {
        let Some(project_id) = self.document_state.active_project else {
            return;
        };
        let project_path = match self
            .document_state
            .projects
            .iter()
            .find(|p| p.id == project_id)
        {
            Some(p) => p.path.clone(),
            None => return,
        };
        if !self.document_state.dirty_paths.contains(&project_path) {
            return;
        }

        // Drain the pending-library map first so the snxprj save below
        // captures the freshly-materialised LibraryEntry rows.
        let pending: Vec<(uuid::Uuid, crate::library::commands::PendingLibrarySpec)> = self
            .document_state
            .projects
            .iter_mut()
            .find(|p| p.id == project_id)
            .map(|p| p.pending_libraries.drain().collect())
            .unwrap_or_default();
        for (library_id, spec) in pending {
            // Re-borrow per iteration so the materialise call owns
            // the project + library state mutably without aliasing.
            let project_path_log;
            let result = if let Some(loaded) = self
                .document_state
                .projects
                .iter_mut()
                .find(|p| p.id == project_id)
            {
                project_path_log = loaded.path.clone();
                crate::library::commands::materialize_pending_library(
                    &mut self.library,
                    &mut loaded.data,
                    library_id,
                    &spec,
                )
            } else {
                continue;
            };
            match result {
                Ok(()) => {
                    tracing::info!(
                        target: "signex::library",
                        project = %project_path_log.display(),
                        library = %spec.lib_path.display(),
                        library_id = %library_id,
                        "materialised pending library"
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        target: "signex::library",
                        project = %project_path_log.display(),
                        library = %spec.lib_path.display(),
                        library_id = %library_id,
                        error = %error,
                        "materialise_pending_library failed; keeping entry pending for retry"
                    );
                    // Re-stash so the user can fix + retry next Save.
                    if let Some(loaded) = self
                        .document_state
                        .projects
                        .iter_mut()
                        .find(|p| p.id == project_id)
                    {
                        loaded.pending_libraries.insert(library_id, spec);
                    }
                }
            }
        }

        let data = match self
            .document_state
            .projects
            .iter()
            .find(|p| p.id == project_id)
        {
            Some(p) => p.data.clone(),
            None => return,
        };
        match signex_types::project::write_project(&project_path, &data) {
            Ok(()) => {
                self.document_state.dirty_paths.remove(&project_path);
                crate::diagnostics::log_info(format!(
                    "[save] Wrote project {}",
                    project_path.display()
                ));
                // v0.22 Phase 8.4 — auto-commit the .snxprj file into
                // its own project git repo when enable_git is on.
                let label = project_path
                    .file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_else(|| project_path.display().to_string());
                let msg = format!("Save {label}");
                self.commit_save_to_project_git(&project_path, &msg);
                // Rebuild the panel ctx so the project root row drops
                // its dirty marker. Without this, the cached
                // `ProjectPanelInfo.is_dirty` snapshot stays `true`
                // until the next user action triggers a refresh and
                // the red dot lingers despite the file being clean.
                self.refresh_panel_ctx();
            }
            Err(error) => {
                crate::diagnostics::log_error(
                    "Failed to save project file",
                    &anyhow::anyhow!("{}", error),
                );
            }
        }
    }

    fn save_active_document_as(&mut self, path: PathBuf) -> Result<()> {
        if let Some(result) =
            self.with_active_schematic_session_mut(|session| session.save_as(path.clone()))
        {
            result.with_context(|| format!("save active schematic as {}", path.display()))?;
            crate::diagnostics::log_info(format!("[save-as] Wrote {}", path.display()));
        }
        Ok(())
    }

    /// Resolve `Message::SavePrimitiveAs { from_path, to_path }` —
    /// re-key the editor and tab from the in-memory `from_path` to
    /// the user-chosen `to_path`, then write the file via
    /// `save_primitive_tab_at`. Same machinery the in-memory editor
    /// already uses for atomic writes; just runs it under the new
    /// path the user picked.
    pub(crate) fn handle_save_primitive_as(
        &mut self,
        from_path: &std::path::Path,
        to_path: &std::path::Path,
    ) {
        // Reject re-keying onto a path that already hosts a different
        // open editor — would clobber the other editor's state.
        if from_path != to_path
            && (self.document_state.symbol_editors.contains_key(to_path)
                || self.document_state.footprint_editors.contains_key(to_path))
        {
            tracing::warn!(
                target: "signex::library",
                from = %from_path.display(),
                to = %to_path.display(),
                "save-as: target path already hosts another open editor — refusing"
            );
            return;
        }

        // Move the SymbolEditorState (or FootprintEditorState) to the
        // new key. The editor's internal `path` field is updated to
        // match so commit-through-adapter / refresh-cache callers see
        // the right path.
        if let Some(mut editor) = self.document_state.symbol_editors.remove(from_path) {
            editor.path = to_path.to_path_buf();
            self.document_state
                .symbol_editors
                .insert(to_path.to_path_buf(), editor);
        } else if let Some(mut editor) = self.document_state.footprint_editors.remove(from_path) {
            editor.path = to_path.to_path_buf();
            self.document_state
                .footprint_editors
                .insert(to_path.to_path_buf(), editor);
        } else {
            tracing::warn!(
                target: "signex::library",
                from = %from_path.display(),
                "save-as: no editor at source path — was the tab closed?"
            );
            return;
        }

        // Update the tab(s) — title, path, kind variant.
        let new_title = to_path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| to_path.display().to_string());
        for tab in self.document_state.tabs.iter_mut() {
            if tab.path == from_path {
                tab.path = to_path.to_path_buf();
                tab.title = new_title.clone();
                tab.kind = match tab.kind {
                    super::super::TabKind::SymbolEditor(_) => {
                        super::super::TabKind::SymbolEditor(to_path.to_path_buf())
                    }
                    super::super::TabKind::FootprintEditor(_) => {
                        super::super::TabKind::FootprintEditor(to_path.to_path_buf())
                    }
                    _ => tab.kind.clone(),
                };
            }
        }

        // Migrate the dirty marker too — `dirty_paths` is keyed on
        // path so it has to follow the rename.
        if self.document_state.dirty_paths.remove(from_path) {
            self.document_state
                .dirty_paths
                .insert(to_path.to_path_buf());
        }

        // Now write the file at the new path. `atomic_write` inside
        // `save_primitive_tab_at` handles `create_dir_all(parent)` so
        // saving into a fresh `<lib>/symbols/` directory just works.
        self.save_primitive_tab_at(to_path);
        crate::diagnostics::log_info(format!("[save-as] Wrote {}", to_path.display()));

        // Library-tracking step. If the user saved into a `.snxlib`
        // directory that isn't mounted yet, mount it so the project
        // tree picks it up; if it isn't yet a member of the active
        // project's `libraries`, append it (project-local for paths
        // inside the project dir, Shared for paths outside) and
        // mark the project file dirty. The `.snxprj` itself isn't
        // re-serialised here — that's a separate persistence flow —
        // but the dirty marker means the next project-close prompt
        // will surface the unsaved library binding.
        self.attach_library_for_path(to_path);
    }

    /// Walk `path`'s ancestors looking for a `.snxlib` directory and,
    /// if found, make sure the active project has a `LibraryEntry`
    /// pointing at it. Mounts the library on the `LibrarySet` if it's
    /// not mounted yet. Logs and skips when there's no `.snxlib`
    /// ancestor (loose-file save outside the library system) or when
    /// no project is active to attach to.
    fn attach_library_for_path(&mut self, path: &std::path::Path) {
        let lib_dir = path.ancestors().find(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("snxlib"))
                .unwrap_or(false)
        });
        let Some(lib_dir) = lib_dir else {
            tracing::warn!(
                target: "signex::library",
                path = %path.display(),
                "save-as: file is outside any `.snxlib/` — not attached to a library"
            );
            return;
        };

        // Mount through `state.open_library` (idempotent — re-opening a
        // mounted library is a no-op). Bail on failure: an invalid
        // library directory shouldn't poison the project's library list.
        if self.library.library_at(lib_dir).is_none() {
            if let Err(e) =
                crate::library::commands::open_library(&mut self.library, lib_dir.to_path_buf())
            {
                tracing::warn!(
                    target: "signex::library",
                    path = %lib_dir.display(),
                    error = %e,
                    "save-as: open_library failed — leaving project untouched"
                );
                return;
            }
        }
        let library_id = self.library.library_at(lib_dir).map(|l| l.library_id);

        // Pick the project to attach to. Prefer the active project (so
        // the user's current focus is what gets the new entry); fall
        // back to the project that already contains `path` (rare —
        // would only happen if the user opened a primitive without an
        // active project, which the rest of the flow doesn't really
        // support). Skip silently when there's no project at all.
        let project_idx = match self.document_state.active_project {
            Some(active_id) => self
                .document_state
                .projects
                .iter()
                .position(|p| p.id == active_id),
            None => None,
        };
        let Some(project_idx) = project_idx else {
            tracing::warn!(
                target: "signex::library",
                path = %lib_dir.display(),
                "save-as: no active project to attach the library to"
            );
            return;
        };

        let loaded = &mut self.document_state.projects[project_idx];
        let project_dir = std::path::PathBuf::from(&loaded.data.dir);

        // Already in the project? Match by library_id when we have one
        // (handles renames); otherwise compare resolved paths.
        let already_attached = loaded.data.libraries.iter().any(|entry| {
            if let (Some(eid), Some(lid)) = (entry.library_id, library_id) {
                eid == lid
            } else {
                loaded.data.resolve_library_path(entry) == lib_dir
            }
        });
        if already_attached {
            self.refresh_panel_ctx();
            return;
        }

        // Project-local when the library lives inside the project dir,
        // Shared otherwise. Project-local stores the path relative to
        // the project so a project move doesn't break the binding.
        let (kind, stored_path) = if let Ok(rel) = lib_dir.strip_prefix(&project_dir) {
            (
                signex_types::project::LibraryEntryKind::ProjectLocal,
                rel.to_path_buf(),
            )
        } else {
            (
                signex_types::project::LibraryEntryKind::Shared,
                lib_dir.to_path_buf(),
            )
        };

        loaded
            .data
            .libraries
            .push(signex_types::project::LibraryEntry {
                path: stored_path,
                kind,
                library_id,
            });

        // Mark the `.snxprj` dirty so the project tree's red dot lights
        // up and the project-close prompt asks the user to persist the
        // new binding. (The actual `.snxprj` write isn't wired yet — a
        // follow-up commit will plumb that through Ctrl+S; for now the
        // in-session view is correct, persistence is best-effort.)
        let project_path = loaded.path.clone();
        self.document_state.dirty_paths.insert(project_path);

        tracing::info!(
            target: "signex::library",
            project = %loaded.path.display(),
            library = %lib_dir.display(),
            "save-as: attached library to active project"
        );

        self.refresh_panel_ctx();
    }
}

/// Build the AsyncFileDialog Task for a primitive's first save.
/// The dialog defaults to the suggested path's parent + filename so
/// the common case is a single Enter key; the user can navigate to
/// a global library directory outside the project if they want a
/// shared symbol. Cancel = no save (editor stays dirty).
pub(crate) fn spawn_save_as_for_new_primitive(suggested: PathBuf) -> iced::Task<Message> {
    let ext = suggested
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("snxsym")
        .to_string();
    let (filter_label, filter_ext) = match ext.as_str() {
        "snxfpt" => ("Signex Footprint", "snxfpt"),
        _ => ("Signex Symbol", "snxsym"),
    };
    let title = match ext.as_str() {
        "snxfpt" => "Save Footprint As",
        _ => "Save Symbol As",
    };
    let default_dir = suggested
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let default_name = suggested
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("New.{filter_ext}"));
    let from = suggested;

    iced::Task::perform(
        async move {
            rfd::AsyncFileDialog::new()
                .set_title(title)
                .add_filter(filter_label, &[filter_ext])
                .set_directory(&default_dir)
                .set_file_name(&default_name)
                .save_file()
                .await
                .map(|file| file.path().to_path_buf())
        },
        move |picked| match picked {
            Some(to_path) => Message::SavePrimitiveAs {
                from_path: from.clone(),
                to_path,
            },
            None => Message::Noop,
        },
    )
}

/// Build the bare-minimum [`SchematicSheet`] used as the starting state
/// for File ▸ New Project. Only the fields that don't have a serde
/// default need explicit values; everything else falls through to the
/// per-field defaults the writer/parser already round-trip.
pub(crate) fn blank_schematic_sheet_for_new_doc() -> signex_types::schematic::SchematicSheet {
    blank_schematic_sheet()
}

fn blank_schematic_sheet() -> signex_types::schematic::SchematicSheet {
    signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        version: 1,
        generator: "signex".into(),
        generator_version: env!("CARGO_PKG_VERSION").into(),
        paper_size: "A4".into(),
        root_sheet_page: "1".into(),
        symbols: Vec::new(),
        wires: Vec::new(),
        junctions: Vec::new(),
        labels: Vec::new(),
        child_sheets: Vec::new(),
        no_connects: Vec::new(),
        text_notes: Vec::new(),
        buses: Vec::new(),
        bus_entries: Vec::new(),
        drawings: Vec::new(),
        no_erc_directives: Vec::new(),
        title_block: Default::default(),
        lib_symbols: Default::default(),
    }
}
