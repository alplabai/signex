//! Renaming a project library's `.snxlib` from the project tree (#739), with
//! the file move on a worker (#859). See `docs/architecture/path-ops.md`,
//! "Library rename".

use super::*;
use crate::app::path_op_jobs::{LibraryRenameJob, LibraryRollbackJob, run_blocking};
use crate::app::path_ops::{
    LibraryRenameDone, LibraryRenameIo, LibraryRenameRequest, LibraryRollbackDone, PathOpKind,
};
use crate::app::state::ErrorNotice;
use crate::library::state::rebase_path;
use signex_types::project::{LibraryEntryKind, LibraryRetarget};
use std::path::{Path, PathBuf};

const NOT_PROJECT_LOCAL: &str = "Only libraries stored inside the project can be renamed from the \
     project tree. Shared and global libraries are referenced by path.";

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

    /// Validate a library rename and start the worker that performs it.
    /// Nothing has changed on disk or in memory when this returns; the
    /// dialog stays open until [`Self::handle_library_renamed`] runs.
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
        let Some((project, project_path)) = state
            .tree_path
            .first()
            .and_then(|&idx| self.document_state.projects.get(idx))
            .map(|p| (p.id, p.path.clone()))
        else {
            self.set_rename_error("The owning project is no longer open.");
            return iced::Task::none();
        };
        // The worker reads, patches and rewrites the owning `.snxprj`, so it
        // is registered too: a project save, a project rename or a reopen of
        // the project in the window would race that read-modify-write.
        if let Err(refusal) = self
            .refuse_start_while_path_op(&project_path, "renamed")
            .map_err(|e| e.to_string())
        {
            self.set_rename_error(&refusal);
            return iced::Task::none();
        }
        // The worker rewrites the `.snxprj` directly. An off-thread write of
        // it still in flight would land afterwards and put the old library
        // path back; it runs only after `update` returns, so refuse (before
        // anything moves) rather than wait.
        if self.document_state.document_saves.is_saving(&project_path) {
            self.refuse_rename_while_project_saving();
            return iced::Task::none();
        }
        // The same holds for a draft inside the library: its write names the
        // old path and its completion would find no editor behind it.
        let fold = self.document_state.fold_for_project(project);
        let saving = self
            .library_draft_paths(&old_path, fold)
            .find(|draft| self.document_state.document_saves.is_saving(draft));
        if let Some(saving) = saving {
            self.refuse_rename_while_library_saving(&saving);
            return iced::Task::none();
        }
        // A "Commit now" or a stock copy writes into the library on a worker
        // through its old path; moving the folder would strand that work.
        if self.library_background_write_running(&old_path) {
            self.refuse_rename_while_library_saving(&old_path);
            return iced::Task::none();
        }
        let id = self.document_state.pending_path_ops.begin_with_dependents(
            PathOpKind::RenameLibrary,
            vec![old_path.clone(), new_path.clone()],
            vec![project_path.clone()],
            fold,
        );
        self.document_state.note_documents_changed();
        let job = LibraryRenameJob {
            old: old_path.clone(),
            new: new_path.clone(),
            project_path: project_path.clone(),
            saves: self.document_state.document_saves.barrier(),
        };
        let request = LibraryRenameRequest {
            id,
            old: old_path,
            new: new_path,
            project,
            project_path,
        };
        iced::Task::perform(run_blocking(move || job.run()), move |joined| {
            let io = joined.unwrap_or_else(LibraryRenameIo::NotRenamed);
            Message::Rename(RenameMsg::LibraryRenamed(Box::new(LibraryRenameDone {
                request,
                io,
            })))
        })
    }

    /// The library worker finished.
    pub(crate) fn handle_library_renamed(
        &mut self,
        done: LibraryRenameDone,
    ) -> iced::Task<Message> {
        let LibraryRenameDone { request, io } = done;
        // A completion already handled finds its operation gone. Handling it
        // again would fail the re-point and roll the file back while memory
        // already names the new path.
        if !self
            .document_state
            .pending_path_ops
            .is_registered(request.id)
        {
            crate::diagnostics::log_warning(
                "A library rename completion arrived twice; the second copy was ignored.",
            );
            return iced::Task::none();
        }
        // The worker may have moved the library and rewritten the `.snxprj`.
        self.document_state.note_document_written_behind_engines();
        let (old, new) = (request.old.clone(), request.new.clone());
        let outcome = match io {
            LibraryRenameIo::Moved(outcome) => outcome,
            LibraryRenameIo::TargetExists => {
                self.document_state.pending_path_ops.finish(request.id);
                self.set_library_dialog_error(&old, "A file with that name already exists.");
                return iced::Task::none();
            }
            LibraryRenameIo::NotRenamed(e) => {
                self.document_state.pending_path_ops.finish(request.id);
                self.report_library_rename_failure(
                    &old,
                    &format!("the file could not be renamed: {e}"),
                );
                self.set_library_dialog_error(&old, &format!("Rename failed: {e}"));
                self.sync_diagnostics_panel_ctx();
                return iced::Task::none();
            }
            LibraryRenameIo::ProjectNotPatched { reason, moved_back } => {
                self.document_state.pending_path_ops.finish(request.id);
                self.close_library_dialog(&old);
                self.report_library_rollback(&old, &new, &reason, moved_back, None);
                self.sync_diagnostics_panel_ctx();
                return iced::Task::none();
            }
        };
        self.close_library_dialog(&old);
        match self.apply_library_rename(request.project, &old, &new, outcome) {
            Ok(()) => {
                self.document_state.pending_path_ops.finish(request.id);
                self.report_library_renamed(&request, outcome);
                self.refresh_panel_ctx();
                self.sync_diagnostics_panel_ctx();
                iced::Task::none()
            }
            Err(reason) => self.start_library_rollback(request, reason, outcome),
        }
    }

    fn close_library_dialog(&mut self, old: &Path) {
        if self
            .ui_state
            .rename_dialog
            .as_ref()
            .is_some_and(|d| d.target_path == old)
        {
            self.ui_state.rename_dialog = None;
        }
    }

    fn set_library_dialog_error(&mut self, old: &Path, message: &str) {
        let open_for_library = self
            .ui_state
            .rename_dialog
            .as_ref()
            .is_some_and(|d| d.target_path == old);
        if open_for_library {
            self.set_rename_error(message);
        }
    }

    /// The Messages-panel lines and follow-up commit for a rename the app
    /// followed. The panel clips a record at 160 characters, so the
    /// instruction goes first and only file names follow.
    fn report_library_renamed(&mut self, request: &LibraryRenameRequest, outcome: LibraryRetarget) {
        let file_name = |p: &Path| {
            p.file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
        };
        let (old, new) = (&request.old, &request.new);
        match outcome {
            LibraryRetarget::Patched => {
                crate::diagnostics::log_info(format!(
                    "Library {} renamed to {}; the saved project now lists the new name.",
                    file_name(old),
                    file_name(new)
                ));
                // Best-effort follow-up commit of the patched `.snxprj`
                // (queued; the git work runs in a Task).
                self.commit_save_to_project_git(
                    &request.project_path,
                    &format!("Rename library {} to {}", file_name(old), file_name(new)),
                );
            }
            LibraryRetarget::ListedAsDifferentKind => {
                // Discard reloads the saved file, which still points at the
                // old (now missing) file.
                crate::diagnostics::log_warning(format!(
                    "Saved project lists {} as a shared path, so it still names the old \
                     file. Save to write {}; Discard keeps the old name.",
                    file_name(old),
                    file_name(new)
                ));
            }
            LibraryRetarget::NoProjectFile | LibraryRetarget::NotListed => {
                crate::diagnostics::log_warning(format!(
                    "Save the project to keep the new path of library {} (renamed to {}).",
                    file_name(old),
                    file_name(new)
                ));
            }
        }
    }

    /// The file moved (and the project file may be patched), but the app
    /// could not follow. Start the worker that undoes both; until it reports
    /// back the path stays registered so nothing writes to either name.
    fn start_library_rollback(
        &mut self,
        request: LibraryRenameRequest,
        reason: String,
        outcome: LibraryRetarget,
    ) -> iced::Task<Message> {
        crate::diagnostics::log_warning(format!(
            "Library rename cancelled: {reason}. Moving {} back.",
            request
                .old
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
        ));
        let patched = outcome == LibraryRetarget::Patched;
        let job = LibraryRollbackJob {
            old: request.old.clone(),
            new: request.new.clone(),
            project_path: request.project_path.clone(),
            patched,
        };
        iced::Task::perform(run_blocking(move || job.run()), move |joined| {
            let (patch_undo, moved_back) = joined.unwrap_or_else(|e| (Ok(()), Err(e)));
            Message::Rename(RenameMsg::LibraryRenameRolledBack(Box::new(
                LibraryRollbackDone {
                    request,
                    reason,
                    patch_undo,
                    moved_back,
                },
            )))
        })
    }

    /// The rollback worker finished.
    pub(crate) fn handle_library_rename_rolled_back(
        &mut self,
        done: LibraryRollbackDone,
    ) -> iced::Task<Message> {
        let LibraryRollbackDone {
            request,
            reason,
            patch_undo,
            moved_back,
        } = done;
        self.document_state.pending_path_ops.finish(request.id);
        // The worker moved the library back and un-patched the project file.
        self.document_state.note_document_written_behind_engines();
        let stale = patch_undo
            .err()
            .map(|suffix| (request.project_path.clone(), suffix));
        self.report_library_rollback(&request.old, &request.new, &reason, moved_back, stale);
        self.sync_diagnostics_panel_ctx();
        iced::Task::none()
    }

    /// Report the end of a rolled-back rename. `stale` is the saved project
    /// that still lists the new path, with the failure text to append; it is
    /// dirtied only once the file is back at `old`, so a save then writes the
    /// (old-path) in-memory entry. If the move-back failed the file stays at
    /// `new` and the old-path entry would be wrong.
    fn report_library_rollback(
        &mut self,
        old: &Path,
        new: &Path,
        reason: &str,
        moved_back: Result<(), String>,
        stale: Option<(PathBuf, String)>,
    ) {
        let mut reason = reason.to_string();
        let mut stale_project = None;
        if let Some((project_path, suffix)) = stale {
            crate::diagnostics::log_error(
                "Library rename: project file still lists the new path",
                &anyhow::anyhow!("{}{suffix}", project_path.display()),
            );
            reason.push_str(&suffix);
            stale_project = Some(project_path);
        }
        match moved_back {
            Ok(()) => {
                let project_is_open =
                    |path: &Path| self.document_state.projects.iter().any(|p| p.path == path);
                if let Some(project) = stale_project.filter(|p| project_is_open(p)) {
                    self.document_state.mark_document_dirty(&project);
                }
                self.report_library_rename_failure(
                    old,
                    &format!("{reason}. The file was renamed back."),
                );
            }
            Err(back) => {
                let detail = format!(
                    "The library is now at {} but the project still lists {}: {back}. \
                     Move it back by hand.",
                    new.display(),
                    old.display()
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

    /// Re-point the mounted library, the project entry and every open
    /// document at `new_path` (the file is already there and the saved
    /// `.snxprj` already patched by the worker). All-or-nothing: an `Err`
    /// leaves every in-memory key on `old_path`; the caller then undoes the
    /// disk side.
    fn apply_library_rename(
        &mut self,
        project: crate::app::ProjectId,
        old_path: &Path,
        new_path: &Path,
        outcome: LibraryRetarget,
    ) -> Result<(), String> {
        let new_name = new_path
            .file_name()
            .ok_or_else(|| "the new library path has no file name".to_string())?;
        let project_idx = self
            .document_state
            .projects
            .iter()
            .position(|p| p.id == project)
            .ok_or_else(|| "the owning project was closed while the library moved".to_string())?;
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
                return Err("the project no longer lists this library".to_string());
            }
            (loaded.path.clone(), entry_idxs)
        };

        self.library
            .relocate_library(old_path, new_path)
            .map_err(|e| format!("the mounted library could not follow the rename: {e}"))?;

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

        let fold = self.document_state.fold_for_project(project);
        self.rekey_library_documents(old_path, new_path, fold);
        Ok(())
    }

    /// The symbol and footprint drafts open at or under `root`.
    fn library_draft_paths(
        &self,
        root: &Path,
        fold: crate::app::state::CaseFold,
    ) -> impl Iterator<Item = PathBuf> {
        let d = &self.document_state;
        d.symbol_editors
            .keys()
            .chain(d.footprint_editors.keys())
            .filter(|p| crate::app::state::scope::path_is_under(p, root, fold))
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// Move every open tab, dirty marker and undocked window that lives at
    /// or under the library's old path.
    fn rekey_library_documents(
        &mut self,
        old_path: &Path,
        new_path: &Path,
        fold: crate::app::state::CaseFold,
    ) {
        // Drafts first: `rekey_open_document` moves the editor, its tab (path
        // and kind), its stored dirty mark and its path-keyed records
        // together, so the dirty draft stays dirty at the new path.
        let drafts: Vec<(PathBuf, PathBuf)> = self
            .library_draft_paths(old_path, fold)
            .map(|p| {
                let to = crate::app::state::scope::rebase_under(&p, old_path, new_path);
                (p, to)
            })
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
