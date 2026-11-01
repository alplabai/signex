//! What a schematic Save As does to the project that listed the sheet (#1824).
//!
//! Save As moves the tab and engine to the new file and leaves the old file
//! on disk untouched. Without this step the project would keep listing the
//! old file, so reopening it would show the unedited sheet and the edit would
//! live in a file the project does not know about.
//!
//! The rule mirrors Rename (`dock/project_navigation/rename_apply.rs`):
//!
//! - A target in the same folder as the old file takes its place in the page
//!   list and as `schematic_root`. The saved `.snxprj` is patched on a worker
//!   (re-read, one entry changed, written atomically), so other unsaved
//!   project edits held in memory are never written and Discard All still
//!   reverts them. The project file is registered in `pending_path_ops` for
//!   the job, so a project save or rename cannot interleave.
//! - Open parent sheets re-point their child-sheet references through the
//!   engine (dirty, undoable). Sheets that are not open are not rewritten;
//!   Messages names them.
//! - A target in another folder is not re-pointed: child-sheet filenames are
//!   relative to the sheet's own folder and are not rebased, so the entry
//!   would not resolve on reopen. Messages says the copy is not part of the
//!   project. The same goes for a target outside the project folder.
//! - The old file is never touched.

use std::path::Path;

use iced::Task;

use super::super::super::*;
use super::save::file_label;
use crate::app::contracts::SaveAsProjectPatched;
use crate::app::project_entry_disk::{EntryPatch, repoint_entries, retarget_sheet_entry};
use crate::app::state::ProjectId;
use crate::app::state::library_entry::{entry_resolves_to, library_entry_location};
use crate::app::state::scope::path_key_with;

impl Signex {
    /// The schematic `from` was saved as `to`. `owner` is the project that
    /// held `from` before the move. Returns the job that patches the saved
    /// project file.
    pub(super) fn follow_save_as_in_project(
        &mut self,
        owner: Option<ProjectId>,
        from: &Path,
        to: &Path,
    ) -> Task<Message> {
        let Some(project_id) = owner else {
            return Task::none();
        };
        let Some(idx) = self
            .document_state
            .projects
            .iter()
            .position(|p| p.id == project_id)
        else {
            return Task::none();
        };
        let project = &self.document_state.projects[idx];
        let (dir, fold, project_file) = (
            project.dir().to_path_buf(),
            project.case_fold,
            project.path.clone(),
        );
        let (_, location) = library_entry_location(&dir, to, fold);
        let same_folder = from
            .parent()
            .zip(to.parent())
            .is_some_and(|(a, b)| path_key_with(a, fold) == path_key_with(b, fold));
        if location != signex_types::project::LibraryEntryKind::ProjectLocal || !same_folder {
            crate::diagnostics::log_warning(format!(
                "[save-as] {} is not in the folder of {}, so the project was not re-pointed: \
                 it is a copy that is not part of the project, which still lists {}",
                file_label(to),
                file_label(from),
                file_label(from)
            ));
            return Task::none();
        }
        let listed = |entry: &str, target: &Path| entry_resolves_to(&dir, entry, target, fold);
        let sheets = &self.document_state.projects[idx].data.sheets;
        if sheets.iter().any(|s| listed(&s.filename, to)) {
            crate::diagnostics::log_warning(format!(
                "[save-as] The project already lists {}, so it was not re-pointed from {}",
                file_label(to),
                file_label(from)
            ));
            return Task::none();
        }
        let was_listed = sheets.iter().any(|s| listed(&s.filename, from));
        self.repoint_parents_of_save_as(idx, from, to);
        if !was_listed {
            return Task::none();
        }
        self.begin_project_entry_patch(project_id, project_file, from, to)
    }

    /// Re-point the open parents' references and say what was left behind.
    fn repoint_parents_of_save_as(&mut self, idx: usize, from: &Path, to: &Path) {
        let left = self.repoint_open_parents_after_save_as(idx, from, to);
        let mut notes = left.not_rewritten;
        notes.extend(left.unchecked);
        if !notes.is_empty() {
            crate::diagnostics::log_warning(format!(
                "[save-as] Sheets that may still reference {}: {}",
                file_label(from),
                notes.join("; ")
            ));
        }
    }

