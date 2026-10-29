//! The row write (#416): saving an insert, update, delete or version bind of
//! rows in one table without re-checking the whole library.
//!
//! The full write validates every cell, prunes every cached price and reads
//! its whole output back. When memory holds what this adapter last wrote,
//! and no load from disk happened since, all of that already passed for
//! every row but the edited one. The row write then checks only the edited
//! row (the same checks, the same errors), prunes only its id's prices, and
//! encodes the whole document, taking unchanged rows' lines from the last
//! row write (`RowLines`); the save protocol around it (atomic write,
//! memory after the rename, one commit) is the same. Anything else takes
//! the full write: the first edit after a load, a new table, a new column,
//! or a file that keeps unknown keys (#1435), whose text only the full
//! write edits in place.
//!
//! Debug builds also run the full write on the same state and assert both
//! give the same bytes, or the same refusal.

use super::helpers::*;
use super::*;

/// What a row edit did to its table.
pub(super) enum RowChange {
    /// Nothing: no write, no commit.
    None,
    /// The row at this index was added or replaced.
    Wrote(usize),
    /// Every row with this id was removed.
    Removed(String),
}

#[cfg(test)]
thread_local! {
    /// Row writes taken on this thread, so tests can tell the paths apart.
    pub(super) static ROW_WRITES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl LocalGitAdapter {
    /// Save `edit`, a change to rows of `table` only, through the row write
    /// when it applies (see the module doc) and the full write otherwise.
    pub(super) fn save_row_edit<F>(
        &self,
        table: &str,
        edit: F,
        asset_rel_paths: &[String],
        message: &str,
        fallback: &str,
    ) -> Result<(), LibraryError>
    where
        F: FnOnce(&mut LibraryFile) -> Result<RowChange, LibraryError>,
    {
        self.save_edit(
            |lf, holds_own_write| {
                let columns_before = lf.tables.get(table).map(|t| t.columns.clone());
                let change = edit(lf)?;
                let (changed, id) = match change {
                    RowChange::None => return Ok(None),
                    RowChange::Wrote(index) => (Some(index), row_id_at(lf, table, index)),
                    RowChange::Removed(id) => (None, Some(id)),
                };
                let same_shape =
                    columns_before.as_ref() == lf.tables.get(table).map(|t| &t.columns);
                // A file with keys this build does not model keeps its text,
                // which only the full write edits in place (#1435).
                let canonical = !lf.layout.keeps_text();
                let Some(id) = id.filter(|_| holds_own_write && same_shape && canonical) else {
                    prune_stale_pricing(lf);
                    let text = lf.write_and_rebase()?;
                    self.row_lines().retain_rows(lf);
                    return Ok(Some(text));
                };
                if let Some(entry) = lf.tables.get_mut(table) {
                    prune_pricing_of(entry, &id);
                }
                let text = lf.write_row_edit(table, changed, &mut self.row_lines());
                #[cfg(debug_assertions)]
                assert_same_as_full_write(lf, &text);
                #[cfg(test)]
                ROW_WRITES.with(|n| n.set(n.get() + 1));
                Ok(Some(text?))
            },
            asset_rel_paths,
            &|| message.to_string(),
            fallback,
        )
        .map(drop)
    }
}

impl LocalGitAdapter {
    /// The row-line cache. A panic while it was held leaves it poisoned;
    /// it is only a cache, so it is emptied and used again rather than
    /// failing every later row write.
    pub(super) fn row_lines(&self) -> std::sync::MutexGuard<'_, crate::library_file::RowLines> {
        self.row_lines.lock().unwrap_or_else(|poisoned| {
            self.row_lines.clear_poison();
            let mut lines = poisoned.into_inner();
            lines.clear();
            lines
        })
    }
}

fn row_id_at(lf: &LibraryFile, table: &str, index: usize) -> Option<String> {
    lf.tables
        .get(table)?
        .rows
        .get(index)?
        .cells
        .get(LEGACY_ROW_ID_COL)
        .cloned()
}

/// [`prune_stale_pricing`] for the prices of one row id in one table, with
/// the same rules: no row with the id drops them all; otherwise the last
/// row with the id that reads as a component decides which still apply.
fn prune_pricing_of(table: &mut LibraryTable, id: &str) {
    let Some(cached) = table.pricing.get_mut(id) else {
        return;
    };
    let mut live = false;
    let mut row = None;
    for r in &table.rows {
        if r.cells.get(LEGACY_ROW_ID_COL).map(String::as_str) == Some(id) {
            live = true;
            if let Ok(component) = library_row_to_component(r) {
                row = Some(component);
            }
        }
    }
    if let Some(row) = row {
        crate::pricing::prune_row_pricing(&row, cached);
    }
    if !live || cached.is_empty() {
        table.pricing.remove(id);
    }
}

/// Debug builds: the row write must produce exactly what the full write
/// (full price prune, full validation, read-back) produces from the same
/// edited state, or refuse with the same error.
#[cfg(debug_assertions)]
fn assert_same_as_full_write(
    lf: &LibraryFile,
    row_write: &Result<String, crate::library_file::LibraryFileError>,
) {
    let mut full = lf.clone();
    prune_stale_pricing(&mut full);
    assert!(
        full == *lf,
        "the row write kept a price the full prune drops"
    );
    match (row_write, full.write()) {
        (Ok(row), Ok(full)) => assert!(*row == full, "the row write differs from the full write"),
        (Err(row), Err(full)) => assert_eq!(row.to_string(), full.to_string()),
        (row, full) => panic!("row write {row:?}, full write {full:?}"),
    }
}
