//! File ▸ Library ▸ New Component… — appends a draft row to the active
//! library's table and opens it; tells the user why when it cannot.

use super::*;

/// Messages-panel text for "New Component" with no library open.
pub(super) const NO_LIBRARY_OPEN: &str =
    "New Component: no library is open. Open a library with a component table first.";

/// Messages-panel text for "New Component" on a library with no table.
pub(super) const NO_COMPONENT_TABLE: &str = "New Component: the library has no component table. Open a library with a component table first.";

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
        let library_idx = 0usize;
        let library = &self.library.open_libraries[library_idx];
        let library_path = library.root.clone();
        let library_id = library.library_id;
        let class = signex_library::ComponentClass::default();
        let table = self
            .library
            .set
            .get(library_id)
            .map(|adapter| adapter.manifest().table_for_class(class.as_str()));
        let Some(table) = table else {
            tracing::warn!(target: "signex::library", "{NO_COMPONENT_TABLE}");
            return Task::none();
        };
        match commands::create_component_row(
            &mut self.library,
            library_idx,
            &table,
            "",
            class,
            None,
            None,
        ) {
            Ok(row_id) => Task::done(Message::Library(LibraryMessage::OpenComponentRow {
                library_path,
                table,
                row_id,
            })),
            Err(e) => {
                tracing::error!(
                    target: "signex::library",
                    error = %e,
                    "New Component: could not append a row to the library table"
                );
                Task::none()
            }
        }
    }
}
