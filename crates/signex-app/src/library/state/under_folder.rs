//! Every library path the Model holds, for checks against a folder that is
//! about to move.

use std::path::{Path, PathBuf};

use super::LibraryState;

impl LibraryState {
    /// Each library path held anywhere in the Model: a mounted library
    /// (Global, Installed, project), the persisted Global list, an open
    /// browser tab's state, a mount still being prepared, and the stock
    /// library. Moving a folder that holds one of these would leave it
    /// pointing at a missing path, and the Global list would keep the stale
    /// path on disk.
    pub(crate) fn held_library_paths(&self) -> impl Iterator<Item = &Path> {
        self.open_libraries
            .iter()
            .map(|l| l.root.as_path())
            .chain(self.installed_libraries.iter().map(PathBuf::as_path))
            .chain(self.global_libraries.iter().map(|g| g.path.as_path()))
            .chain(self.library_browsers.keys().map(PathBuf::as_path))
            .chain(self.pending_mounts.keys().map(PathBuf::as_path))
            .chain(self.stock_library.as_deref())
    }
}
