//! Document/project save handlers. Split from `handlers/document_files.rs`.

use std::path::PathBuf;

use anyhow::{Context, Result};

use super::super::super::*;
use crate::app::document::DocumentKey;

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
    /// Ctrl+S: the active tab's document is named once as a [`DocumentKey`]
    /// and saved by its kind (ADR-0001 A2). The active project's `.snxprj`
    /// is saved beside every kind but a Project tab.
    pub(crate) fn save_active_document(&mut self) -> Result<iced::Task<Message>> {
        if let Some(path) = self.document_state.active_path.clone() {
            self.commit_title_block_drafts_where(|sheet| sheet == path);
        }
        let key = match self.document_state.tabs.get(self.document_state.active_tab) {
            Some(tab) => {
                // Refused before any branch below could ask for a Save As
                // dialog for a path that is only missing because it is being
                // renamed.
                self.refuse_save_while_path_op(&tab.path)?;
                self.refuse_save_while_restoring(&tab.path)?;
                DocumentKey::of(tab)
            }
            None => None,
        };
        match key {
            // A Project tab writes its `.snxprj` off the UI thread.
            Some(DocumentKey::Project(project_id)) => self.begin_project_save(project_id),
            Some(DocumentKey::Symbol(path) | DocumentKey::Footprint(path)) => {
                Ok(self.save_primitive_document(path))
            }
            key => self.save_sheet_pcb_and_project(key),
        }
    }

    /// Ctrl+S on a standalone `.snxsym` / `.snxfpt` tab: persist the JSON
    /// through `save_primitive_tab_at`, or, when the file does not exist
    /// yet (the tab was minted in memory by `Add New`), open a dialog for
    /// where the primitive lands, including a global library directory
    /// outside the active project. Its result dispatches
    /// `FileMsg::SavePrimitiveAs`, which re-keys the editor and tab and
    /// writes the file.
    fn save_primitive_document(&mut self, path: PathBuf) -> iced::Task<Message> {
        if !path.exists() {
            return super::spawn_save_as_for_primitive(self.ui_state.io.picker.clone(), path);
        }
        // The write runs off the UI thread; the completion logs it, clears
        // the dirty marker and queues the git commit.
        let task = match self.save_primitive_tab_at(&path) {
            Ok(task) => task,
            Err(e) => {
                self.report_save_failure("Save failed", &e);
                iced::Task::none()
            }
        };
        // Adding a primitive to `Project::libraries` marks the `.snxprj`
        // dirty apart from the primitive; saving it here drops the
        // project-root red dot with the primitive's own.
        let project_save = self.save_active_project_if_dirty();
        self.refresh_panel_ctx();
        iced::Task::batch([task, project_save])
    }

    /// Ctrl+S on a sheet, a PCB or any tab with no file of its own: write
    /// the active sheet and the PCB `key` names, then the dirty project.
    fn save_sheet_pcb_and_project(
        &mut self,
        key: Option<DocumentKey>,
    ) -> Result<iced::Task<Message>> {
        // The write runs off the UI thread; the git auto-commit and the
        // dirty-marker clear follow its completion (`handle_document_saved`).
        let mut saves = Vec::new();
        if let Some(path) = self.document_state.active_path.clone()
            && self.document_state.engines.contains_key(&path)
        {
            // Another spelling of this file held open must not be overwritten
            // by this write (#1447).
            match self.guard_spelling_save(&path)? {
                super::dir_case::SaveGuard::Clear => {
                    saves.push(self.begin_schematic_save(path, None)?);
                }
                super::dir_case::SaveGuard::Waiting(probe) => saves.push(probe),
            }
        }
        if let Some(DocumentKey::Pcb(path)) = key {
            match self.begin_pcb_save(path) {
                Ok(task) => saves.extend(task),
                Err(e) => self.report_save_failure("Failed to save PCB", &e),
            }
        }
        // Best-effort: a failure saving the project file is logged but does
        // not block the document save.
        saves.push(self.save_active_project_if_dirty());
        // The project tree's red dirty dot reads the panel context, so it
        // must be refreshed to see the cleared dirty mark.
        self.refresh_panel_ctx();
        Ok(iced::Task::batch(saves))
    }

    /// Save the active project's `.snxprj` when it is dirty, off the UI
    /// thread. No-op if no project is active or the file isn't dirty.
    ///
    /// Pending libraries (registered via the New Library flow) are created
    /// by the same worker job, before the file that lists them
    /// (`begin_project_save`). A failure is reported and the project stays
    /// dirty so the user can fix the cause and save again; it never blocks
    /// the document save this runs beside.
    fn save_active_project_if_dirty(&mut self) -> iced::Task<Message> {
        let Some(project_id) = self.document_state.active_project else {
            return iced::Task::none();
        };
        match self.begin_project_save(project_id) {
            Ok(task) => task,
            Err(error) => {
                self.report_save_failure("Failed to save project file", &error);
                iced::Task::none()
            }
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

    /// Start the off-thread save of the loaded project whose `.snxprj` sits
    /// at `path`. Path-keyed for the Save-All-on-close/exit dispatcher, which
    /// iterates dirty paths and cannot assume the dirty `.snxprj` belongs to
    /// the *active* project. Errs when no loaded project owns `path` or the
    /// save cannot start; the write's own failure is reported by its
    /// completion and leaves the file dirty, which the Save All settle step
    /// reads.
    pub(crate) fn save_project_at_path(
        &mut self,
        path: &std::path::Path,
    ) -> Result<iced::Task<Message>> {
        let project_id = self
            .document_state
            .projects
            .iter()
            .find(|p| p.path == path)
            .map(|p| p.id)
            .with_context(|| format!("no loaded project at {}", path.display()))?;
        self.begin_project_save(project_id)
    }

    /// Start the write of the document at `path`, by the kind that holds it
    /// (a sheet, a symbol or footprint draft, a project file), without
    /// asking which tab is active (#2172). Like Save All's per-file write,
    /// but a sheet also keeps the case-variant guard of Ctrl+S.
    pub(crate) fn begin_document_path_save(
        &mut self,
        path: &std::path::Path,
    ) -> Result<iced::Task<Message>> {
        self.refuse_save_while_path_op(path)?;
        self.refuse_save_while_restoring(path)?;
        match self.document_state.document_key_for_path(path) {
            Some(DocumentKey::Schematic(_)) => match self.guard_spelling_save(path)? {
                super::dir_case::SaveGuard::Clear => {
                    self.begin_schematic_save(path.to_path_buf(), None)
                }
                super::dir_case::SaveGuard::Waiting(probe) => Ok(probe),
            },
            Some(DocumentKey::Symbol(_) | DocumentKey::Footprint(_)) => {
                self.save_primitive_tab_at(path)
            }
            Some(DocumentKey::Project(_)) => self.save_project_at_path(path),
            Some(DocumentKey::Pcb(_) | DocumentKey::Row(_)) | None => anyhow::bail!(
                "no open editor, engine or project backs this file, so it cannot be saved"
            ),
        }
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
        self.commit_title_block_drafts_where(|sheet| sheet == from_path);
        self.save_sheet_as(from_path, path)
    }

    /// [`Self::save_active_document_as`] without taking the person's
    /// title-block drafts into the sheet first: an agent's Save As writes the
    /// document as it is (#2172).
    pub(crate) fn save_sheet_as(
        &mut self,
        from_path: &std::path::Path,
        path: PathBuf,
    ) -> Result<iced::Task<Message>> {
        self.refuse_save_while_restoring(from_path)?;
        // A name typed without an extension gets `.snxsch`, like the
        // symbol / footprint Save As; if that file exists the user was never
        // asked about overwriting it, so nothing is written.
        let picked = path;
        let path = super::with_primitive_extension(picked.clone(), "snxsch");
        if path != picked {
            if path.exists() {
                crate::diagnostics::log_warning(format!(
                    "[save-as] {} already exists and was not overwritten; \
                     pick the full file name including the extension to replace it",
                    path.display()
                ));
                return Ok(iced::Task::none());
            }
            crate::diagnostics::log_info(format!(
                "[save-as] No file extension given; saving as {}",
                path.display()
            ));
        }
        // The target may be a document whose restore is still running (even
        // one cancelled): the restore would overwrite what this writes.
        self.refuse_save_while_restoring(&path)?;
        self.refuse_save_as_target(&path, "snxsch")?;
        let from_key = DocumentKey::Schematic(from_path.to_path_buf());
        match self.document_state.tabs.get(self.document_state.active_tab) {
            Some(tab) if DocumentKey::of(tab).as_ref() == Some(&from_key) => {}
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
        self.refuse_while_path_op(from_path, "saved")?;
        self.refuse_while_path_op(picked_path, "saved")?;
        // The OS dialog only confirmed overwriting the name the user
        // typed. When we append the extension the target is a different
        // file the user was never asked about, so refuse if it exists.
        let resolved = super::with_primitive_extension(
            picked_path.to_path_buf(),
            super::primitive_extension(from_path),
        );
        let to_path = resolved.as_path();
        // Re-keying onto a file whose restore is still running (even a
        // cancelled one) would let the restore overwrite this save; say so
        // before the generic "already open" refusal can.
        self.refuse_save_while_restoring(to_path)?;
        self.refuse_save_as_target(to_path, super::primitive_extension(from_path))?;
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

    /// Move the symbol / footprint editor, its tab(s), the dirty marker and
    /// any undocked window showing it from `from` to `to`. `title` overrides the tab title (used
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
        self.document_state.rekey_document_dirty(from, to);
        // An undocked window showing the editor follows it, so its
        // per-window commands still find the document (#1801). The rollback
        // of a failed Save As runs through here too.
        if from != to {
            self.rekey_undocked_windows(from, to);
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
        // new binding.
        let project_path = loaded.path.clone();

        let scope = match kind {
            signex_types::project::LibraryEntryKind::Shared => "Shared",
            _ => "project-local",
        };
        crate::diagnostics::log_info(format!(
            "[save-as] Attached library {} to project {} as {scope}",
            lib_dir.display(),
            loaded.path.display()
        ));
        self.document_state.mark_document_dirty(&project_path);

        self.refresh_panel_ctx();
    }
}
