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
        let Some(target_path) = self.tree_path_to_file_path(&tree_path) else {
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
    /// `is_project_rename = true` and renames the trio
    /// `<old>.snxprj` / `<old>.snxsch` / `<old>.snxpcb` together.
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
        if let Err(refusal) = self.refuse_rename_overlap(&state.target_path, &new_path) {
            self.set_rename_error(&refusal);
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
        let parents_to_read = match &kind {
            crate::app::path_ops::FileRenameKind::Document(plan) => {
                self.unopened_rewrite_parents(plan)
            }
            crate::app::path_ops::FileRenameKind::Project => Vec::new(),
        };
        // The unopened parents the worker reads are registered too, so
        // opening one meanwhile is refused instead of being replaced by the
        // rewritten copy (or replacing it).
        let id = self.document_state.pending_path_ops.begin_with_dependents(
            PathOpKind::Rename,
            vec![from.clone(), new_path.clone()],
            parents_to_read.clone(),
            fold,
        );
        let job = crate::app::path_op_jobs::RenameJob {
            from: from.clone(),
            to: new_path.clone(),
            saves: self.document_state.document_saves.barrier(),
            parents_to_read,
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
                let (result, parents) =
                    joined.unwrap_or_else(|e| (Err(RenameIoError::Os(e)), Default::default()));
                Message::Rename(RenameMsg::FileRenamed(Box::new(FileRenameDone {
                    request,
                    result,
                    parents,
                })))
            },
        )
    }

    pub(super) fn set_rename_error(&mut self, msg: &str) {
        if let Some(d) = self.ui_state.rename_dialog.as_mut() {
            d.error = Some(msg.to_string());
        }
    }

    /// Project-root rename: `state.target_path` is the project's
    /// `.snxprj`; the buffer is the new *stem*. We rename ONLY the
    /// `.snxprj` file and update the project's in-memory `name`.
    /// Companion schematic / pcb files keep their existing filenames:
    /// they're independent entities, possibly shared across workflows
    /// or referenced from version control with their original names.
    /// The `.snxprj`'s `schematic_root` / `pcb_file` are filename
    /// strings that continue to point at the unchanged sheet files.
    fn handle_project_rename_submit(
        &mut self,
        state: &crate::app::RenameDialogState,
    ) -> iced::Task<Message> {
        let new_stem = state.buffer.trim();
        if new_stem.is_empty() {
            self.set_rename_error("Name cannot be empty.");
            return iced::Task::none();
        }
        if new_stem.contains(['/', '\\', '.']) {
            self.set_rename_error("Name cannot contain '/', '\\', or '.'.");
            return iced::Task::none();
        }
        let dir = match state.target_path.parent() {
            Some(d) => d.to_path_buf(),
            None => {
                self.set_rename_error("Project file has no parent directory.");
                return iced::Task::none();
            }
        };
        let old_stem = match state.target_path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => {
                self.set_rename_error("Project file has no stem.");
                return iced::Task::none();
            }
        };
        if new_stem == old_stem {
            self.ui_state.rename_dialog = None;
            return iced::Task::none();
        }
        let new_prj = dir.join(format!("{new_stem}.snxprj"));
        if let Err(refusal) = self.refuse_rename_overlap(&state.target_path, &new_prj) {
            self.set_rename_error(&refusal);
            return iced::Task::none();
        }
        // Rename only the .snxprj. Schematic / PCB files keep their
        // original filenames; the project's `schematic_root` /
        // `pcb_file` references continue to point at them.
        let new_name = new_stem.to_string();
        self.start_file_rename(
            state,
            new_prj,
            new_name,
            crate::app::path_ops::FileRenameKind::Project,
        )
    }
}
