//! Part-number uniqueness for the database backend, matching the local git
//! backend's `check_pn_unique`: a PN is the library-wide key, compared
//! trimmed and case-sensitively, and a blank PN (an unfilled draft) never
//! conflicts.
//!
//! The server stores rows by (library_id, table_name, row_id) only, so the
//! adapter checks against the rows it reads back before a write. Two writers
//! racing the same new PN can still both pass; closing that needs a server
//! side index, which cannot be added while existing libraries may hold
//! duplicates.

use uuid::Uuid;

use super::DatabaseAdapter;
use crate::adapter::{LibraryAdapter, LibraryError};
use crate::component::ComponentRow;

/// Refuse `internal_pn` if a row other than `except_row_id` already holds it.
pub(super) fn ensure_pn_free(
    rows: &[(String, ComponentRow)],
    internal_pn: &str,
    except_row_id: Option<Uuid>,
) -> Result<(), LibraryError> {
    let wanted = internal_pn.trim();
    if wanted.is_empty() {
        return Ok(());
    }
    for (table, stored) in rows {
        if Some(stored.row_id) != except_row_id && stored.internal_pn.as_str().trim() == wanted {
            return Err(LibraryError::Conflict(format!(
                "internal_pn {internal_pn:?} already exists in table {table:?}"
            )));
        }
    }
    Ok(())
}

impl DatabaseAdapter {
    pub(super) fn check_pn_free_for_insert(&self, row: &ComponentRow) -> Result<(), LibraryError> {
        if row.internal_pn.as_str().trim().is_empty() {
            return Ok(());
        }
        ensure_pn_free(&self.iter_rows()?, row.internal_pn.as_str(), None)
    }

    /// Only checked when the PN differs (trimmed) from the stored row's, so
    /// a library that already holds duplicate PNs still lets their other
    /// fields be edited.
    pub(super) fn check_pn_free_for_update(
        &self,
        table: &str,
        row: &ComponentRow,
    ) -> Result<(), LibraryError> {
        if row.internal_pn.as_str().trim().is_empty() {
            return Ok(());
        }
        match self.read_row(table, crate::identity::RowId::from_uuid(row.row_id)) {
            Ok(stored) if stored.internal_pn.as_str().trim() == row.internal_pn.as_str().trim() => {
                return Ok(());
            }
            // No stored row: the write itself reports that.
            Err(LibraryError::NotFound(_)) => return Ok(()),
            Err(e) => return Err(e),
            Ok(_) => {}
        }
        ensure_pn_free(
            &self.iter_rows()?,
            row.internal_pn.as_str(),
            Some(row.row_id),
        )
    }
}