    /// Patch the saved project file on a worker, or, when the file is busy,
    /// leave the change in memory for the user to save.
    fn begin_project_entry_patch(
        &mut self,
        project_id: ProjectId,
        project_file: std::path::PathBuf,
        from: &Path,
        to: &Path,
    ) -> Task<Message> {
        let fold = self
            .document_state
            .project_by_id(project_id)
            .map_or_else(Default::default, |p| p.case_fold);
        let busy = self.document_state.document_saves.is_saving(&project_file)
            || self
                .document_state
                .pending_path_ops
                .blocking_start(&project_file)
                .is_some();
        if busy {
            self.repoint_in_memory(project_id, from, to, true);
            crate::diagnostics::log_warning(format!(
                "[save-as] The project file is busy; save the project to list {}",
                file_label(to)
            ));
            return Task::none();
        }
        // The job reads, patches and rewrites the saved `.snxprj`, so the file
        // is registered for the whole run: a project save, a rename or an Add
        // New started meanwhile is refused. Released by the completion.
        self.document_state.pending_path_ops.begin_with_dependents(
            crate::app::path_ops::PathOpKind::SaveAsSheet,
            vec![to.to_path_buf()],
            vec![project_file.clone()],
            fold,
        );
        let (from_path, to_path) = (from.to_path_buf(), to.to_path_buf());
        let new_name = to
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        Task::perform(
            {
                let from = from_path.clone();
                async move {
                    crate::app::handlers::dock::project_navigation::add_io::on_blocking_pool(
                        move || {
                            retarget_sheet_entry(&project_file, &from, &new_name, fold)
                                .map_err(|e| e.to_string())
                        },
                    )
                    .await
                }
            },
            move |result| {
                Message::File(FileMsg::SaveAsProjectPatched(Box::new(
                    SaveAsProjectPatched {
                        project_id,
                        from: from_path.clone(),
                        to: to_path.clone(),
                        result,
                    },
                )))
            },
        )
    }

    /// The patch of the saved project file finished. The in-memory entries
    /// follow; the project is marked dirty only when the file on disk could
    /// not be updated, so edits the user did not save stay unsaved.
    pub(crate) fn handle_save_as_project_patched(&mut self, done: SaveAsProjectPatched) {
        let SaveAsProjectPatched {
            project_id,
            from,
            to,
            result,
        } = done;
        self.document_state
            .pending_path_ops
            .finish_save_as_sheet(&to);
        let name = file_label(&to);
        match result {
            Ok(EntryPatch::Patched(bytes)) => {
                self.repoint_in_memory(project_id, &from, &to, false);
                crate::diagnostics::log_info(format!(
                    "[save-as] The saved project file now lists {name}"
                ));
                let project_file = self
                    .document_state
                    .project_by_id(project_id)
                    .map(|p| p.path.clone());
                if let Some(project_file) = project_file {
                    self.commit_bytes_to_project_git(
                        &project_file,
                        &format!("Save sheet as {name}"),
                        crate::app::state::CommitBytes { bytes, sequence: 0 },
                    );
                }
            }
            Ok(EntryPatch::NotListed) => {
                // The saved project does not list the sheet, so the in-memory
                // project is the only one that does: it needs a save.
                self.repoint_in_memory(project_id, &from, &to, true);
                crate::diagnostics::log_warning(format!(
                    "[save-as] The saved project file does not list {}; save the project to \
                     list {name}",
                    file_label(&from)
                ));
            }
            Ok(EntryPatch::NoProjectFile) => {
                self.repoint_in_memory(project_id, &from, &to, true);
                crate::diagnostics::log_warning(format!(
                    "[save-as] The project has not been saved yet; save it to list {name}"
                ));
            }
            Ok(EntryPatch::Unreadable(reason)) => {
                self.repoint_in_memory(project_id, &from, &to, true);
                crate::diagnostics::log_warning(format!(
                    "[save-as] The saved project file could not be read ({reason}); save the \
                     project to list {name}"
                ));
            }
            Err(reason) => {
                self.repoint_in_memory(project_id, &from, &to, true);
                self.report_save_failure(
                    "Save As could not update the project file",
                    &anyhow::anyhow!("{reason}; the project file was not changed"),
                );
            }
        }
        self.refresh_panel_ctx();
    }

    /// Point the loaded project's entries at the new file name.
    fn repoint_in_memory(&mut self, id: ProjectId, from: &Path, to: &Path, dirty: bool) {
        let Some(loaded) = self.document_state.projects.iter_mut().find(|p| p.id == id) else {
            return;
        };
        let new_name = to
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let (dir, fold) = (loaded.dir().to_path_buf(), loaded.case_fold);
        let changed = repoint_entries(&mut loaded.data, &dir, from, &new_name, fold);
        let project_file = loaded.path.clone();
        if changed && dirty {
            self.document_state.mark_document_dirty(&project_file);
        }
    }
}
