//! Remove-from-project / delete-file flow for the project-navigation dock.
//!
//! Extracted verbatim from the project-navigation dock handlers
//! (`handlers/dock/project_navigation`); pure code motion, zero
//! behaviour change.

use super::*;
use crate::app::state::scope::{CaseFold, path_is_under, path_key_with};

impl Signex {
    pub(super) fn open_remove_dialog(&mut self, tree_path: Vec<usize>) {
        let Some(target_path) = self.tree_path_to_file_path(&tree_path) else {
            return;
        };
        let display_name = target_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let is_outside_project = self.is_outside_project_target(&tree_path, &target_path);
        self.ui_state.remove_dialog = Some(crate::app::RemoveDialogState {
            target_path,
            tree_path,
            display_name,
            is_outside_project,
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
        match target.try_exists() {
            Ok(true) => {}
            Ok(false) => {
                // Orphan entry: nothing on disk, but a tab may still hold it (#736).
                self.drop_deleted_documents(target, CaseFold::host_default());
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
        let result = if target.is_dir() {
            std::fs::remove_dir_all(target)
        } else {
            std::fs::remove_file(target)
        };
        match result {
            // The file is gone: forget any document still held for it (#736).
            Ok(()) => self.drop_deleted_documents(target, CaseFold::host_default()),
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
    fn close_tabs_of_removed(
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
        let doc = &mut self.document_state;
        let gone = |p: &std::path::PathBuf| path_is_under(p, deleted, fold);
        doc.engines.retain(|p, _| !gone(p));
        doc.symbol_editors.retain(|p, _| !gone(p));
        doc.footprint_editors.retain(|p, _| !gone(p));
        let discarded = doc.dirty_paths.iter().filter(|p| gone(p)).count();
        doc.dirty_paths.retain(|p| !gone(p));
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

        let Some(state) = self.ui_state.remove_dialog.take() else {
            return iced::Task::none();
        };

        // Close any tab backed by this file before we touch disk or
        // drop it from the project's in-memory list — otherwise the
        // tab would keep referring to a file that no longer exists.
        let is_delete = matches!(choice, RemoveChoice::DeleteFile);
        let close_tasks =
            self.close_tabs_of_removed(&state.target_path, is_delete, CaseFold::host_default());

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
        if let Some(filename) = state
            .target_path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            && let Some(idx) = owner_idx
            && let Some(loaded) = self.document_state.projects.get_mut(idx)
        {
            project_path = Some(loaded.path.clone());
            let before = loaded.data.sheets.len();
            loaded
                .data
                .sheets
                .retain(|entry| entry.filename != filename);
            if loaded.data.sheets.len() != before {
                mutated = true;
            }
            if loaded.data.schematic_root.as_deref() == Some(filename.as_str()) {
                loaded.data.schematic_root = None;
                mutated = true;
            }
            if loaded.data.pcb_file.as_deref() == Some(filename.as_str()) {
                loaded.data.pcb_file = None;
                mutated = true;
            }
            // F23 — also prune library entries. Library file_name on
            // the entry is the `.snxlib` filename for both
            // ProjectLocal (relative path stored on the entry) and
            // Shared (absolute path stored), so the same file_name
            // match works for both kinds.
            let lib_before = loaded.data.libraries.len();
            loaded.data.libraries.retain(|entry| {
                entry
                    .path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|n| n != filename)
                    .unwrap_or(true)
            });
            if loaded.data.libraries.len() != lib_before {
                mutated = true;
            }
            // Drop any pending registration too, in case the user
            // saved-then-removed within a single session before
            // materialise ran (rare but possible if materialise failed
            // and the entry stayed pending).
            let pending_before = loaded.pending_libraries.len();
            loaded.pending_libraries.retain(|_, spec| {
                spec.lib_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|n| n != filename)
                    .unwrap_or(true)
            });
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
    use crate::app::{FootprintEditorState, Signex, TabInfo, TabKind};

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

        app.drop_deleted_documents(Path::new("/w/Top.snxsch"), CaseFold::Fold);

        assert!(!app.document_state.engines.contains_key(&keyed));
        assert!(!app.document_state.dirty_paths.contains(&keyed));
    }

    #[test]
    fn delete_keeps_an_engine_whose_case_differs_on_a_preserving_host() {
        let (mut app, _task) = Signex::new();
        let keyed = PathBuf::from("/w/top.snxsch");
        dirty_engine(&mut app, &keyed);

        app.drop_deleted_documents(Path::new("/w/Top.snxsch"), CaseFold::Preserve);

        assert!(app.document_state.engines.contains_key(&keyed));
    }

    fn footprint_tab(app: &mut Signex, path: &Path) {
        let file =
            signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("fp"));
        app.document_state.footprint_editors.insert(
            path.to_path_buf(),
            FootprintEditorState::new(path.to_path_buf(), file),
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

        let _ = app.close_tabs_of_removed(Path::new("/w/Lib.snxlib"), true, CaseFold::Fold);

        let left: Vec<_> = app.document_state.tabs.iter().map(|t| &t.path).collect();
        assert_eq!(left, [Path::new("/w/other.snxlib/footprints/h.snxfpt")]);
    }

    #[test]
    fn exclude_closes_only_the_files_own_tabs() {
        let (mut app, _task) = Signex::new();
        footprint_tab(&mut app, Path::new("/w/lib.snxlib/footprints/g.snxfpt"));

        let _ = app.close_tabs_of_removed(Path::new("/w/lib.snxlib"), false, CaseFold::Fold);

        assert_eq!(app.document_state.tabs.len(), 1);
    }
}
