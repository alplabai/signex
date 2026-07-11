//! Library Browser table-admin handlers — the inline "+ Add Table",
//! rename-table and delete-table flows fired from the browser tab's
//! category strip.
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.

use super::*;

impl Signex {
    /// Flip the browser into add-table mode.
    pub(super) fn handle_browser_begin_add_table(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.adding_table = Some(crate::library::state::NewTableDraft::default());
        }
        Task::none()
    }

    /// Live-edit of the `+ Add Table` name buffer.
    pub(super) fn handle_browser_set_new_table_name(
        &mut self,
        library_path: std::path::PathBuf,
        value: String,
    ) -> Task<Message> {
        if let Some(state) = self.library.library_browsers.get_mut(&library_path)
            && let Some(draft) = state.adding_table.as_mut()
        {
            draft.name = value;
            draft.error = None;
        }
        Task::none()
    }

    /// Cancel the inline `+ Add Table` form without writing.
    pub(super) fn handle_browser_cancel_add_table(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.adding_table = None;
        }
        Task::none()
    }

    /// Delete an empty table from the strip's per-tab `×` button.
    pub(super) fn handle_browser_delete_table(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
    ) -> Task<Message> {
        let library_id = match self.library.library_at(&library_path) {
            Some(lib) => lib.library_id,
            None => return Task::none(),
        };
        let adapter = match self.library.set.get(library_id) {
            Some(a) => a,
            None => return Task::none(),
        };
        if let Err(error) =
            adapter.delete_empty_table(&table, &format!("delete empty table {table}"))
        {
            if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                s.delete_error = Some(error.to_string());
            }
            return Task::none();
        }
        if let Err(e) = self.library.refresh_components(&library_path) {
            tracing::warn!(
                target: "signex::library",
                path = %library_path.display(),
                error = %e,
                "refresh after delete table failed"
            );
        }
        if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
            s.delete_error = None;
            if s.active_table.as_deref() == Some(table.as_str()) {
                s.active_table = None;
            }
        }
        Task::none()
    }

    /// Dismiss the inline delete-table error message.
    pub(super) fn handle_browser_dismiss_delete_error(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
            s.delete_error = None;
        }
        Task::none()
    }

    /// Flip a table row into inline rename mode.
    pub(super) fn handle_browser_begin_rename_table(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
    ) -> Task<Message> {
        if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
            s.renaming_table = Some((table.clone(), table));
            s.rename_error = None;
        }
        Task::none()
    }

    /// Live-edit of the inline rename buffer.
    pub(super) fn handle_browser_set_rename_name(
        &mut self,
        library_path: std::path::PathBuf,
        value: String,
    ) -> Task<Message> {
        if let Some(s) = self.library.library_browsers.get_mut(&library_path)
            && let Some((_, buf)) = s.renaming_table.as_mut()
        {
            *buf = value;
            s.rename_error = None;
        }
        Task::none()
    }

    /// Cancel the inline rename without writing.
    pub(super) fn handle_browser_cancel_rename_table(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
            s.renaming_table = None;
            s.rename_error = None;
        }
        Task::none()
    }

    /// Confirm — calls `rename_table` on the adapter, swaps every
    /// in-memory reference to the table over to the new name.
    pub(super) fn handle_browser_confirm_rename_table(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        let Some(state) = self.library.library_browsers.get(&library_path).cloned() else {
            return Task::none();
        };
        let Some((old_name, new_buf)) = state.renaming_table.clone() else {
            return Task::none();
        };
        let new_trimmed = new_buf.trim().to_string();
        if new_trimmed == old_name {
            if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                s.renaming_table = None;
            }
            return Task::none();
        }
        let library_id = match self.library.library_at(&library_path) {
            Some(lib) => lib.library_id,
            None => return Task::none(),
        };
        let adapter = match self.library.set.get(library_id) {
            Some(a) => a,
            None => return Task::none(),
        };
        if let Err(error) = adapter.rename_table(
            &old_name,
            &new_trimmed,
            &format!("rename table {old_name} → {new_trimmed}"),
        ) {
            if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                s.rename_error = Some(error.to_string());
            }
            return Task::none();
        }
        if let Err(e) = self.library.refresh_components(&library_path) {
            tracing::warn!(
                target: "signex::library",
                path = %library_path.display(),
                error = %e,
                "refresh after rename table failed"
            );
        }
        if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
            s.renaming_table = None;
            s.rename_error = None;
            if s.active_table.as_deref() == Some(old_name.as_str()) {
                s.active_table = Some(new_trimmed);
            }
        }
        Task::none()
    }

    /// Confirm `+ Add Table` — calls `create_empty_table` on the
    /// adapter, refreshes the browser cache, switches the active
    /// tab to the new table.
    pub(super) fn handle_browser_confirm_add_table(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        let Some(state) = self.library.library_browsers.get(&library_path).cloned() else {
            return Task::none();
        };
        let Some(draft) = state.adding_table.as_ref().cloned() else {
            return Task::none();
        };
        let trimmed = draft.name.trim().to_string();
        if trimmed.is_empty() {
            if let Some(s) = self.library.library_browsers.get_mut(&library_path)
                && let Some(d) = s.adding_table.as_mut()
            {
                d.error = Some("Table name cannot be empty.".into());
            }
            return Task::none();
        }
        let library_id = match self.library.library_at(&library_path) {
            Some(lib) => lib.library_id,
            None => return Task::none(),
        };
        let adapter = match self.library.set.get(library_id) {
            Some(a) => a,
            None => return Task::none(),
        };
        if let Err(error) =
            adapter.create_empty_table(&trimmed, &format!("create empty table {trimmed}"))
        {
            if let Some(s) = self.library.library_browsers.get_mut(&library_path)
                && let Some(d) = s.adding_table.as_mut()
            {
                d.error = Some(error.to_string());
            }
            return Task::none();
        }
        if let Err(e) = self.library.refresh_components(&library_path) {
            tracing::warn!(
                target: "signex::library",
                path = %library_path.display(),
                error = %e,
                "refresh after add table failed"
            );
        }
        if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
            s.adding_table = None;
            s.active_table = Some(trimmed);
        }
        Task::none()
    }
}
