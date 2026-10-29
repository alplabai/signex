//! Renaming a project library's `.snxlib` from the project tree (#739).
//!
//! The file rename and the re-point of everything that knows the library by
//! its path happen in one synchronous `update`, like the sheet rename: the
//! project's `LibraryEntry`, the mounted adapter, and every open tab, editor
//! map and dirty marker move together, so no write can land between the file
//! moving and the mount following it. The one library entry of the saved
//! `.snxprj` is patched atomically on disk in the same update (so a Discard,
//! quit or crash cannot leave the project listing a file that no longer
//! exists), but every other unsaved project edit stays in memory and the
//! project stays dirty for it. A project that was
//! never saved has no file to patch, so it is marked dirty and the Messages
//! panel says it needs saving.
//!
//! Only project-local libraries are renameable here. Shared and Global
//! libraries live outside the project and stay refused (#657).

use super::*;
use crate::app::state::ErrorNotice;
use crate::library::state::rebase_path;
use signex_types::project::{LibraryEntryKind, LibraryRetarget, retarget_library_entry};
use std::path::{Path, PathBuf};

const NOT_PROJECT_LOCAL: &str = "Only libraries stored inside the project can be renamed from the \
     project tree. Shared and global libraries are referenced by path.";

/// Why `apply_library_rename` failed, and which saved project (if any) is
/// left listing the new path.
struct RenameFailure {
    reason: String,
    stale_project: Option<PathBuf>,
}

impl Signex {
    /// The `.snxlib` the dialog targets, when it is one of its owning
    /// project's library entries (matched relative to the project dir or
    /// absolute). Returns the entry's kind.
    pub(super) fn project_library_kind(
        &self,
        state: &crate::app::RenameDialogState,
    ) -> Option<LibraryEntryKind> {
        let loaded = state
            .tree_path
            .first()
            .and_then(|&idx| self.document_state.projects.get(idx))?;
        loaded
            .data
            .libraries
            .iter()
            .find(|e| loaded.data.resolve_library_path(e) == state.target_path)
            .map(|e| e.kind)
    }

