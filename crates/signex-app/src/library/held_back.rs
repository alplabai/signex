//! The one place the app reports the pending files a library commit had
//! to leave out because they could not be read (#1729). Every adapter that
//! commits is drained through here: the mounted ones after each update and
//! before an unmount, the ones opened for a single job when it ends.

use signex_library::LibraryAdapter;

/// Report, in the Messages panel, every file `adapter`'s commits held back
/// since it was last asked. `library` names the library in the line.
pub fn report_held_back(library: &str, adapter: &dyn LibraryAdapter) {
    for held in adapter.take_held_back_files() {
        crate::diagnostics::log_warning(format!(
            "Not committed, kept pending: could not read {} ({}) in library {library}",
            held.path, held.reason
        ));
    }
}
