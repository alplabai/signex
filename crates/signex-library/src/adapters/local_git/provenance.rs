//! The manifest's provenance keys, read through the adapter (#3112).

use super::LocalGitAdapter;
use crate::library_file::LibraryProvenance;

impl LocalGitAdapter {
    /// `license`, `source`, `converted_by` and `requires` of the `[library]`
    /// block, as the file holds them now.
    pub fn provenance(&self) -> LibraryProvenance {
        self.refresh_for_read();
        match self.library_file.read() {
            Ok(file) => file.manifest.library.provenance(),
            Err(poisoned) => poisoned.into_inner().manifest.library.provenance(),
        }
    }
}
