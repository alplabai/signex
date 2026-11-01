//! Rename flows for the project-navigation dock: the dialog, the checks that
//! need only in-memory state, and the start of the move.
//!
//! The `fs::rename` runs on a worker (#859); `rename_apply` re-keys what knew
//! the old path when it reports back. Until then the path is registered in
//! `pending_path_ops`, which refuses saves, opens and second moves of it.

use super::*;
use crate::app::state::scope::CaseFold;

impl Signex {
    pub(super) fn open_rename_dialog(&mut self, tree_path: Vec<usize>) {
        if self.refuse_on_missing_child(&tree_path, "Rename", false) {
            return;
        }
        let Some(target_path) = self.tree_path_to_file_path(&tree_path) else {
            crate::diagnostics::log_warning(
                "Rename cancelled: the selected project-tree row no longer resolves to a file.",
            );
            return;
        };
        let filename = target_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let project_id = self.project_id_at_tree_path(&tree_path);
        self.ui_state.rename_dialog = Some(crate::app::RenameDialogState {
            target_path,
            tree_path,
            project_id,
            buffer: filename,
            error: None,
            is_project_rename: false,
        });
    }

    /// Open the rename modal seeded with the project name (the `.snxprj`
    /// file stem). On submit, [`handle_rename_submit`] sees
    /// `is_project_rename = true` and renames the project (see `project_rename`).
    pub(crate) fn open_project_rename_dialog(&mut self, tree_path: Vec<usize>) {
        let Some(&project_idx) = tree_path.first() else {
            return;
        };
        let Some(project) = self.document_state.projects.get(project_idx) else {
            return;
        };
        let target_path = project.path.clone();
        let stem = target_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        self.ui_state.rename_dialog = Some(crate::app::RenameDialogState {
            target_path,
            tree_path,
            project_id: Some(project.id),
            buffer: stem,
            error: None,
            is_project_rename: true,
        });
    }

