//! Library Browser grid-interaction handlers — active-table / row
//! selection, search, sort, per-cell edit buffers, the lifecycle
//! and class filters.
//!
//! Split out of the library dispatcher (`dispatch/library`).

use super::*;

impl Signex {
    /// Active table change inside a Library Browser tab.
    pub(in crate::app::dispatch::library) fn handle_browser_select_table(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
    ) -> Task<Message> {
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.active_table = Some(table);
            state.selected_row = None;
        }
        Task::none()
    }

    /// Search-buffer edit inside a Library Browser tab.
    pub(in crate::app::dispatch::library) fn handle_browser_search_changed(
        &mut self,
        library_path: std::path::PathBuf,
        value: String,
    ) -> Task<Message> {
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.search = value.clone();
        }
        // Write-through so reopening this library next session
        // restores the same filter — UX_IMPROVEMENTS §1.1.
        // Per-path scoping prevents two libraries from
        // sharing the same search term.
        crate::fonts::write_library_browser_search(&library_path, &value);
        Task::none()
    }

    /// Column-header click — toggles sort direction on the matching key.
    pub(in crate::app::dispatch::library) fn handle_browser_sort_column(
        &mut self,
        library_path: std::path::PathBuf,
        column_key: String,
    ) -> Task<Message> {
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.toggle_sort(column_key);
        }
        Task::none()
    }

    /// Row click inside the browser grid.
    pub(in crate::app::dispatch::library) fn handle_browser_select_row(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        row_id: RowId,
    ) -> Task<Message> {
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            // Switch active table when the click lands on a row
            // in a different table — keeps the preview pane and
            // selection coherent.
            state.active_table = Some(table);
            state.selected_row = Some(row_id);
        }
        Task::none()
    }

    /// User dismissed the delete confirm modal without deleting.
    pub(in crate::app::dispatch::library) fn handle_browser_delete_row_cancel(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.delete_confirm = None;
        }
        Task::none()
    }

    /// Live edit of a cell in the browser grid — updates the per-cell
    /// edit buffer.
    pub(in crate::app::dispatch::library) fn handle_browser_cell_edit(
        &mut self,
        library_path: std::path::PathBuf,
        row_id: RowId,
        column: String,
        value: String,
    ) -> Task<Message> {
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.cell_edit.insert((row_id, column), value);
        }
        Task::none()
    }

    /// Drop the per-cell edit buffer (Esc).
    pub(in crate::app::dispatch::library) fn handle_browser_cell_cancel(
        &mut self,
        library_path: std::path::PathBuf,
        row_id: RowId,
        column: String,
    ) -> Task<Message> {
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.cell_edit.remove(&(row_id, column));
        }
        Task::none()
    }

    /// Pick a lifecycle filter mode for the active Library Browser tab.
    pub(in crate::app::dispatch::library) fn handle_browser_set_lifecycle_filter(
        &mut self,
        library_path: std::path::PathBuf,
        filter: crate::library::state::LifecycleFilter,
    ) -> Task<Message> {
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.lifecycle_filter = filter;
            // Drop the row selection so we never end up with a
            // selected row that the new filter has just hidden
            // — the side preview pane would otherwise render
            // a row the user can no longer see in the grid.
            state.selected_row = None;
        }
        Task::none()
    }

    /// Toggle the per-class filter.
    pub(in crate::app::dispatch::library) fn handle_browser_class_filter_clicked(
        &mut self,
        library_path: std::path::PathBuf,
        key: String,
    ) -> Task<Message> {
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.class_filter = match state.class_filter.as_deref() {
                Some(current) if current == key => None,
                _ => Some(key.clone()),
            };
            // Reset selected_row in case the previously-selected row
            // is filtered out.
            state.selected_row = None;
        }
        Task::none()
    }
}
