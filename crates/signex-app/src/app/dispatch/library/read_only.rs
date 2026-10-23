//! Read-only library guard — the user-facing half of issue #329.
//!
//! The adapter boundary already refuses every write with
//! `LibraryError::ReadOnly` (`signex_library::ReadOnlyAdapter`). These
//! helpers sit one step earlier in the dispatcher so a write attempt on
//! a read-only mount (the stock library) stops before any draft state,
//! modal, or buffer is created and the user is told in plain language
//! why, instead of seeing a permission error or nothing at all.

use super::*;

impl Signex {
    /// Refuse a write aimed at the library at `library_path` when it is
    /// mounted read-only. Returns `true` when the write was refused, in
    /// which case the explanation card is already raised and the caller
    /// must stop.
    pub(crate) fn refuse_read_only_write(&mut self, library_path: &std::path::Path) -> bool {
        if !self.library.is_read_only(library_path) {
            return false;
        }
        let name = self
            .library
            .library_at(library_path)
            .map(|lib| lib.display_name.clone())
            .unwrap_or_else(|| library_path.display().to_string());
        crate::diagnostics::log_warning(format!(
            "[library] write refused: {name} is mounted read-only"
        ));
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::read_only(&name));
        true
    }

    /// Error for a direct file write (the standalone symbol / footprint
    /// editors write the container themselves, bypassing the adapter)
    /// into a read-only library, `None` when `path` is writable.
    pub(crate) fn read_only_write_error(&self, path: &std::path::Path) -> Option<anyhow::Error> {
        let name = self.library.read_only_library_name_containing(path)?;
        Some(anyhow::Error::new(signex_library::LibraryError::ReadOnly {
            library: name,
        }))
    }
}