    /// Validate and perform a library rename.
    ///
    /// The `fs::rename` and the in-memory re-point happen inside this one
    /// `update`, the way a sheet rename does: no message can be processed
    /// between the file moving and the mount following it, so no row edit
    /// can recreate the old `.snxlib` or miss the renamed one. The
    /// destination is checked for existence immediately before the rename.
    pub(super) fn begin_library_rename(
        &mut self,
        state: &crate::app::RenameDialogState,
        new_path: PathBuf,
    ) -> iced::Task<Message> {
        let old_path = state.target_path.clone();
        if let Some(reason) = self.library_rename_refusal(state, &old_path) {
            self.set_rename_error(&reason);
            return iced::Task::none();
        }
        let Some((project, project_file)) = state
            .tree_path
            .first()
            .and_then(|&idx| self.document_state.projects.get(idx))
            .map(|p| (p.id, p.path.clone()))
        else {
            self.set_rename_error("The owning project is no longer open.");
            return iced::Task::none();
        };
        // The rename rewrites the `.snxprj` directly. An off-thread write of
        // it still in flight would land afterwards and put the old library
        // path back; it runs only after `update` returns, so refuse (before
        // anything moves) rather than wait (design doc
        // 2026-10-26-project-tab-shell §3).
        if self.document_state.document_saves.is_saving(&project_file) {
            self.refuse_rename_while_project_saving();
            return iced::Task::none();
        }
        // The same holds for a draft inside the library: its write names the
        // old path and its completion would find no editor behind it.
        if self
            .library_draft_paths(&old_path)
            .any(|draft| self.document_state.document_saves.is_saving(&draft))
        {
            self.refuse_rename_while_library_saving();
            return iced::Task::none();
        }
        if new_path.exists() {
            self.set_rename_error("A file with that name already exists.");
            return iced::Task::none();
        }
        if let Err(e) = std::fs::rename(&old_path, &new_path) {
            let reason = format!("the file could not be renamed: {e}");
            self.report_library_rename_failure(&old_path, &reason);
            self.set_rename_error(&format!("Rename failed: {e}"));
            self.sync_diagnostics_panel_ctx();
            return iced::Task::none();
        }
        self.ui_state.rename_dialog = None;
        match self.apply_library_rename(project, &old_path, &new_path) {
            Ok(outcome) => {
                // The Messages panel clips a record at 160 characters, so
                // the instruction goes first and only file names follow.
                let file_name = |p: &Path| {
                    p.file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
                };
                match outcome {
                    LibraryRetarget::Patched => {
                        crate::diagnostics::log_info(format!(
                            "Library {} renamed to {}; the saved project now lists the new name.",
                            file_name(&old_path),
                            file_name(&new_path)
                        ));
                        // Best-effort follow-up commit of the patched
                        // `.snxprj` (queued; the git work runs in a Task).
                        let project_file = self
                            .document_state
                            .projects
                            .iter()
                            .find(|p| p.id == project)
                            .map(|p| p.path.clone());
                        if let Some(project_file) = project_file {
                            self.commit_save_to_project_git(
                                &project_file,
                                &format!(
                                    "Rename library {} to {}",
                                    file_name(&old_path),
                                    file_name(&new_path)
                                ),
                            );
                        }
                    }
                    LibraryRetarget::ListedAsDifferentKind => {
                        // Discard reloads the saved file, which still points
                        // at the old (now missing) file.
                        crate::diagnostics::log_warning(format!(
                            "Saved project lists {} as a shared path, so it still names the old \
                             file. Save to write {}; Discard keeps the old name.",
                            file_name(&old_path),
                            file_name(&new_path)
                        ));
                    }
                    LibraryRetarget::NoProjectFile | LibraryRetarget::NotListed => {
                        crate::diagnostics::log_warning(format!(
                            "Save the project to keep the new path of library {} (renamed to {}).",
                            file_name(&old_path),
                            file_name(&new_path)
                        ));
                    }
                }
                self.refresh_panel_ctx();
            }
            Err(failure) => self.undo_library_file_rename(
                &old_path,
                &new_path,
                &failure.reason,
                failure.stale_project.as_deref(),
            ),
        }
        self.sync_diagnostics_panel_ctx();
        iced::Task::none()
    }

    /// The file moved but the app could not follow. Nothing has been
    /// processed since (same `update`), so moving the file back restores the
    /// exact prior state.
    ///
    /// `stale_project` is the saved `.snxprj` that still lists `new_path`;
    /// it is dirtied only once the file is back at `old_path`, so a save
    /// then writes the (old-path) in-memory entry. If the move-back fails the
    /// file stays at `new_path` and the old-path entry would be wrong.
    fn undo_library_file_rename(
        &mut self,
        old_path: &Path,
        new_path: &Path,
        reason: &str,
        stale_project: Option<&Path>,
    ) {
        match std::fs::rename(new_path, old_path) {
            Ok(()) => {
                if let Some(project) = stale_project {
                    self.document_state.mark_document_dirty(project);
                }
                self.report_library_rename_failure(
                    old_path,
                    &format!("{reason}. The file was renamed back."),
                )
            }
            Err(back) => {
                let detail = format!(
                    "The library is now at {} but the project still lists {}: {back}. \
                     Move it back by hand.",
                    new_path.display(),
                    old_path.display()
                );
                crate::diagnostics::log_error(
                    "Library rename rollback failed",
                    &anyhow::anyhow!("{detail}"),
                );
                self.document_state.error_notice = Some(ErrorNotice::rename_failed(detail));
            }
        }
    }

