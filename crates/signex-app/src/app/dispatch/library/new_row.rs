//! Appending a draft row to a library table, off the UI thread (#893).
//!
//! The row is built here, in `update`, from state it already holds; the
//! `insert_row` (write lock, file rewrite, git commit) runs on a worker.
//! [`Signex::row_created`] applies the result.

use super::*;
use crate::library::commands::{self, NewComponentRow};
use crate::library::refresh::RefreshFollowUp;
use crate::library::writes::{AfterRowCreated, WriteFailure, WriteKind, write_task};

impl Signex {
    /// Start inserting a draft row, with an empty part number, into `table`
    /// of the library at `library_idx`. `then` is what happens when it has
    /// landed.
    pub(super) fn create_row_task(
        &mut self,
        library_idx: usize,
        table: &str,
        class: signex_library::ComponentClass,
        then: AfterRowCreated,
    ) -> Task<Message> {
        let built =
            commands::build_component_row(&self.library, library_idx, table, "", class, None, None);
        let NewComponentRow {
            library_root,
            table,
            row_id,
            row,
            commit_message,
            unbound,
        } = match built {
            Ok(new) => new,
            Err(e) => {
                crate::diagnostics::log_warning(format!(
                    "New component was not added to table {table}: {e}"
                ));
                self.sync_diagnostics_panel_ctx();
                return Task::none();
            }
        };
        let kind = WriteKind::RowCreated {
            table: table.clone(),
            row_id,
            internal_pn: row.internal_pn.as_str().to_string(),
            unbound: unbound.iter().map(|s| s.to_string()).collect(),
            then,
        };
        write_task(&mut self.library, &library_root, kind, move |adapter| {
            adapter
                .insert_row(&table, row, &commit_message)
                .map(|()| None)
        })
    }

    /// The insert finished.
    pub(super) fn row_created(
        &mut self,
        library_path: std::path::PathBuf,
        created: CreatedRow,
        outcome: Result<Option<u64>, WriteFailure>,
    ) -> Task<Message> {
        let CreatedRow {
            table,
            row_id,
            internal_pn,
            unbound,
            then,
        } = created;
        if let Err(failure) = outcome {
            crate::diagnostics::log_warning(format!(
                "New component was not added to table {table}: {} ({})",
                failure.reason,
                library_path.display()
            ));
            self.sync_diagnostics_panel_ctx();
            return Task::none();
        }
        commands::report_unbound_row(&internal_pn, &unbound);
        match then {
            AfterRowCreated::OpenPreview => {
                let refresh = self.refresh_library_task(&library_path, RefreshFollowUp::Nothing);
                Task::batch([
                    refresh,
                    Task::done(Message::Library(LibraryMessage::OpenComponentRow {
                        library_path,
                        table,
                        row_id,
                    })),
                ])
            }
            AfterRowCreated::SelectInBrowser => {
                if let Some(state) = self.library.library_browsers.get_mut(&library_path) {
                    state.active_table = Some(table.clone());
                    state.selected_row = Some(row_id);
                }
                tracing::info!(
                    target: "signex::library",
                    library = %library_path.display(),
                    table = %table,
                    row_id = %row_id,
                    "browser: minted draft row inline"
                );
                // The new row may sort outside the built window: scroll to
                // it once the refresh has brought it into the cache.
                self.refresh_library_task(&library_path, RefreshFollowUp::RevealSelected)
            }
        }
    }
}

/// What a created row carries from its [`WriteKind::RowCreated`].
pub(super) struct CreatedRow {
    pub table: String,
    pub row_id: RowId,
    pub internal_pn: String,
    pub unbound: Vec<String>,
    pub then: AfterRowCreated,
}