    pub(crate) fn handle_rename_submit(&mut self) -> iced::Task<Message> {
        let Some(mut state) = self.ui_state.rename_dialog.clone() else {
            return iced::Task::none();
        };
        if !self.repoint_dialog_tree_path(state.project_id, &mut state.tree_path) {
            crate::diagnostics::log_warning(format!(
                "Rename of {} cancelled: its project was closed while the dialog was open.",
                state.target_path.display()
            ));
            self.set_rename_error("The project was closed while this dialog was open.");
            return iced::Task::none();
        }

        if state.is_project_rename {
            return self.handle_project_rename_submit(&state);
        }

        let new_name = state.buffer.trim();
        if new_name.is_empty() {
            self.set_rename_error("Name cannot be empty.");
            return iced::Task::none();
        }
        // Reject path separators so users can't escape the project dir
        // and so a fs::rename never crosses filesystem boundaries.
        if new_name.contains(['/', '\\']) {
            self.set_rename_error("Name cannot contain path separators.");
            return iced::Task::none();
        }
        // A native file keeps its extension: dropping or changing it
        // would orphan the file from every editor and project reference.
        let old_ext = state.target_path.extension().and_then(|e| e.to_str());
        let new_ext = std::path::Path::new(new_name)
            .extension()
            .and_then(|e| e.to_str());
        if old_ext != new_ext {
            let msg = match old_ext {
                Some(ext) => format!("The file extension must stay \".{ext}\"."),
                None => "This file has no extension; the new name cannot add one.".to_string(),
            };
            self.set_rename_error(&msg);
            return iced::Task::none();
        }
        let parent = match state.target_path.parent() {
            Some(p) => p,
            None => {
                self.set_rename_error("Target file has no parent directory.");
                return iced::Task::none();
            }
        };
        let new_path = parent.join(new_name);
        if new_path == state.target_path {
            // No-op rename — close the dialog silently.
            self.ui_state.rename_dialog = None;
            return iced::Task::none();
        }
        if let Err(error) = self.refuse_while_project_moves(&state.target_path, "rename") {
            self.set_rename_error(&error.to_string());
            return iced::Task::none();
        }
        if let Err(refusal) = self.refuse_rename_overlap(&state.target_path, &new_path) {
            self.set_rename_error(&refusal);
            return iced::Task::none();
        }
        // An open still reading this file would land on the old path after
        // the move, and a save of that tab would recreate it (#1096).
        if let Some(opening) = self
            .pending_open_under(&state.target_path)
            .or_else(|| self.recovery_check_under(&state.target_path))
        {
            crate::diagnostics::log_warning(format!(
                "Rename of {} refused: {} is still opening.",
                state.target_path.display(),
                opening.display()
            ));
            self.set_rename_error("This file is still opening; rename it once it has opened.");
            return iced::Task::none();
        }
        // A restore still running would rewrite the old path after the move,
        // recreating the file the user just renamed.
        if let Some(restoring) = self
            .document_state
            .pending_restores
            .find_under(&state.target_path)
        {
            let label = restoring.file_name().map_or_else(
                || restoring.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
            crate::diagnostics::log_warning(format!(
                "Rename refused: a History restore of {label} is still running."
            ));
            self.set_rename_error(
                "A History restore of this file is still running; rename it once the restore has finished.",
            );
            return iced::Task::none();
        }
        // A project library moves its entry, mount and open documents
        // together once the file rename lands (#739).
        if self.project_library_kind(&state).is_some() {
            return self.begin_library_rename(&state, new_path);
        }

        // Find every parent sheet that references this file BEFORE it
        // moves, while the hierarchy walk can still reach it.
        // Only a schematic sheet can be a child-sheet target; any other
        // file skips the walk so a malformed sheet cannot raise a notice.
        let is_sheet = state.target_path.extension().and_then(|e| e.to_str()) == Some("snxsch");
        let plan = state
            .tree_path
            .first()
            .filter(|_| is_sheet)
            .map(|&idx| self.plan_reference_rewrites(idx, &state.target_path, new_name))
            .unwrap_or_default();

        // Another open project's sheets that place this file are not re-pointed
        // by this rename: refuse rather than leave their symbols dangling.
        {
            let owner = state.project_id.or_else(|| {
                let idx = *state.tree_path.first()?;
                self.document_state.projects.get(idx).map(|p| p.id)
            });
            let users = self.sheet_users(&state.target_path, owner);
            let refusal = if !users.parents.is_empty() {
                let names: Vec<String> = users
                    .parents
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect();
                Some(format!(
                    "Another open project or an open sheet places this sheet and would be left \
                     pointing at the old name: {}",
                    names.join(", ")
                ))
            } else {
                users.unknown.map(|why| {
                    format!(
                        "Whether another project or open sheet places this sheet cannot be ruled \
                         out ({why}). Try again once the project has been read."
                    )
                })
            };
            if let Some(refusal) = refusal {
                crate::diagnostics::log_warning(format!(
                    "Rename of {} refused: {refusal}",
                    state.target_path.display()
                ));
                self.set_rename_error(&refusal);
                return iced::Task::none();
            }
        }

        // A read-only parent (newer Signex) would refuse the re-point after
        // the file moved, breaking the hierarchy: refuse the whole rename
        // while nothing has changed (#1402).
        if !plan.read_only_parents.is_empty() {
            let detail = super::sheet_references::read_only_parents_detail(&plan.read_only_parents);
            crate::diagnostics::log_warning(format!(
                "Rename of {} refused. {detail}",
                state.target_path.display()
            ));
            self.document_state.error_notice = Some(crate::app::state::ErrorNotice::rename(detail));
            self.ui_state.rename_dialog = None;
            return iced::Task::none();
        }

        let new_name = new_name.to_string();
        self.start_file_rename(
            &state,
            new_path,
            new_name,
            crate::app::path_ops::FileRenameKind::Document(plan),
        )
    }

    /// Refuse a rename whose source or destination collides with a move or
    /// delete still running. The text leads with the cause.
    pub(super) fn refuse_rename_overlap(
        &self,
        from: &std::path::Path,
        to: &std::path::Path,
    ) -> Result<(), String> {
        self.refuse_start_while_path_op(from, "renamed")
            .and_then(|()| self.refuse_start_while_path_op(to, "renamed"))
            .map_err(|e| e.to_string())
    }

    /// Register the move and return the task that runs it. Nothing on disk or
    /// in memory has changed yet: the dialog stays open and `rename_apply`
    /// finishes the job.
    fn start_file_rename(
        &mut self,
        state: &crate::app::RenameDialogState,
        new_path: std::path::PathBuf,
        new_name: String,
        kind: crate::app::path_ops::FileRenameKind,
    ) -> iced::Task<Message> {
        use crate::app::path_ops::{FileRenameDone, FileRenameRequest, PathOpKind, RenameIoError};
        let owner = self
            .document_state
            .projects
            .get(state.tree_path.first().copied().unwrap_or(usize::MAX))
            .map(|p| p.id);
        let fold = owner.map_or_else(CaseFold::host_default, |id| {
            self.document_state.fold_for_project(id)
        });
        let from = state.target_path.clone();
        let crate::app::path_ops::FileRenameKind::Document(plan) = &kind;
        let parents_to_read = self.unopened_rewrite_parents(plan);
        // The worker also re-points the owning project's saved `.snxprj`
        // entry (#1821), a read-modify-write like the library rename's: a
        // project save in flight would land afterwards and put the old name
        // back, so refuse before anything moves.
        let project_file = owner.and_then(|id| {
            self.document_state
                .project_by_id(id)
                .map(|p| p.path.clone())
        });
        if let Some(project_file) = &project_file {
            if let Err(refusal) = self
                .refuse_start_while_path_op(project_file, "renamed")
                .map_err(|e| e.to_string())
            {
                self.set_rename_error(&refusal);
                return iced::Task::none();
            }
            if self.document_state.document_saves.is_saving(project_file) {
                self.refuse_rename_while_project_saving();
                return iced::Task::none();
            }
        }
        // The unopened parents the worker reads, and the project file it
        // rewrites, are registered too, so opening or saving one meanwhile is
        // refused instead of being replaced by (or replacing) the rewrite.
        // Parents are rewritten on disk too (#1849), open or not, and a saved
        // reference may exist only on disk, so every candidate page is
        // guarded and scanned.
        let parent_candidates = self.disk_parent_candidates(owner, plan, &from);
        let mut dependents = parent_candidates.clone();
        dependents.extend(project_file.iter().cloned());
        let id = self.document_state.pending_path_ops.begin_with_dependents(
            PathOpKind::Rename,
            vec![from.clone(), new_path.clone()],
            dependents,
            fold,
        );
        // A background read of the old file (project Replace) is out of date.
        self.document_state.note_documents_changed();
        let job = crate::app::path_op_jobs::RenameJob {
            from: from.clone(),
            to: new_path.clone(),
            saves: self.document_state.document_saves.barrier(),
            parents_to_read,
            rewrites: plan.rewrites.clone(),
            parent_candidates,
            fold,
            project: project_file.map(|project_file| crate::app::path_op_jobs::ProjectEntryPatch {
                project_file,
                new_name: new_name.clone(),
                fold,
            }),
        };
        let request = FileRenameRequest {
            id,
            from,
            to: new_path,
            new_name,
            owner,
            kind,
        };
        iced::Task::perform(
            crate::app::path_op_jobs::run_blocking(move || job.run()),
            move |joined| {
                let (result, parents, parent_disk) = joined.unwrap_or_else(|e| {
                    (
                        Err(RenameIoError::Os(e)),
                        Default::default(),
                        Default::default(),
                    )
                });
                Message::Rename(RenameMsg::FileRenamed(Box::new(FileRenameDone {
                    request,
                    result,
                    parents,
                    parent_disk,
                })))
            },
        )
    }

    /// A file at or under `target` whose recovery check has not reported:
    /// renaming it now would lose the copy the check finds (#1323).
    fn recovery_check_under(&self, target: &std::path::Path) -> Option<std::path::PathBuf> {
        use crate::app::state::scope::path_is_under;
        let ds = &self.document_state;
        let owner = ds
            .projects
            .iter()
            .find(|p| path_is_under(target, p.dir(), p.case_fold));
        let fold = crate::app::state::project_case::fold_of(owner);
        ds.recovery_check_due
            .iter()
            .chain(ds.autosave.checking_paths())
            .find(|p| path_is_under(p, target, fold))
            .cloned()
    }

    pub(super) fn set_rename_error(&mut self, msg: &str) {
        if let Some(d) = self.ui_state.rename_dialog.as_mut() {
            d.error = Some(msg.to_string());
        }
    }

    /// Refuse a library rename that would strand an off-thread draft write:
    /// shown in the dialog and logged, naming the draft's file.
    pub(super) fn refuse_rename_while_library_saving(&mut self, draft: &std::path::Path) {
        let label = draft.file_name().map_or_else(
            || draft.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let reason = format!("{label} is still being saved; try again in a moment.");
        self.set_rename_error(&reason);
        crate::diagnostics::log_warning(format!("[rename] refused: {reason}"));
    }

    /// Refuse a rename that would race an off-thread `.snxprj` write: shown
    /// in the dialog and logged to the Messages panel.
    pub(super) fn refuse_rename_while_project_saving(&mut self) {
        const REASON: &str = "The project file is still being saved; try again in a moment.";
        self.set_rename_error(REASON);
        crate::diagnostics::log_warning(format!("[rename] refused: {REASON}"));
    }
}
