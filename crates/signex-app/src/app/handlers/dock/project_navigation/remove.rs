//! Remove-from-project / delete-file flow for the project-navigation dock.
//!
//! Extracted verbatim from the project-navigation dock handlers
//! (`handlers/dock/project_navigation`); pure code motion, zero
//! behaviour change.

use super::*;
use crate::app::state::library_entry::entry_resolves_to;
use crate::app::state::scope::{CaseFold, path_is_under, path_key_with};

impl Signex {
    pub(super) fn open_remove_dialog(&mut self, tree_path: Vec<usize>) {
        let Some(target_path) = self.tree_path_to_file_path(&tree_path) else {
            crate::diagnostics::log_warning(
                "Remove cancelled: the selected project-tree row no longer resolves to a file.",
            );
            return;
        };
        let display_name = target_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let is_outside_project = self.is_outside_project_target(&tree_path, &target_path);
        let project_id = self.project_id_at_tree_path(&tree_path);
        self.ui_state.remove_dialog = Some(crate::app::RemoveDialogState {
            target_path,
            tree_path,
            project_id,
            display_name,
            is_outside_project,
            unsaved_prompt: false,
        });
    }

    /// Open the Remove dialog for the library `row` names (#742).
    pub(super) fn open_library_remove_dialog(&mut self, row: crate::app::LibraryRowRef) {
        let Some(project_idx) = self.resolve_library_row(&row, "Remove") else {
            return;
        };
        let display_name = row
            .root
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let tree_path = vec![project_idx];
        let is_outside_project = self.is_outside_project_target(&tree_path, &row.root);
        self.ui_state.remove_dialog = Some(crate::app::RemoveDialogState {
            target_path: row.root,
            tree_path,
            project_id: Some(row.project_id),
            display_name,
            is_outside_project,
            unsaved_prompt: false,
        });
    }

    /// Folder holding the `.snxprj` that owns the tree row at `tree_path`.
    fn project_root_for_tree_path(&self, tree_path: &[usize]) -> Option<std::path::PathBuf> {
        let project = self.document_state.projects.get(*tree_path.first()?)?;
        project.path.parent().map(std::path::Path::to_path_buf)
    }

    /// Is the tree row's target a Shared / Global library entry?
    fn is_shared_library_target(&self, tree_path: &[usize], target: &std::path::Path) -> bool {
        tree_path
            .first()
            .and_then(|i| self.document_state.projects.get(*i))
            .is_some_and(|p| super::remove_guard::is_shared_or_global_library(&p.data, target))
    }

    /// Kind-aware outside check: a Shared / Global library entry is outside
    /// the project whatever its path says; otherwise fall back to path
    /// containment (#139).
    fn is_outside_project_target(&self, tree_path: &[usize], target: &std::path::Path) -> bool {
        self.is_shared_library_target(tree_path, target)
            || super::remove_guard::is_outside_project(
                target,
                self.project_root_for_tree_path(tree_path).as_deref(),
            )
    }

