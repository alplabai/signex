//! Part-number uniqueness for the database backend, matching the local git
//! backend's `check_pn_unique`: a PN is the library-wide key, compared
//! trimmed and case-sensitively, and a blank PN (an unfilled draft) never
//! conflicts.
//!
//! The adapter checks against the rows it reads back before a write, which
//! gives a clear error early. Two writers racing the same new PN can both
//! pass that check; the server's unique index on the trimmed PN per library
//! is what decides the race, and its 409 comes back as the same
//! `LibraryError::Conflict`.

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
    /// saving a row without changing its PN never trips on the check (a
    /// local checkout or an older server may still hold duplicates).
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
