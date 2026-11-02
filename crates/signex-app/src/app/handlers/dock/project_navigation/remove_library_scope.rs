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
    fn library_folder_roots(target: &std::path::Path) -> Vec<std::path::PathBuf> {
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
        let dirty = Self::library_folder_roots(target)
            .iter()
            .flat_map(|root| self.unsaved_documents(root, fold))
            .next()?;
        Some(format!(
            "Deleting the library would discard unsaved changes to {}. Save or close it first.",
            dirty.display()
        ))
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
