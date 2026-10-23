//! Renaming a project library's `.snxlib` from the project tree (#739).
//!
//! The file rename runs in a `Task`; when it lands, everything that knows
//! the library by its path moves with it in one step: the project's
//! `LibraryEntry`, the persisted `.snxprj`, the mounted adapter, and every
//! open tab, editor map and dirty marker. If any step fails the project is
//! left on the old path and the file is renamed back, so the project never
//! points at a missing library.
//!
//! Only project-local libraries are renameable here. Shared and Global
//! libraries live outside the project and stay refused (#657).

use super::*;
use crate::app::state::ErrorNotice;
use crate::library::state::rebase_path;
use signex_types::project::LibraryEntryKind;
use std::path::{Path, PathBuf};

const NOT_PROJECT_LOCAL: &str = "Only libraries stored inside the project can be renamed from the \
     project tree. Shared and global libraries are referenced by path.";

/// `fs::rename` off the UI thread.
async fn rename_on_disk(from: PathBuf, to: PathBuf) -> Result<(), String> {
    tokio::task::spawn_blocking(move || std::fs::rename(&from, &to).map_err(|e| e.to_string()))
        .await
        .unwrap_or_else(|e| Err(format!("rename task failed: {e}")))
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

    /// Validate a library rename and start it. Nothing changes until the
    /// `fs::rename` task reports back through
    /// [`handle_library_file_renamed`](Self::handle_library_file_renamed).
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
        let Some(project) = state
            .tree_path
            .first()
            .and_then(|&idx| self.document_state.projects.get(idx))
            .map(|p| p.id)
        else {
            self.set_rename_error("The owning project is no longer open.");
            return iced::Task::none();
        };
        self.ui_state.rename_dialog = None;
        let (from, to) = (old_path.clone(), new_path.clone());
        iced::Task::perform(rename_on_disk(from, to), move |result| {
            Message::Rename(RenameMsg::LibraryFileRenamed {
                project,
                old_path: old_path.clone(),
                new_path: new_path.clone(),
                result,
            })
        })
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

    /// The `fs::rename` task finished.
    pub(crate) fn handle_library_file_renamed(
        &mut self,
        project: crate::app::ProjectId,
        old_path: &Path,
        new_path: &Path,
        result: Result<(), String>,
    ) -> iced::Task<Message> {
        let task = self.finish_library_file_renamed(project, old_path, new_path, result);
        // The Rename dispatcher does not run `finish_update`; republish so
        // the outcome is in the Messages panel when this message is done.
        self.sync_diagnostics_panel_ctx();
        task
    }

    fn finish_library_file_renamed(
        &mut self,
        project: crate::app::ProjectId,
        old_path: &Path,
        new_path: &Path,
        result: Result<(), String>,
    ) -> iced::Task<Message> {
        if let Err(reason) = result {
            self.report_library_rename_failure(
                old_path,
                &format!("the file could not be renamed: {reason}"),
            );
            return iced::Task::none();
        }
        match self.apply_library_rename(project, old_path, new_path) {
            Ok(()) => {
                crate::diagnostics::log_info(format!(
                    "Renamed library {} to {}",
                    old_path.display(),
                    new_path.display()
                ));
                self.refresh_panel_ctx();
                iced::Task::none()
            }
            Err(reason) => {
                self.report_library_rename_failure(
                    old_path,
                    &format!("{reason}. The file is being renamed back."),
                );
                let (from, to) = (new_path.to_path_buf(), old_path.to_path_buf());
                let (old, new) = (old_path.to_path_buf(), new_path.to_path_buf());
                iced::Task::perform(rename_on_disk(from, to), move |result| {
                    Message::Rename(RenameMsg::LibraryRenameRolledBack {
                        old_path: old.clone(),
                        new_path: new.clone(),
                        result,
                    })
                })
            }
        }
    }

    /// The rollback `fs::rename` finished: say whether the library is back
    /// where the project expects it.
    pub(crate) fn handle_library_rename_rolled_back(
        &mut self,
        old_path: &Path,
        new_path: &Path,
        result: Result<(), String>,
    ) {
        match result {
            Ok(()) => crate::diagnostics::log_info(format!(
                "Library rename rolled back; {} is back at {}",
                new_path.display(),
                old_path.display()
            )),
            Err(reason) => {
                let detail = format!(
                    "The library is now at {} but the project still lists {}: {reason}. \
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
        self.sync_diagnostics_panel_ctx();
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

    /// Re-point the project entry, the mounted library and every open
    /// document at `new_path`, persisting the project. All-or-nothing: an
    /// `Err` leaves every in-memory key on `old_path`.
    fn apply_library_rename(
        &mut self,
        project: crate::app::ProjectId,
        old_path: &Path,
        new_path: &Path,
    ) -> Result<(), String> {
        let new_name = new_path
            .file_name()
            .ok_or_else(|| "the new library path has no file name".to_string())?;
        let project_idx = self
            .document_state
            .projects
            .iter()
            .position(|p| p.id == project)
            .ok_or_else(|| "the owning project is no longer open".to_string())?;
        let (project_path, entry_idx) = {
            let loaded = &self.document_state.projects[project_idx];
            let entry_idx = loaded
                .data
                .libraries
                .iter()
                .position(|e| {
                    e.kind == LibraryEntryKind::ProjectLocal
                        && loaded.data.resolve_library_path(e) == old_path
                })
                .ok_or_else(|| "the project no longer lists this library".to_string())?;
            (loaded.path.clone(), entry_idx)
        };

        self.library
            .relocate_library(old_path, new_path)
            .map_err(|e| format!("the mounted library could not follow the rename: {e}"))?;

        let old_entry_path = {
            let entry = &mut self.document_state.projects[project_idx].data.libraries[entry_idx];
            let old_entry_path = entry.path.clone();
            entry.path = old_entry_path.with_file_name(new_name);
            old_entry_path
        };
        let was_dirty = self.document_state.dirty_paths.contains(&project_path);
        self.document_state.dirty_paths.insert(project_path.clone());
        if let Err(error) = self.save_project_at_path(&project_path) {
            self.document_state.projects[project_idx].data.libraries[entry_idx].path =
                old_entry_path;
            if !was_dirty {
                self.document_state.dirty_paths.remove(&project_path);
            }
            if let Err(back) = self.library.relocate_library(new_path, old_path) {
                crate::diagnostics::log_error(
                    "Library rename revert failed",
                    &anyhow::anyhow!(back),
                );
            }
            return Err(format!("the project file could not be saved: {error:#}"));
        }

        self.rekey_library_documents(old_path, new_path);
        Ok(())
    }

    /// Move every open tab, dirty marker and undocked window that lives at
    /// or under the library's old path.
    fn rekey_library_documents(&mut self, old_path: &Path, new_path: &Path) {
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
            .dirty_paths
            .iter()
            .filter_map(|p| rebase_path(p, old_path, new_path).map(|m| (p.clone(), m)))
            .collect();
        for (from, to) in moved_dirty {
            self.document_state.dirty_paths.remove(&from);
            self.document_state.dirty_paths.insert(to);
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
