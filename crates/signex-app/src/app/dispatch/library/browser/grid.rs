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
        // Persisted debounced and off-thread so reopening this library
        // next session restores the same filter (#859). Per-path scoping
        // prevents two libraries from sharing the same search term.
        self.persist_library_search_soon(&library_path, &value)
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

    /// The grid's scrollable reported a new offset / viewport height.
    pub(in crate::app::dispatch::library) fn handle_browser_grid_scrolled(
        &mut self,
        library_path: std::path::PathBuf,
        offset_y: f32,
        viewport_h: f32,
    ) -> Task<Message> {
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.grid_scroll = crate::library::browser::GridScroll {
                offset_y,
                viewport_h,
            };
        }
        Task::none()
    }

    /// Scroll the grid so the selected row is in view; a no-op when it
    /// already is, or is not in the filtered list. The selection can move
    /// without a click (a freshly added row), so it may be outside the
    /// window the grid builds.
    pub(in crate::app::dispatch::library) fn reveal_selected_browser_row(
        &mut self,
        library_path: &std::path::Path,
    ) -> Task<Message> {
        // Bring the filtered rows up to date: the row set just changed.
        self.library.sync_view_caches();
        let Some(index) = self.library.browser_row_position_of_selection(library_path) else {
            return Task::none();
        };
        let Some(state) = self.library.library_browsers.get_mut(library_path) else {
            return Task::none();
        };
        let Some(offset_y) = crate::library::browser::reveal_row_offset(index, state.grid_scroll)
        else {
            return Task::none();
        };
        state.grid_scroll.offset_y = offset_y;
        iced::widget::operation::scroll_to(
            crate::library::browser::grid_scroll_id(library_path),
            iced::widget::scrollable::AbsoluteOffset {
                x: None,
                y: Some(offset_y),
            },
        )
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
        if self.refuse_read_only_write(&library_path) {
            return Task::none();
        }
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
