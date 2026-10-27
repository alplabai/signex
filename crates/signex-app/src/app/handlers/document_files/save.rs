//! Document/project save handlers. Split from `handlers/document_files.rs`.

use std::path::PathBuf;

use anyhow::{Context, Result};

use super::super::super::*;

/// Messages trims an entry to this many bytes.
const MESSAGES_ENTRY_BYTES: usize = 160;

/// The advice leads each per-object line so a long object description is
/// the part that gets cut, never the instruction.
const OFFENDER_LINE_PREFIX: &str = "Save blocked, delete column past limit: ";

/// How a file is named in a Messages line: the bare file name, since the
/// panel keeps ~157 bytes and a full path alone can use them up.
pub(super) fn file_label(path: &std::path::Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// `text` cut to at most `max_bytes`, on a char boundary.
fn clip_to_bytes(text: &str, max_bytes: usize) -> &str {
    let mut end = text.len().min(max_bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// A save refused for too many foreign TSV columns gets one Messages
/// line per object that carries a column past the cap, since a single
/// entry is trimmed to 160 bytes and would lose the names.
pub(crate) fn log_foreign_column_offenders(error: &anyhow::Error) {
    use signex_types::format::FormatError;
    let Some(FormatError::TsvTooManyForeignColumns {
        objects,
        more_objects,
        ..
    }) = error.chain().find_map(|cause| {
        if let Some(format_error) = cause.downcast_ref::<FormatError>() {
            return Some(format_error);
        }
        cause
            .downcast_ref::<std::io::Error>()?
            .get_ref()?
            .downcast_ref::<FormatError>()
    })
    else {
        return;
    };
    let room = MESSAGES_ENTRY_BYTES - OFFENDER_LINE_PREFIX.len();
    for object in objects {
        crate::diagnostics::log_warning(format!(
            "{OFFENDER_LINE_PREFIX}{}",
            clip_to_bytes(object, room)
        ));
    }
    if *more_objects > 0 {
        crate::diagnostics::log_warning(format!(
            "Save blocked: {more_objects} more objects carry a column past the limit"
        ));
    }
}

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
                        return Ok(super::spawn_save_as_for_primitive(path));
                    }
                    // The write runs off the UI thread; the completion logs
                    // it, clears the dirty marker and queues the git commit.
                    let task = match self.save_primitive_tab_at(&path) {
                        Ok(task) => task,
                        Err(e) => {
                            self.report_save_failure("Save failed", &e);
                            iced::Task::none()
                        }
                    };
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
                    return Ok(task);
                }
                _ => {}
            }
        }
        // The write runs off the UI thread; the git auto-commit and the
        // dirty-marker clear follow its completion (`handle_document_saved`).
        let mut saves = Vec::new();
        if let Some(path) = self.document_state.active_path.clone()
            && self.document_state.engines.contains_key(&path)
        {
            saves.push(self.begin_schematic_save(path, None)?);
        }
        if let Some(tab) = self.document_state.tabs.get(self.document_state.active_tab)
            && matches!(tab.kind, super::super::super::TabKind::Pcb)
        {
            let path = tab.path.clone();
            match self.begin_pcb_save(path) {
                Ok(task) => saves.extend(task),
                Err(e) => self.report_save_failure("Failed to save PCB", &e),
            }
        }
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
        Ok(iced::Task::batch(saves))
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
        if let Err(error) = self.persist_project_by_id(project_id) {
            self.report_save_failure("Failed to save project file", &error);
        }
    }

    /// A save failed: log it and raise the Save Failed card. A log line
    /// alone is invisible, and the dirty marker stays set, so the user
    /// would believe Ctrl+S worked (#110).
    pub(crate) fn report_save_failure(&mut self, context: &str, error: &anyhow::Error) {
        crate::diagnostics::log_error(context, error);
        log_foreign_column_offenders(error);
        // The first failure of a save sequence is the one that explains it;
        // a follow-up write (e.g. the project file) must not replace it.
        if self.document_state.error_notice.is_none() {
            self.document_state.error_notice = Some(crate::app::state::ErrorNotice::save(format!(
                "{context}: {error:#}"
            )));
        }
    }

    /// Save the loaded project whose `.snxprj` sits at `path`. Thin
    /// path-keyed wrapper over [`persist_project_by_id`] used by the
    /// Save-All-on-close/exit dispatcher, which iterates dirty paths and
    /// cannot assume the dirty `.snxprj` belongs to the *active*
    /// project. Returns an error when no loaded project owns `path`, or
    /// when the underlying `.snxprj` write fails.
    pub(crate) fn save_project_at_path(&mut self, path: &std::path::Path) -> Result<()> {
        let project_id = self
            .document_state
            .projects
            .iter()
            .find(|p| p.path == path)
            .map(|p| p.id)
            .with_context(|| format!("no loaded project at {}", path.display()))?;
        self.persist_project_by_id(project_id)
    }

    /// Materialise any pending libraries, then write the identified
    /// project's `.snxprj` to disk. On success clears the project's
    /// dirty marker, auto-commits to the project git repo, and
    /// refreshes the panel context. Returns an error for a failed
    /// `.snxprj` write, and also when a pending library could not be
    /// created: the entry is re-stashed for retry, the `.snxprj` is still
    /// written, but the project stays dirty and the error names the
    /// libraries it lacks, so Save All callers never treat it as saved.
    pub(crate) fn persist_project_by_id(
        &mut self,
        project_id: crate::app::state::ProjectId,
    ) -> Result<()> {
        let project_path = match self
            .document_state
            .projects
            .iter()
            .find(|p| p.id == project_id)
        {
            Some(p) => p.path.clone(),
            None => anyhow::bail!("project {project_id} is no longer loaded"),
        };
        // The write would recreate the old folder a rename is about to
        // remove, and the edit would be lost after the re-key.
        self.refuse_while_project_moves(&project_path, "save")?;

        // Drain the pending-library map first so the snxprj save below
        // captures the freshly-materialised LibraryEntry rows.
        let pending: Vec<(uuid::Uuid, crate::library::commands::PendingLibrarySpec)> = self
            .document_state
            .projects
            .iter_mut()
            .find(|p| p.id == project_id)
            .map(|p| p.pending_libraries.drain().collect())
            .unwrap_or_default();
        let mut unmaterialised: Vec<String> = Vec::new();
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
                    loaded.case_fold,
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
                    unmaterialised.push(format!("{} ({error})", spec.display_name));
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
            None => anyhow::bail!("project {project_id} vanished before write"),
        };
        signex_types::project::write_project(&project_path, &data)
            .with_context(|| format!("write project {}", project_path.display()))?;

        // The `.snxprj` is on disk but lacks a library that still exists
        // only in memory. Not saved: the project stays dirty, and Save All
        // (quit, Close Project) reports it instead of dropping the library.
        if !unmaterialised.is_empty() {
            anyhow::bail!(
                "{} was written without {} new librar{} that could not be created: {}",
                project_path.display(),
                unmaterialised.len(),
                if unmaterialised.len() == 1 {
                    "y"
                } else {
                    "ies"
                },
                unmaterialised.join("; ")
            );
        }
        self.document_state.dirty_paths.remove(&project_path);
        crate::diagnostics::log_info(format!("[save] Wrote project {}", project_path.display()));
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
        Ok(())
    }

    /// Schematic Save As. `from_path` is the tab the dialog was opened
    /// for; the dialog is not modal, so refuse when the active tab is no
    /// longer that schematic instead of writing another document to
    /// `path`. Symbol / footprint tabs go through `handle_save_primitive_as`.
    pub(crate) fn save_active_document_as(
        &mut self,
        from_path: &std::path::Path,
        path: PathBuf,
    ) -> Result<iced::Task<Message>> {
        match self.document_state.tabs.get(self.document_state.active_tab) {
            Some(tab) if matches!(tab.kind, TabKind::Schematic) && tab.path == from_path => {}
            _ => anyhow::bail!(
                "save as {}: the active tab is no longer the schematic {} (it was \
                 switched or closed while the dialog was open) — nothing was written",
                path.display(),
                from_path.display()
            ),
        }
        if self.save_as_target_is_other_open_document(&path) {
            // Reason first and the bare file name: the Messages panel keeps
            // ~157 bytes, which a full path alone can use up.
            anyhow::bail!(
                "that file is open, still opening, or holds unsaved edits; nothing was \
                 written: {}",
                file_label(&path)
            );
        }
        if self.document_state.document_saves.is_saving(&path) {
            anyhow::bail!(
                "a write to that file (or a case variant of its name) is still running; try \
                 again once it finishes: {}",
                file_label(&path)
            );
        }
        // The tab keeps its old path and title until the write succeeds.
        self.begin_schematic_save(path, Some(from_path.to_path_buf()))
    }

    /// Whether `path` is hosted by another open tab or parked engine, or
    /// its open is still in flight: re-keying onto it would replace that
    /// engine and discard its unsaved edits, or let the late open result
    /// replace the re-keyed engine (#1089).
    pub(super) fn save_as_target_is_other_open_document(&self, path: &std::path::Path) -> bool {
        let active = self.document_state.active_path.as_deref();
        // Another spelling of the same file counts (`lib.snxsch` vs the open
        // `Lib.snxsch` on a case-insensitive volume): re-keying onto it would
        // leave two documents on one file.
        if self
            .held_spelling(path)
            .is_some_and(|held| Some(held.as_path()) != active)
        {
            return true;
        }
        active != Some(path)
            && (self.document_state.engines.contains_key(path)
                || self.document_state.tabs.iter().any(|tab| tab.path == path)
                || self.document_state.pending_opens.contains(path))
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
        picked_path: &std::path::Path,
    ) -> Result<iced::Task<Message>> {
        // The OS dialog only confirmed overwriting the name the user
        // typed. When we append the extension the target is a different
        // file the user was never asked about, so refuse if it exists.
        let resolved = super::with_primitive_extension(
            picked_path.to_path_buf(),
            super::primitive_extension(from_path),
        );
        let to_path = resolved.as_path();
        if to_path != picked_path {
            if to_path.exists() {
                crate::diagnostics::log_warning(format!(
                    "[save-as] {} already exists and was not overwritten; \
                     pick the full file name including the extension to replace it",
                    to_path.display()
                ));
                return Ok(iced::Task::none());
            }
            crate::diagnostics::log_info(format!(
                "[save-as] No file extension given; saving as {}",
                to_path.display()
            ));
        }

        // Re-key first so the save (keyed by the editor's path) writes the
        // new file. The write runs off the UI thread; if it fails the
        // completion undoes the re-key, so a tab never points at a file
        // that does not exist. On success the completion runs the
        // library-tracking step (`attach_library_for_path`): mount a
        // `.snxlib` the file landed in and bind it to the active project.
        let previous_title = self.retarget_primitive(from_path, to_path, None)?;
        let origin = crate::persist::SaveAsOrigin {
            from: from_path.to_path_buf(),
            title: previous_title,
        };
        match self.save_primitive_tab_replacing(to_path, origin.clone()) {
            Ok(task) => Ok(task),
            Err(error) => {
                self.retarget_primitive(to_path, from_path, origin.title)
                    .context("roll back Save As re-key")?;
                Err(error.context(format!(
                    "save as {}: write failed, document left at {}",
                    to_path.display(),
                    from_path.display()
                )))
            }
        }
    }

    /// Move the symbol / footprint editor, its tab(s) and the dirty
    /// marker from `from` to `to`. `title` overrides the tab title (used
    /// by the rollback); `None` derives it from `to`'s file stem.
    /// Returns the title the first moved tab had before the move.
    ///
    /// Refuses, moving nothing, when `to` already hosts another editor or
    /// tab: overwriting it would drop that editor's edits and leave two
    /// tabs on one file.
    pub(crate) fn retarget_primitive(
        &mut self,
        from: &std::path::Path,
        to: &std::path::Path,
        title: Option<String>,
    ) -> Result<Option<String>> {
        if from != to
            && (self
                .held_spelling(to)
                .is_some_and(|held| held.as_path() != from)
                || self.document_state.symbol_editors.contains_key(to)
                || self.document_state.footprint_editors.contains_key(to)
                || self.document_state.tabs.iter().any(|tab| tab.path == to))
        {
            anyhow::bail!("{} is already open in another tab", to.display());
        }
        if let Some(mut editor) = self.document_state.symbol_editors.remove(from) {
            editor.path = to.to_path_buf();
            self.document_state
                .symbol_editors
                .insert(to.to_path_buf(), editor);
        } else if let Some(mut editor) = self.document_state.footprint_editors.remove(from) {
            editor.path = to.to_path_buf();
            self.document_state
                .footprint_editors
                .insert(to.to_path_buf(), editor);
        } else {
            anyhow::bail!("no editor at {} — was the tab closed?", from.display());
        }

        let new_title = title.unwrap_or_else(|| {
            to.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| to.display().to_string())
        });
        let mut previous_title = None;
        for tab in self.document_state.tabs.iter_mut() {
            if tab.path == from {
                previous_title.get_or_insert_with(|| tab.title.clone());
                tab.path = to.to_path_buf();
                tab.title = new_title.clone();
                tab.kind = match tab.kind {
                    TabKind::SymbolEditor(_) => TabKind::SymbolEditor(to.to_path_buf()),
                    TabKind::FootprintEditor(_) => TabKind::FootprintEditor(to.to_path_buf()),
                    _ => tab.kind.clone(),
                };
            }
        }
        // `dirty_paths` is keyed on path, so it follows the move.
        if self.document_state.dirty_paths.remove(from) {
            self.document_state.dirty_paths.insert(to.to_path_buf());
        }
        Ok(previous_title)
    }

    /// Walk `path`'s ancestors looking for a `.snxlib` directory and,
    /// if found, make sure the active project has a `LibraryEntry`
    /// pointing at it. Mounts the library on the `LibrarySet` if it's
    /// not mounted yet. Logs and skips when there's no `.snxlib`
    /// ancestor (loose-file save outside the library system) or when
    /// no project is active to attach to.
    pub(crate) fn attach_library_for_path(&mut self, path: &std::path::Path) {
        let lib_dir = path.ancestors().find(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("snxlib"))
                .unwrap_or(false)
        });
        let Some(lib_dir) = lib_dir else {
            crate::diagnostics::log_warning(format!(
                "[save-as] {} is outside any `.snxlib/` — not attached to a library",
                path.display()
            ));
            return;
        };

        // Mount through `state.open_library` (idempotent — re-opening a
        // mounted library is a no-op). Bail on failure: an invalid
        // library directory shouldn't poison the project's library list.
        if self.library.library_at(lib_dir).is_none()
            && let Err(e) =
                crate::library::commands::open_library(&mut self.library, lib_dir.to_path_buf())
        {
            crate::diagnostics::log_warning(format!(
                "[save-as] could not open library {} ({e}) — project left untouched",
                lib_dir.display()
            ));
            return;
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
            crate::diagnostics::log_warning(format!(
                "[save-as] no active project to attach library {} to",
                lib_dir.display()
            ));
            return;
        };

        let loaded = &mut self.document_state.projects[project_idx];
        let project_dir = std::path::PathBuf::from(&loaded.data.dir);
        let fold = loaded.case_fold;

        // Already in the project? Match by library_id when we have one
        // (handles renames); otherwise compare resolved paths.
        use crate::app::state::library_entry::library_entry_location;
        use crate::app::state::scope::path_key_with;
        let wanted = path_key_with(lib_dir, fold);
        let already_attached = loaded.data.libraries.iter().any(|entry| {
            if let (Some(eid), Some(lid)) = (entry.library_id, library_id) {
                eid == lid
            } else {
                path_key_with(&loaded.data.resolve_library_path(entry), fold) == wanted
            }
        });
        if already_attached {
            self.refresh_panel_ctx();
            return;
        }

        // Project-local when the library lives inside the project dir,
        // Shared otherwise. Project-local stores the path relative to
        // the project so a project move doesn't break the binding.
        let (stored_path, kind) = library_entry_location(&project_dir, lib_dir, fold);

        loaded
            .data
            .libraries
            .push(signex_types::project::LibraryEntry {
                path: stored_path,
                kind,
                library_id,
                unknown: Default::default(),
            });

        // Mark the `.snxprj` dirty so the project tree's red dot lights
        // up and the project-close prompt asks the user to persist the
        // new binding. (The actual `.snxprj` write isn't wired yet — a
        // follow-up commit will plumb that through Ctrl+S; for now the
        // in-session view is correct, persistence is best-effort.)
        let project_path = loaded.path.clone();
        self.document_state.dirty_paths.insert(project_path);

        let scope = match kind {
            signex_types::project::LibraryEntryKind::Shared => "Shared",
            _ => "project-local",
        };
        crate::diagnostics::log_info(format!(
            "[save-as] Attached library {} to project {} as {scope}",
            lib_dir.display(),
            loaded.path.display()
        ));

        self.refresh_panel_ctx();
    }
}