    /// Why this library cannot be renamed right now, or `None` when it can.
    fn library_rename_refusal(
        &self,
        state: &crate::app::RenameDialogState,
        old_path: &Path,
    ) -> Option<String> {
        if self.project_library_kind(state) != Some(LibraryEntryKind::ProjectLocal) {
            return Some(NOT_PROJECT_LOCAL.to_string());
        }
        let owner_idx = state.tree_path.first().copied();
        let also_listed_by = self
            .document_state
            .projects
            .iter()
            .enumerate()
            .find(|(i, p)| {
                Some(*i) != owner_idx
                    && p.data
                        .libraries
                        .iter()
                        .any(|e| p.data.resolve_library_path(e) == old_path)
            });
        if let Some((_, other)) = also_listed_by {
            return Some(format!(
                "This library is also used by project \"{}\"; renaming it would break that project.",
                other.data.name
            ));
        }
        let registered = self
            .library
            .installed_libraries
            .iter()
            .any(|p| p == old_path)
            || self
                .library
                .global_libraries
                .iter()
                .any(|e| e.path == old_path);
        if registered {
            return Some(
                "This library is also registered globally; renaming it would leave that \
                 registration pointing at a missing file."
                    .to_string(),
            );
        }
        if self.library.pending_mounts.contains_key(old_path) {
            return Some("This library is still opening. Try again in a moment.".to_string());
        }
        None
    }

    fn report_library_rename_failure(&mut self, old_path: &Path, reason: &str) {
        let detail = format!("Could not rename library {}: {reason}", old_path.display());
        // The Messages panel clips a record at 160 characters, so the
        // reason goes first and only the file name follows.
        let name = old_path.file_name().map_or_else(
            || old_path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        crate::diagnostics::log_error(
            "Library rename failed",
            &anyhow::anyhow!("{reason} ({name})"),
        );
        self.document_state.error_notice = Some(ErrorNotice::rename_failed(detail));
    }

    /// Re-point the saved `.snxprj` entry, the mounted library, the project
    /// entry and every open document at `new_path` (the file is already
    /// there). All-or-nothing: an `Err` leaves every in-memory key on
    /// `old_path` and the `.snxprj` on disk as it was.
    fn apply_library_rename(
        &mut self,
        project: crate::app::ProjectId,
        old_path: &Path,
        new_path: &Path,
    ) -> Result<LibraryRetarget, RenameFailure> {
        let plain = |reason: String| RenameFailure {
            reason,
            stale_project: None,
        };
        let new_name = new_path
            .file_name()
            .ok_or_else(|| plain("the new library path has no file name".to_string()))?;
        let project_idx = self
            .document_state
            .projects
            .iter()
            .position(|p| p.id == project)
            .ok_or_else(|| plain("the owning project is no longer open".to_string()))?;
        let (project_path, entry_idxs) = {
            let loaded = &self.document_state.projects[project_idx];
            let entry_idxs: Vec<usize> = loaded
                .data
                .libraries
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    e.kind == LibraryEntryKind::ProjectLocal
                        && loaded.data.resolve_library_path(e) == old_path
                })
                .map(|(i, _)| i)
                .collect();
            if entry_idxs.is_empty() {
                return Err(plain(
                    "the project no longer lists this library".to_string(),
                ));
            }
            (loaded.path.clone(), entry_idxs)
        };

        let outcome = retarget_library_entry(&project_path, old_path, new_name).map_err(|e| {
            plain(format!(
                "the project file could not be updated ({e}); nothing was changed"
            ))
        })?;
        if let Err(e) = self.library.relocate_library(old_path, new_path) {
            let mut reason = format!("the mounted library could not follow the rename: {e}");
            let mut stale_project = None;
            if outcome == LibraryRetarget::Patched
                && let Err(suffix) = self.undo_project_patch(&project_path, old_path, new_path)
            {
                reason.push_str(&suffix);
                stale_project = Some(project_path);
            }
            return Err(RenameFailure {
                reason,
                stale_project,
            });
        }

