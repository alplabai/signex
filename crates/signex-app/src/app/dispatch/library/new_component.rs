//! File ▸ Library ▸ New Component… — appends a draft row to the active
//! library's table and opens it; tells the user why when it cannot.

use super::*;
use crate::library::writes::AfterRowCreated;

/// Messages-panel text for "New Component" with no library open.
pub(super) const NO_LIBRARY_OPEN: &str =
    "New Component: no library is open. Open a library with a component table first.";

/// Messages-panel text for "New Component" when the listed library's
/// adapter is not mounted, so no component table can be resolved.
fn library_not_loaded(display_name: &str) -> String {
    format!(
        "New Component: library \"{display_name}\" is not loaded, so it has no component table to add to."
    )
}

impl Signex {
    /// File ▸ Library ▸ New Component… — appends a draft row directly to
    /// the active library's first table and focuses it. With no library
    /// open, or no table to append to, nothing can be created, so the
    /// reason goes to the Messages panel instead of failing silently.
    pub(super) fn handle_new_component(&mut self) -> Task<Message> {
        if self.library.open_libraries.is_empty() {
            tracing::warn!(target: "signex::library", "{NO_LIBRARY_OPEN}");
            return Task::none();
        }
        // The first library that accepts writes; the stock library is
        // mounted read-only and can never be the target.
        let Some(library_idx) = self
            .library
            .open_libraries
            .iter()
            .position(|lib| !self.library.is_read_only(&lib.root))
        else {
            tracing::warn!(
                target: "signex::library",
                "New Component: every open library is mounted read-only; open or create a writable library first."
            );
            return Task::none();
        };
        let library = &self.library.open_libraries[library_idx];
        let library_id = library.library_id;
        let display_name = library.display_name.clone();
        let class = signex_library::ComponentClass::default();
        let table = self
            .library
            .set
            .get(library_id)
            .map(|adapter| adapter.manifest().table_for_class(class.as_str()));
        let Some(table) = table else {
            tracing::warn!(target: "signex::library", "{}", library_not_loaded(&display_name));
            return Task::none();
        };
        self.create_row_task(library_idx, &table, class, AfterRowCreated::OpenPreview)
    }
}
