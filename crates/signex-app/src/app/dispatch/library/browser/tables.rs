//! Library Browser table-admin handlers — the inline "+ Add Table",
//! rename-table and delete-table flows fired from the browser tab's
//! category strip.
//!
//! Extracted verbatim from the library dispatcher (`dispatch/library`);
//! pure code motion, zero behaviour change.

use super::*;
use crate::library::refresh::RefreshFollowUp;
use crate::library::writes::{WriteFailure, WriteKind, write_task};

impl Signex {
    /// Flip the browser into add-table mode.
    pub(in crate::app::dispatch::library) fn handle_browser_begin_add_table(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        if self.refuse_read_only_write(&library_path) {
            return Task::none();
        }
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.adding_table = Some(crate::library::state::NewTableDraft::default());
        }
        Task::none()
    }

    /// Live-edit of the `+ Add Table` name buffer.
    pub(in crate::app::dispatch::library) fn handle_browser_set_new_table_name(
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
    pub(in crate::app::dispatch::library) fn handle_browser_cancel_add_table(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
            state.adding_table = None;
        }
        Task::none()
    }

    /// Delete an empty table from the strip's per-tab `×` button.
    pub(in crate::app::dispatch::library) fn handle_browser_delete_table(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
    ) -> Task<Message> {
        if self.refuse_read_only_write(&library_path) {
            return Task::none();
        }
        let name = table.clone();
        write_task(
            &mut self.library,
            &library_path,
            WriteKind::TableDeleted {
                name: table.clone(),
            },
            move |adapter| {
                adapter
                    .delete_empty_table(&name, &format!("delete empty table {name}"))
                    .map(|()| None)
            },
        )
    }

    /// The table delete finished.
    pub(in crate::app::dispatch::library) fn table_deleted(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
        outcome: Result<Option<u64>, WriteFailure>,
    ) -> Task<Message> {
        if let Err(failure) = outcome {
            tracing::error!(
                target: "signex::library",
                path = %library_path.display(),
                error = %failure.reason,
                "delete table failed; library left unchanged"
            );
            if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                s.delete_error = Some(failure.reason);
            }
            return Task::none();
        }
        let refresh = self.refresh_library_task(&library_path, RefreshFollowUp::Nothing);
        if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
            s.delete_error = None;
            if s.active_table.as_deref() == Some(table.as_str()) {
                s.active_table = None;
            }
        }
        refresh
    }

    /// Dismiss the inline delete-table error message.
    pub(in crate::app::dispatch::library) fn handle_browser_dismiss_delete_error(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
            s.delete_error = None;
        }
        Task::none()
    }

    /// Flip a table row into inline rename mode.
    pub(in crate::app::dispatch::library) fn handle_browser_begin_rename_table(
        &mut self,
        library_path: std::path::PathBuf,
        table: String,
    ) -> Task<Message> {
        if self.refuse_read_only_write(&library_path) {
            return Task::none();
        }
        if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
            s.renaming_table = Some((table.clone(), table));
            s.rename_error = None;
        }
        Task::none()
    }

    /// Live-edit of the inline rename buffer.
    pub(in crate::app::dispatch::library) fn handle_browser_set_rename_name(
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
    pub(in crate::app::dispatch::library) fn handle_browser_cancel_rename_table(
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
    pub(in crate::app::dispatch::library) fn handle_browser_confirm_rename_table(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        if self.refuse_read_only_write(&library_path) {
            return Task::none();
        }
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
        if state.rename_saving {
            return Task::none();
        }
        if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
            s.rename_saving = true;
            s.rename_error = None;
        }
        let (old, new) = (old_name.clone(), new_trimmed.clone());
        write_task(
            &mut self.library,
            &library_path,
            WriteKind::TableRenamed {
                old: old_name,
                new: new_trimmed,
            },
            move |adapter| {
                adapter
                    .rename_table(&old, &new, &format!("rename table {old} → {new}"))
                    .map(|()| None)
            },
        )
    }

    /// The table rename finished.
    pub(in crate::app::dispatch::library) fn table_renamed(
        &mut self,
        library_path: std::path::PathBuf,
        old_name: String,
        new_name: String,
        outcome: Result<Option<u64>, WriteFailure>,
    ) -> Task<Message> {
        if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
            s.rename_saving = false;
        }
        if let Err(failure) = outcome {
            tracing::error!(
                target: "signex::library",
                path = %library_path.display(),
                error = %failure.reason,
                "rename table failed; library left unchanged"
            );
            if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
                s.rename_error = Some(failure.reason);
            }
            return Task::none();
        }
        let refresh = self.refresh_library_task(&library_path, RefreshFollowUp::Nothing);
        if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
            s.renaming_table = None;
            s.rename_error = None;
            if s.active_table.as_deref() == Some(old_name.as_str()) {
                s.active_table = Some(new_name);
            }
        }
        refresh
    }

    /// Confirm `+ Add Table` — calls `create_empty_table` on the
    /// adapter, refreshes the browser cache, switches the active
    /// tab to the new table.
    pub(in crate::app::dispatch::library) fn handle_browser_confirm_add_table(
        &mut self,
        library_path: std::path::PathBuf,
    ) -> Task<Message> {
        if self.refuse_read_only_write(&library_path) {
            return Task::none();
        }
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
        if draft.saving {
            return Task::none();
        }
        if let Some(s) = self.library.library_browsers.get_mut(&library_path)
            && let Some(d) = s.adding_table.as_mut()
        {
            d.saving = true;
            d.error = None;
        }
        let name = trimmed.clone();
        write_task(
            &mut self.library,
            &library_path,
            WriteKind::TableCreated { name: trimmed },
            move |adapter| {
                adapter
                    .create_empty_table(&name, &format!("create empty table {name}"))
                    .map(|()| None)
            },
        )
    }

    /// The table create finished.
    pub(in crate::app::dispatch::library) fn table_created(
        &mut self,
        library_path: std::path::PathBuf,
        name: String,
        outcome: Result<Option<u64>, WriteFailure>,
    ) -> Task<Message> {
        if let Err(failure) = outcome {
            tracing::error!(
                target: "signex::library",
                path = %library_path.display(),
                error = %failure.reason,
                "create table failed; library left unchanged"
            );
            if let Some(s) = self.library.library_browsers.get_mut(&library_path)
                && let Some(d) = s.adding_table.as_mut()
            {
                d.saving = false;
                d.error = Some(failure.reason);
            }
            return Task::none();
        }
        let refresh = self.refresh_library_task(&library_path, RefreshFollowUp::Nothing);
        if let Some(s) = self.library.library_browsers.get_mut(&library_path) {
            s.adding_table = None;
            s.active_table = Some(name);
        }
        refresh
    }
}
