//! Re-keying a mounted library after its `.snxlib` was renamed on disk
//! (#739).
//!
//! Everything that remembers a library by its `.snxlib` path — the
//! mount, the editors, the browser tabs' state, the modals — has to move
//! to the new path together. A key left on the old path points at a file
//! that no longer exists, and the adapter behind it would recreate the
//! library there on its next write.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use signex_library::LibraryError;

use super::{EditorAddress, LibraryState};

/// `path` moved from under `old` to under `new`, or `None` when it does
/// not live at or below `old`. A library's own key equals `old`; the
/// synthetic Component Preview tab paths sit below it.
pub fn rebase_path(path: &Path, old: &Path, new: &Path) -> Option<PathBuf> {
    path.strip_prefix(old).ok().map(|rest| new.join(rest))
}

fn rebased_or_same(path: &Path, old: &Path, new: &Path) -> PathBuf {
    rebase_path(path, old, new).unwrap_or_else(|| path.to_path_buf())
}

fn rebase_address(address: &mut EditorAddress, old: &Path, new: &Path) {
    address.library_path = rebased_or_same(&address.library_path, old, new);
}

impl LibraryState {
    /// Move the library mounted at `old` to `new` (the same `.snxlib`,
    /// renamed). The adapter is told its new path rather than re-opened,
    /// so nothing is read from disk and the cached rows, open editors and
    /// browser state all carry over.
    ///
    /// A path that is not mounted has nothing to move and succeeds. A
    /// failure leaves every key on `old`.
    pub fn relocate_library(&mut self, old: &Path, new: &Path) -> Result<(), LibraryError> {
        if self.pending_mounts.contains_key(old) {
            return Err(LibraryError::Conflict(format!(
                "{} is still opening",
                old.display()
            )));
        }
        let Some(idx) = self.open_libraries.iter().position(|lib| lib.root == old) else {
            return Ok(());
        };
        self.set.relocate(old, new)?;
        self.open_libraries[idx].root = new.to_path_buf();

        let editors = std::mem::take(&mut self.editors);
        self.editors = editors
            .into_iter()
            .map(|(mut key, mut state)| {
                rebase_address(&mut key, old, new);
                state.library_path = rebased_or_same(&state.library_path, old, new);
                (key, state)
            })
            .collect::<HashMap<_, _>>();

        let browsers = std::mem::take(&mut self.library_browsers);
        self.library_browsers = browsers
            .into_iter()
            .map(|(key, mut browser)| {
                browser.library_path = rebased_or_same(&browser.library_path, old, new);
                if let Some(modal) = browser.edit_modal.as_mut() {
                    rebase_address(&mut modal.address, old, new);
                }
                (rebased_or_same(&key, old, new), browser)
            })
            .collect::<HashMap<_, _>>();

        let skipped = std::mem::take(&mut self.skipped_updates_for);
        self.skipped_updates_for = skipped
            .into_iter()
            .map(|p| rebased_or_same(&p, old, new))
            .collect::<HashSet<_>>();

        if let Some((path, _)) = self.picker.as_mut().and_then(|p| p.selected.as_mut()) {
            *path = rebased_or_same(path, old, new);
        }
        if let Some(confirm) = self.close_library_confirm.as_mut() {
            confirm.library_path = rebased_or_same(&confirm.library_path, old, new);
            for address in &mut confirm.dirty_editors {
                rebase_address(address, old, new);
            }
        }
        if let Some(options) = self.document_options.as_mut() {
            options.library_path = rebased_or_same(&options.library_path, old, new);
        }
        Ok(())
    }
}
