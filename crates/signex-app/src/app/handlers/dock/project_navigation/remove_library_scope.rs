//! What a library delete must wait for, and what it takes with it.
//!
//! Deleting `<dir>/<name>.snxlib` also clears the library's known folders
//! (`library_delete`), so a draft living there, a write still running or a
//! mount still preparing would be lost or recreate files after the delete.

use super::*;
use crate::app::library_delete::known_folders;
use crate::app::state::scope::{CaseFold, path_is_under};

impl Signex {
    /// The known library folders beside the manifest `target`; empty for any
    /// other target.
    pub(super) fn library_folder_roots(target: &std::path::Path) -> Vec<std::path::PathBuf> {
        if !crate::app::file_extension::has_extension(target, "snxlib") {
            return Vec::new();
        }
        let Some(dir) = target.parent() else {
            return Vec::new();
        };
        known_folders()
            .into_iter()
            .map(|(folder, _)| dir.join(folder))
            .collect()
    }

    /// Why deleting the library `target` must wait, or `None`.
    pub(super) fn library_delete_refusal(
        &self,
        target: &std::path::Path,
        fold: CaseFold,
    ) -> Option<String> {
        if !crate::app::file_extension::has_extension(target, "snxlib") {
            return None;
        }
        if self
            .library
            .pending_mounts
            .keys()
            .any(|p| path_is_under(p, target, fold))
        {
            return Some("This library is still opening. Try again in a moment.".to_string());
        }
        if let Some(editor) = self.dirty_editors_under(target, fold).first() {
            return Some(format!(
                "Deleting the library would discard unsaved edits to {}. Save or close it first.",
                self.library.editor_label(editor)
            ));
        }
        let dirty = Self::library_folder_roots(target)
            .iter()
            .flat_map(|root| self.unsaved_documents(root, fold))
            .next()?;
        Some(format!(
            "Deleting the library would discard unsaved changes to {}. Save or close it first.",
            dirty.display()
        ))
    }

    /// Editors holding unsaved edits for the library `target`, compared by
    /// the same case policy as the drafts beside them.
    fn dirty_editors_under(
        &self,
        target: &std::path::Path,
        fold: CaseFold,
    ) -> Vec<crate::library::state::EditorAddress> {
        self.library
            .dirty_editors_matching(|root| path_is_under(root, target, fold))
    }

    /// Say in Messages which unsaved editors the finished delete of `target`
    /// takes with it: one may have gone dirty while the worker ran.
    pub(super) fn report_editors_lost_to_delete(&self, target: &std::path::Path, fold: CaseFold) {
        for editor in self.dirty_editors_under(target, fold) {
            crate::diagnostics::log_warning(format!(
                "Deleted library {}: unsaved edits to {} were discarded with it.",
                target.display(),
                self.library.editor_label(&editor)
            ));
        }
    }

    /// The library was deleted from `dir`: close the tabs and drop the
    /// documents of the files its clean-up removed. What is in `left` stays.
    pub(super) fn forget_deleted_library_folders(
        &mut self,
        dir: &std::path::Path,
        left: &[std::path::PathBuf],
        fold: CaseFold,
    ) -> Vec<iced::Task<Message>> {
        let roots: Vec<_> = known_folders()
            .into_iter()
            .map(|(folder, _)| dir.join(folder))
            .collect();
        let gone = |p: &std::path::Path| {
            roots.iter().any(|r| path_is_under(p, r, fold))
                && !left.iter().any(|l| path_is_under(p, l, fold))
        };
        let tasks = self.close_tabs_where(gone);
        self.drop_documents_where(dir, gone);
        tasks
    }
}