    /// Delete the dialog's target from disk, but only if it is provably
    /// inside the owning project's folder (#139). A Shared / Global
    /// library lives outside it and belongs to other projects too, so a
    /// refusal is surfaced to the user rather than only logged. This
    /// holds even when the dialog state's `is_outside_project` is wrong.
    fn delete_project_owned_target(&mut self, state: &crate::app::RemoveDialogState) {
        // F23 — three cases:
        //   1. Regular file (sheets, pcbs, primitives) → remove_file.
        //   2. `.snxlib` directory package → remove_dir_all.
        //   3. Orphan entry (missing on disk) → no-op; the caller still
        //      prunes the in-memory entry.
        let target = &state.target_path;
        let fold = state.project_id.map_or_else(CaseFold::host_default, |id| {
            self.document_state.fold_for_project(id)
        });
        match target.try_exists() {
            Ok(true) => {}
            Ok(false) => {
                // Orphan entry: nothing on disk, but a tab may still hold it (#736).
                self.drop_deleted_documents(target, fold);
                return;
            }
            Err(e) => {
                crate::diagnostics::log_error(
                    "Could not check whether the project file exists",
                    &anyhow::anyhow!("{}: {e}", target.display()),
                );
                return;
            }
        }
        let root = self.project_root_for_tree_path(&state.tree_path);
        let contained = root
            .as_deref()
            .is_some_and(|r| super::remove_guard::is_inside_project_root(target, r));
        if !contained || self.is_shared_library_target(&state.tree_path, target) {
            self.document_state.error_notice =
                Some(crate::app::state::ErrorNotice::delete(format!(
                    "\"{}\" is a shared library or lives outside this project's folder, so \
                     it was not deleted from disk. It has only been removed from the project.",
                    state.display_name
                )));
            return;
        }
        // A write still running for this file would recreate it after the
        // delete; let it finish first (same bounded wait as rename).
        self.document_state.document_saves.barrier().wait();
        let result = if target.is_dir() {
            std::fs::remove_dir_all(target)
        } else {
            std::fs::remove_file(target)
        };
        match result {
            // The file is gone: forget any document still held for it (#736).
            Ok(()) => self.drop_deleted_documents(target, fold),
            Err(e) => {
                crate::diagnostics::log_error(
                    "Failed to delete project file",
                    &anyhow::anyhow!("{}: {e}", target.display()),
                );
                self.document_state.error_notice = Some(crate::app::state::ErrorNotice::delete(
                    format!("Could not delete \"{}\": {e}", state.display_name),
                ));
            }
        }
    }

    /// Close the tabs of the file being removed. A delete also closes tabs
    /// of anything under it (a `.snxlib` directory's editors); an exclude
    /// only closes the file's own tabs.
    pub(super) fn close_tabs_of_removed(
        &mut self,
        target: &std::path::Path,
        include_descendants: bool,
        fold: CaseFold,
    ) -> Vec<iced::Task<Message>> {
        let matches_target = |p: &std::path::Path| {
            if include_descendants {
                path_is_under(p, target, fold)
            } else {
                path_key_with(p, fold) == path_key_with(target, fold)
            }
        };
        let mut tasks = Vec::new();
        while let Some(idx) = self
            .document_state
            .tabs
            .iter()
            .position(|t| matches_target(&t.path))
        {
            tasks.push(self.close_tab_now(idx));
        }
        tasks
    }

    /// Forget the in-memory documents of a file (or `.snxlib` directory)
    /// the user just deleted. A parked dirty engine or draft would
    /// otherwise survive in `dirty_paths` and be written back by Save All.
    fn drop_deleted_documents(&mut self, deleted: &std::path::Path, fold: CaseFold) {
        let gone = |p: &std::path::PathBuf| path_is_under(p, deleted, fold);
        let dead: Vec<_> = self
            .document_state
            .engines
            .keys()
            .filter(|p| gone(p))
            .cloned()
            .collect();
        for path in &dead {
            self.forget_net_colors_of(path);
        }
        let doc = &mut self.document_state;
        doc.engines.retain(|p, _| !gone(p));
        doc.symbol_editors.retain(|p, _| !gone(p));
        doc.footprint_editors.retain(|p, _| !gone(p));
        let discarded = doc.dirty_paths.iter().filter(|p| gone(p)).count();
        doc.dirty_paths.retain(|p| !gone(p));
        doc.document_saves
            .forget_where(|p| path_is_under(p, deleted, fold));
        if discarded > 0 {
            crate::diagnostics::log_warning(format!(
                "Deleted {}: discarded {discarded} unsaved document(s)",
                deleted.display()
            ));
        }
    }

