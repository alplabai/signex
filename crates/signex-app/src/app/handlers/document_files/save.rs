//! Document/project save handlers. Split from `handlers/document_files.rs`.

use std::path::PathBuf;

use anyhow::{Context, Result};

use super::super::super::*;

impl Signex {
    pub(crate) fn save_active_document(&mut self) -> Result<iced::Task<Message>> {
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
        // `Message::File(FileMsg::SavePrimitiveAs { from, to })` which
        // re-keys the editor + tab and writes the file.
        if let Some(active_tab) = self.document_state.tabs.get(self.document_state.active_tab) {
            match &active_tab.kind {
                super::super::super::TabKind::SymbolEditor(path)
                | super::super::super::TabKind::FootprintEditor(path) => {
                    let path = path.clone();
                    if !path.exists() {
                        return Ok(super::spawn_save_as_for_new_primitive(path));
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

    pub(crate) fn save_active_document_as(&mut self, path: PathBuf) -> Result<()> {
        if let Some(result) =
            self.with_active_schematic_session_mut(|session| session.save_as(path.clone()))
        {
            result.with_context(|| format!("save active schematic as {}", path.display()))?;
            crate::diagnostics::log_info(format!("[save-as] Wrote {}", path.display()));
        }
        Ok(())
    }

    /// Resolve `Message::File(FileMsg::SavePrimitiveAs { from_path, to_path })` —
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
                    super::super::super::TabKind::SymbolEditor(_) => {
                        super::super::super::TabKind::SymbolEditor(to_path.to_path_buf())
                    }
                    super::super::super::TabKind::FootprintEditor(_) => {
                        super::super::super::TabKind::FootprintEditor(to_path.to_path_buf())
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
