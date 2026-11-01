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
//! - Parent sheets that reference the sheet are re-pointed on disk by the same
//!   job (#1849): each saved parent is re-read and only its child-sheet
//!   filename changes, so a parent's other unsaved edits are never written and
//!   Discard All leaves project and parents naming the same file. Open parents
//!   follow through the engine; one with no other unsaved edits stays clean.
//!   When the project file could not take the new name the parents are not
//!   written: the open ones re-point in memory (dirty) and the project is saved
//!   with them; Messages names what is left.
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
use crate::app::parent_reference_disk::patch_parents;
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
        if !was_listed {
            self.repoint_parents_of_save_as(idx, from, to);
            return Task::none();
        }
        let leaf = file_label(to);
        let plan = self.plan_reference_rewrites(idx, from, &leaf);
        self.begin_project_entry_patch(project_id, project_file, from, to, plan)
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
        plan: crate::app::path_ops::ReferencePlan,
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
            self.repoint_parents_in_memory(project_id, from, to);
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
        // The saved parents that reference the sheet are rewritten by the
        // same job (#1849), so they are guarded too; one a move or delete is
        // still working on is left to the in-memory fallback.
        let mut disk_plan = plan.clone();
        disk_plan.rewrites.retain(|rewrite| {
            self.document_state
                .pending_path_ops
                .blocking_start(&rewrite.parent)
                .is_none()
        });
        let mut dependents: Vec<std::path::PathBuf> = disk_plan
            .rewrites
            .iter()
            .map(|rewrite| rewrite.parent.clone())
            .collect();
        dependents.sort();
        dependents.dedup();
        dependents.push(project_file.clone());
        self.document_state.pending_path_ops.begin_with_dependents(
            crate::app::path_ops::PathOpKind::SaveAsSheet,
            vec![to.to_path_buf()],
            dependents,
            fold,
        );
        let saves = self.document_state.document_saves.barrier();
        let (from_path, to_path) = (from.to_path_buf(), to.to_path_buf());
        let new_name = to
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        Task::perform(
            {
                let from = from_path.clone();
                let rewrites = disk_plan.rewrites;
                async move {
                    crate::app::handlers::dock::project_navigation::add_io::on_blocking_pool(
                        move || {
                            // A parent save still running would land after the
                            // rewrite and put the old name back.
                            saves.wait();
                            let patched =
                                retarget_sheet_entry(&project_file, &from, &new_name, fold)
                                    .map_err(|e| e.to_string());
                            // Parents follow only a project that now lists the
                            // new file: the pair stays consistent on disk.
                            let disk = if matches!(patched, Ok(EntryPatch::Patched(_))) {
                                patch_parents(&rewrites)
                            } else {
                                Default::default()
                            };
                            Ok((patched, disk))
                        },
                    )
                    .await
                }
            },
            move |joined| {
                let (result, parent_disk) = match joined {
                    Ok((result, disk)) => (result, disk),
                    Err(e) => (Err(e), Default::default()),
                };
                Message::File(FileMsg::SaveAsProjectPatched(Box::new(
                    SaveAsProjectPatched {
                        project_id,
                        from: from_path.clone(),
                        to: to_path.clone(),
                        result,
                        plan: plan.clone(),
                        parent_disk,
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
            plan,
            parent_disk,
        } = done;
        self.document_state
            .pending_path_ops
            .finish_save_as_sheet(&to);
        let name = file_label(&to);
        match result {
            Ok(EntryPatch::Patched(bytes)) => {
                self.repoint_in_memory(project_id, &from, &to, false);
                self.follow_save_as_parents_and_report(&from, &plan, &parent_disk);
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
                self.repoint_parents_in_memory(project_id, &from, &to);
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
                self.repoint_parents_in_memory(project_id, &from, &to);
                self.repoint_in_memory(project_id, &from, &to, true);
                crate::diagnostics::log_warning(format!(
                    "[save-as] The project has not been saved yet; save it to list {name}"
                ));
            }
            Ok(EntryPatch::Unreadable(reason)) => {
                self.repoint_parents_in_memory(project_id, &from, &to);
                self.repoint_in_memory(project_id, &from, &to, true);
                crate::diagnostics::log_warning(format!(
                    "[save-as] The saved project file could not be read ({reason}); save the \
                     project to list {name}"
                ));
            }
            Err(reason) => {
                self.repoint_parents_in_memory(project_id, &from, &to);
                self.repoint_in_memory(project_id, &from, &to, true);
                self.report_save_failure(
                    "Save As could not update the project file",
                    &anyhow::anyhow!("{reason}; the project file was not changed"),
                );
            }
        }
        self.refresh_panel_ctx();
    }

    /// The project file did not take the new name, so the parents' saved
    /// references stay on the old file: the open ones follow in memory (dirty)
    /// and are saved with the project.
    fn repoint_parents_in_memory(&mut self, id: ProjectId, from: &Path, to: &Path) {
        if let Some(idx) = self.document_state.projects.iter().position(|p| p.id == id) {
            self.repoint_parents_of_save_as(idx, from, to);
        }
    }

    /// Follow the parents the worker re-pointed on disk and say what was left.
    fn follow_save_as_parents_and_report(
        &mut self,
        from: &Path,
        plan: &crate::app::path_ops::ReferencePlan,
        disk: &crate::app::parent_reference_disk::ParentDiskOutcomes,
    ) {
        let left = self.follow_save_as_parents(plan, disk);
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