    pub(crate) fn handle_remove_confirm(
        &mut self,
        choice: crate::app::RemoveChoice,
    ) -> iced::Task<Message> {
        use crate::app::RemoveChoice;

        // Excluding a document (or a library / folder holding documents) with
        // unsaved edits asks first (#738): the modal stays open and moves to
        // its Save / Discard / Cancel stage.
        if choice == RemoveChoice::ExcludeFromProject {
            // A footprint draft only reaches `dirty_paths` on a sync.
            self.sync_editor_dirty_paths();
            if let Some(dialog) = self.ui_state.remove_dialog.clone()
                && self.has_unsaved_edits(&dialog.target_path, self.dialog_fold(&dialog))
            {
                let Some(dialog) = self.repointed_dialog(&dialog) else {
                    self.ui_state.remove_dialog = None;
                    self.refresh_panel_ctx();
                    return iced::Task::none();
                };
                if self.discard_blocked_by_other_project(&dialog) {
                    crate::diagnostics::log_warning(format!(
                        "{} has unsaved edits and another open project also uses it: \
                         Discard is not offered, only Save or Cancel.",
                        dialog.display_name
                    ));
                    self.refresh_panel_ctx();
                }
                if let Some(dialog) = self.ui_state.remove_dialog.as_mut() {
                    dialog.unsaved_prompt = true;
                }
                return iced::Task::none();
            }
        }

        // An open still reading a file being deleted would land on a path
        // that no longer exists, and a save of that tab would recreate it
        // (#1096). The dialog stays open so the delete can be confirmed
        // again once the file has opened.
        if choice == RemoveChoice::DeleteFile
            && let Some(dialog) = self.ui_state.remove_dialog.as_ref()
            && let Some(opening) = self.pending_open_under(&dialog.target_path)
        {
            let detail = format!(
                "{} is still opening, so {} was not deleted. Delete it again once it has opened.",
                opening.display(),
                dialog.display_name
            );
            crate::diagnostics::log_warning(&detail);
            self.document_state.error_notice = Some(crate::app::state::ErrorNotice::delete(detail));
            return iced::Task::none();
        }

        // A restore still running for a file being deleted would write it
        // back (or leave a stray file) when its worker finishes.
        if choice == RemoveChoice::DeleteFile
            && let Some(dialog) = self.ui_state.remove_dialog.as_ref()
            && let Some(restoring) = self
                .document_state
                .pending_restores
                .find_under(&dialog.target_path)
        {
            let detail = format!(
                "A History restore of {} is still running, so {} was not deleted. Delete it again once the restore has finished.",
                restoring.display(),
                dialog.display_name
            );
            crate::diagnostics::log_warning(&detail);
            self.document_state.error_notice = Some(crate::app::state::ErrorNotice::delete(detail));
            return iced::Task::none();
        }

        let Some(state) = self.ui_state.remove_dialog.take() else {
            return iced::Task::none();
        };
        self.apply_remove(&state, choice)
    }

    /// A pending open of `target` itself or of a file under it (a library
    /// directory or a folder).
    pub(super) fn pending_open_under(
        &self,
        target: &std::path::Path,
    ) -> Option<std::path::PathBuf> {
        self.document_state.pending_opens.find_under(target)
    }

