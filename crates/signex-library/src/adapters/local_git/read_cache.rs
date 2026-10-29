//! Row reads that convert only what they need (#1437).
//!
//! Turning a stored row into a `ComponentRow` costs a CSV record plus a
//! SHA-256 over its cells. `read_row` converts the one row asked for.
//! `snapshot_table` keeps what it converted, so a read after a one-row edit
//! converts one row, not the table.
//!
//! No invalidation to get wrong: an entry holds the `Arc<LibraryRow>` it
//! was converted from and is valid only while the table still holds that
//! same `Arc`. A write replaces the row's `Arc` and a reload from disk
//! replaces every one, so a changed row always misses. Holding the `Arc`
//! also stops its address being reused and forces `Arc::make_mut` to copy,
//! so a pointer match means the same cells.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

struct Cached {
    /// Held only so the key's address cannot be reused while the entry
    /// lives; the key is what is compared.
    _source: Arc<LibraryRow>,
    component: ComponentRow,
}

/// Per table, converted rows keyed by the address of their source `Arc`.
#[derive(Default)]
pub(super) struct RowReads {
    tables: Mutex<HashMap<String, HashMap<usize, Cached>>>,
    conversions: AtomicUsize,
}

impl std::fmt::Debug for RowReads {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RowReads").finish_non_exhaustive()
    }
}

impl RowReads {
    /// One stored row as a `ComponentRow`, with its base revision.
    pub(super) fn convert(&self, row: &LibraryRow) -> Result<ComponentRow, LibraryError> {
        self.conversions.fetch_add(1, Ordering::Relaxed);
        let mut component = library_row_to_component(row)?;
        component.server_revision = Some(row_base(row));
        Ok(component)
    }

    /// How many rows were converted so far; tests assert on the difference.
    #[cfg(test)]
    pub(super) fn conversions(&self) -> usize {
        self.conversions.load(Ordering::Relaxed)
    }

    /// Every row of `rows` as a `ComponentRow`: rows still holding the
    /// `Arc` they were converted from are served as kept, the rest are
    /// converted. What is kept is exactly the rows of `rows`, so rows a
    /// write replaced or removed are dropped here.
    pub(super) fn table(
        &self,
        name: &str,
        rows: &[Arc<LibraryRow>],
    ) -> Result<Vec<ComponentRow>, LibraryError> {
        let mut tables = self.tables.lock().unwrap_or_else(|e| e.into_inner());
        let mut previous = tables.remove(name).unwrap_or_default();
        let mut kept: HashMap<usize, Cached> = HashMap::with_capacity(rows.len());
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let key = Arc::as_ptr(row) as usize;
            let cached = match previous.remove(&key) {
                Some(hit) => hit,
                None => Cached {
                    _source: Arc::clone(row),
                    component: self.convert(row)?,
                },
            };
            out.push(cached.component.clone());
            kept.insert(key, cached);
        }
        tables.insert(name.to_string(), kept);
        Ok(out)
    }

    /// Forget every table not in `keep`: a renamed or removed table.
    pub(super) fn retain_tables<'a>(&self, keep: impl Iterator<Item = &'a String>) {
        let keep: std::collections::HashSet<&String> = keep.collect();
        self.tables
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|name, _| keep.contains(name));
    }

    /// Drop everything kept: the rows it was converted from were replaced
    /// by a reload, so nothing in it can hit again.
    pub(super) fn clear(&self) {
        self.tables
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    /// Names of the tables with kept rows, sorted.
    #[cfg(test)]
    pub(super) fn kept_tables(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .tables
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .cloned()
            .collect();
        names.sort();
        names
    }
}

impl LocalGitAdapter {
    /// The row `row_id` of `table`, converting that row only.
    pub(super) fn read_component_row(
        &self,
        table: &str,
        row_id: RowId,
    ) -> Result<ComponentRow, LibraryError> {
        self.refresh_for_read();
        let guard = self
            .library_file
            .read()
            .map_err(|_| LibraryError::Backend("library_file read lock poisoned".into()))?;
        let wanted = row_id.as_uuid();
        let target = wanted.to_string();
        let missing = || LibraryError::NotFound(format!("row {row_id} in table {table}"));
        let entry = guard.tables.get(table).ok_or_else(missing)?;
        validate_legacy_header(table, &entry.columns)?;
        let row = entry
            .rows
            .iter()
            .find(|r| row_has_id(r, wanted, &target))
            .ok_or_else(missing)?;
        self.row_reads.convert(row)
    }
}

/// Whether the stored row is `wanted`. The id is compared as a UUID, so a
/// hand-edited uppercase or unhyphenated cell that `read_table` shows is
/// found too; the plain string match is the fast path.
fn row_has_id(row: &LibraryRow, wanted: uuid::Uuid, wanted_text: &str) -> bool {
    let Some(cell) = row.cells.get(LEGACY_ROW_ID_COL) else {
        return false;
    };
    if cell == wanted_text {
        return true;
    }
    uuid::Uuid::parse_str(cell).is_ok_and(|id| id == wanted)
}