        for idx in entry_idxs {
            let entry = &mut self.document_state.projects[project_idx].data.libraries[idx];
            entry.path = entry.path.with_file_name(new_name);
        }
        // A saved project was patched on disk and needs nothing more (its
        // other unsaved edits keep their own dirty flag). Without a saved
        // entry to patch, the project is marked dirty so Close Project and
        // quit prompt for the new path.
        if outcome != LibraryRetarget::Patched {
            self.document_state.mark_document_dirty(&project_path);
        }

        self.rekey_library_documents(old_path, new_path);
        Ok(outcome)
    }

    /// Point the saved `.snxprj` entries back at `old_path` after the mount
    /// could not follow. `Err` carries text to append to the failure reason
    /// when the reverse patch fails too: the saved project still lists
    /// `new_path`, and the caller dirties it once the file is back.
    fn undo_project_patch(
        &mut self,
        project_path: &Path,
        old_path: &Path,
        new_path: &Path,
    ) -> Result<(), String> {
        let old_name = old_path.file_name().unwrap_or_default();
        let Err(back) = retarget_library_entry(project_path, new_path, old_name) else {
            return Ok(());
        };
        crate::diagnostics::log_error(
            "Library rename: project file still lists the new path",
            &anyhow::anyhow!(
                "{} lists {}: {back}. Save the project to restore the old path.",
                project_path.display(),
                new_path.display()
            ),
        );
        Err(format!(
            "; the project file {} still lists {}: {back}. Save the project to fix it",
            project_path.display(),
            new_path.display()
        ))
    }

    /// The symbol and footprint drafts open at or under `root`.
    fn library_draft_paths(&self, root: &Path) -> impl Iterator<Item = PathBuf> {
        let d = &self.document_state;
        d.symbol_editors
            .keys()
            .chain(d.footprint_editors.keys())
            .filter(|p| p.starts_with(root))
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// Move every open tab, dirty marker and undocked window that lives at
    /// or under the library's old path.
    fn rekey_library_documents(&mut self, old_path: &Path, new_path: &Path) {
        // Drafts first: `rekey_open_document` moves the editor, its tab (path
        // and kind), its stored dirty mark and its path-keyed records
        // together, so the dirty draft stays dirty at the new path.
        let drafts: Vec<(PathBuf, PathBuf)> = self
            .library_draft_paths(old_path)
            .filter_map(|p| rebase_path(&p, old_path, new_path).map(|to| (p, to)))
            .collect();
        for (from, to) in drafts {
            self.rekey_open_document(&from, &to);
        }
        let new_title = new_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string);
        for tab in self.document_state.tabs.iter_mut() {
            if let Some(moved) = rebase_path(&tab.path, old_path, new_path) {
                let is_library_tab = tab.path == old_path;
                tab.path = moved;
                if let (true, Some(title)) = (is_library_tab, &new_title) {
                    tab.title = title.clone();
                }
            }
            match &mut tab.kind {
                crate::app::TabKind::LibraryBrowser(p) => {
                    if let Some(moved) = rebase_path(p, old_path, new_path) {
                        *p = moved;
                    }
                }
                crate::app::TabKind::ComponentEditor(editor) => {
                    if let Some(moved) = rebase_path(&editor.library_path, old_path, new_path) {
                        editor.library_path = moved;
                    }
                }
                _ => {}
            }
        }
        let moved_dirty: Vec<(PathBuf, PathBuf)> = self
            .document_state
            .dirty_document_paths()
            .into_iter()
            .filter_map(|p| rebase_path(&p, old_path, new_path).map(|m| (p, m)))
            .collect();
        for (from, to) in moved_dirty {
            self.document_state.rekey_document_dirty(&from, &to);
        }
        for kind in self.ui_state.windows.values_mut() {
            if let crate::app::state::WindowKind::UndockedTab { path, .. } = kind
                && let Some(moved) = rebase_path(path, old_path, new_path)
            {
                *path = moved;
            }
        }
    }
}

#[cfg(test)]
mod tests;