    /// Carry out a confirmed Remove: close the file's tabs, delete it from
    /// disk for `DeleteFile`, and drop it from the owning project.
    pub(super) fn apply_remove(
        &mut self,
        state: &crate::app::RemoveDialogState,
        choice: crate::app::RemoveChoice,
    ) -> iced::Task<Message> {
        use crate::app::RemoveChoice;
        let Some(state) = self.repointed_dialog(state) else {
            return iced::Task::none();
        };

        // Close any tab backed by this file before we touch disk or
        // drop it from the project's in-memory list — otherwise the
        // tab would keep referring to a file that no longer exists.
        let is_delete = matches!(choice, RemoveChoice::DeleteFile);
        let fold = self.dialog_fold(&state);
        let close_tasks = self.close_tabs_of_removed(&state.target_path, is_delete, fold);

        if matches!(choice, RemoveChoice::DeleteFile) {
            // F23 — handle three cases:
            //   1. Regular file (sheets, pcbs, primitives) → remove_file.
            //   2. `.snxlib` directory package → remove_dir_all.
            //   3. Orphan entry (file/dir missing on disk) → no-op,
            //      Exclude semantic falls through naturally so the
            //      LibraryEntry / SheetEntry is still pruned below.
            self.delete_project_owned_target(&state);
            // Orphan target: silently fall through. The user's intent
            // ("get this row out of the project") is satisfied by the
            // in-memory entry pruning below.
        }

        // Drop the sheet from the *owning* project's in-memory data so
        // the tree rebuild reflects the removal. Resolves project via
        // the dialog's `tree_path[0]` so removing a sheet of project B
        // doesn't accidentally mutate active project A. Session-scoped:
        // reopening the project rescans disk and (for Exclude)
        // re-discovers the file, until proper project-write support
        // lands. (#54)
        let owner_idx = state.tree_path.first().copied();
        let mut mutated = false;
        let mut project_path: Option<std::path::PathBuf> = None;
        if let Some(idx) = owner_idx
            && let Some(loaded) = self.document_state.projects.get_mut(idx)
        {
            project_path = Some(loaded.path.clone());
            // By resolved path, not file name: a sheet registered from a
            // subdirectory is `sub/Power.snxsch` (#1367).
            let fold = loaded.case_fold;
            let dir = loaded.dir().to_path_buf();
            let is_target = |entry: &str| entry_resolves_to(&dir, entry, &state.target_path, fold);
            let before = loaded.data.sheets.len();
            loaded
                .data
                .sheets
                .retain(|entry| !is_target(&entry.filename));
            if loaded.data.sheets.len() != before {
                mutated = true;
            }
            if loaded.data.schematic_root.as_deref().is_some_and(is_target) {
                loaded.data.schematic_root = None;
                mutated = true;
            }
            if loaded.data.pcb_file.as_deref().is_some_and(is_target) {
                loaded.data.pcb_file = None;
                mutated = true;
            }
            // F23 — also prune library entries. Library file_name on
            // the entry is the `.snxlib` filename for both
            // ProjectLocal (relative path stored on the entry) and
            // Shared (absolute path stored), so the same file_name
            // match works for both kinds.
            // Match by resolved path, not file name: two libraries may share
            // a name (`Power.snxlib`) and only the targeted one goes (#742).
            let lib_before = loaded.data.libraries.len();
            let data = &loaded.data;
            let keep: Vec<bool> = data
                .libraries
                .iter()
                .map(|entry| data.resolve_library_path(entry) != state.target_path)
                .collect();
            let mut keep = keep.into_iter();
            loaded
                .data
                .libraries
                .retain(|_| keep.next().unwrap_or(true));
            if loaded.data.libraries.len() != lib_before {
                mutated = true;
            }
            // Drop any pending registration too, in case the user
            // saved-then-removed within a single session before
            // materialise ran (rare but possible if materialise failed
            // and the entry stayed pending).
            let pending_before = loaded.pending_libraries.len();
            loaded
                .pending_libraries
                .retain(|_, spec| spec.lib_path != state.target_path);
            if loaded.pending_libraries.len() != pending_before {
                mutated = true;
            }
        }
        if mutated && let Some(p) = project_path {
            // Match Add Existing — Remove from Project also dirties the
            // .snxprj so the project root row gets the red dot until
            // the user saves the metadata change. The on-disk file
            // delete already happened above when DeleteFile was picked;
            // the dirty bit only tracks the project's *list* of
            // children, which lives in the .snxprj.
            self.document_state.dirty_paths.insert(p);
        }

        self.refresh_panel_ctx();
        if close_tasks.is_empty() {
            iced::Task::none()
        } else {
            iced::Task::batch(close_tasks)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::app::state::scope::CaseFold;
    use crate::app::{FootprintDocument, Signex, TabInfo, TabKind};

    fn sheet() -> signex_types::schematic::SchematicSheet {
        signex_types::schematic::SchematicSheet {
            uuid: uuid::Uuid::new_v4(),
            paper_size: "A4".to_string(),
            ..Default::default()
        }
    }

    fn dirty_engine(app: &mut Signex, path: &Path) {
        let engine = signex_engine::Engine::new(sheet()).expect("engine");
        app.document_state
            .engines
            .insert(path.to_path_buf(), engine);
        app.document_state.dirty_paths.insert(path.to_path_buf());
    }

    #[test]
    fn delete_drops_an_engine_keyed_with_different_case_on_a_folding_host() {
        let (mut app, _task) = Signex::new();
        let keyed = PathBuf::from("/w/top.snxsch");
        dirty_engine(&mut app, &keyed);

        app.drop_deleted_documents(Path::new("/w/Top.snxsch"), CaseFold::FOLD);

        assert!(!app.document_state.engines.contains_key(&keyed));
        assert!(!app.document_state.dirty_paths.contains(&keyed));
    }

    #[test]
    fn delete_drops_the_removed_sheets_net_colours() {
        let (mut app, _task) = Signex::new();
        let gone = PathBuf::from("/w/gone.snxsch");
        let kept = PathBuf::from("/w/kept.snxsch");
        let wire_of = |id| signex_types::schematic::Wire {
            uuid: id,
            ..Default::default()
        };
        let (gone_wire, kept_wire) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        for (path, id) in [(&gone, gone_wire), (&kept, kept_wire)] {
            let mut s = sheet();
            s.wires.push(wire_of(id));
            app.document_state
                .engines
                .insert(path.clone(), signex_engine::Engine::new(s).expect("engine"));
            app.ui_state.wire_color_overrides.insert(
                id,
                signex_types::theme::Color {
                    r: 1,
                    g: 0,
                    b: 0,
                    a: 255,
                },
            );
        }

        app.drop_deleted_documents(&gone, CaseFold::PRESERVE);

        assert!(!app.ui_state.wire_color_overrides.contains_key(&gone_wire));
        assert!(app.ui_state.wire_color_overrides.contains_key(&kept_wire));
    }

    #[test]
    fn delete_keeps_an_engine_whose_case_differs_on_a_preserving_host() {
        let (mut app, _task) = Signex::new();
        let keyed = PathBuf::from("/w/top.snxsch");
        dirty_engine(&mut app, &keyed);

        app.drop_deleted_documents(Path::new("/w/Top.snxsch"), CaseFold::PRESERVE);

        assert!(app.document_state.engines.contains_key(&keyed));
    }

    fn footprint_tab(app: &mut Signex, path: &Path) {
        let file =
            signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("fp"));
        app.document_state.footprint_editors.insert(
            path.to_path_buf(),
            FootprintDocument::new(path.to_path_buf(), file),
        );
        app.document_state.tabs.push(TabInfo {
            title: "fp".into(),
            path: path.to_path_buf(),
            cached_document: None,
            dirty: false,
            project_id: None,
            kind: TabKind::FootprintEditor(path.to_path_buf()),
        });
    }

    #[test]
    fn delete_closes_tabs_of_everything_under_the_deleted_directory() {
        let (mut app, _task) = Signex::new();
        footprint_tab(&mut app, Path::new("/w/lib.snxlib/footprints/g.snxfpt"));
        footprint_tab(&mut app, Path::new("/w/other.snxlib/footprints/h.snxfpt"));

        let _ = app.close_tabs_of_removed(Path::new("/w/Lib.snxlib"), true, CaseFold::FOLD);

        let left: Vec<_> = app.document_state.tabs.iter().map(|t| &t.path).collect();
        assert_eq!(left, [Path::new("/w/other.snxlib/footprints/h.snxfpt")]);
    }

    #[test]
    fn exclude_closes_only_the_files_own_tabs() {
        let (mut app, _task) = Signex::new();
        footprint_tab(&mut app, Path::new("/w/lib.snxlib/footprints/g.snxfpt"));

        let _ = app.close_tabs_of_removed(Path::new("/w/lib.snxlib"), false, CaseFold::FOLD);

        assert_eq!(app.document_state.tabs.len(), 1);
    }
}
