//! Test-only [`LibraryAdapter`] stubs.
//!
//! Shared by the cache-refresh regressions in `library::state` and in
//! `app::dispatch::library::editor`, which both need a mounted library
//! whose primitive listings fail.

use signex_library::{
    LibraryAdapter, LibraryError, LibraryMeta, LibraryMode, Manifest, PrimitiveSummary,
    UsersConfig, WorkflowConfig,
};
use uuid::Uuid;

/// A mounted library whose three primitive listings always fail.
///
/// Models the reachable cases — a `symbols/` directory that cannot be
/// read, a library server returning 500 — as the `LibraryError` those
/// paths actually produce. `list_tables` succeeds with no tables so
/// `refresh_components` reaches the primitive listings.
pub(crate) struct FailingListingAdapter {
    manifest: Manifest,
}

impl FailingListingAdapter {
    pub(crate) fn new(library_id: Uuid) -> Self {
        Self {
            manifest: Manifest {
                library: LibraryMeta {
                    name: "FailingLib".to_string(),
                    library_id,
                    description: None,
                },
                mode: LibraryMode::default(),
                workflow: WorkflowConfig::default(),
                users: UsersConfig::default(),
                tables: Vec::new(),
            },
        }
    }

    fn listing_failure() -> LibraryError {
        LibraryError::Backend("listing unavailable".to_string())
    }
}

impl LibraryAdapter for FailingListingAdapter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn list_tables(&self) -> Result<Vec<String>, LibraryError> {
        Ok(Vec::new())
    }

    fn list_symbols(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        Err(Self::listing_failure())
    }

    fn list_footprints(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        Err(Self::listing_failure())
    }

    fn list_sims(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        Err(Self::listing_failure())
    }
}

/// An adapter that counts every call a `view` could use to reach the
/// disk. `manifest()` is deliberately uncounted: it is a borrow of the
/// in-memory header and does no IO.
pub(crate) struct CountingAdapter {
    manifest: Manifest,
    calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl CountingAdapter {
    /// The adapter plus a handle on its counter — the adapter itself is
    /// boxed into a `LibrarySet` and cannot be asked afterwards.
    pub(crate) fn new(library_id: Uuid) -> (Self, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let adapter = Self {
            manifest: FailingListingAdapter::new(library_id).manifest,
            calls: calls.clone(),
        };
        (adapter, calls)
    }

    fn count(&self) {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

impl LibraryAdapter for CountingAdapter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn list_tables(&self) -> Result<Vec<String>, LibraryError> {
        self.count();
        Ok(Vec::new())
    }

    fn library_classes(&self) -> Vec<signex_library::ClassEntry> {
        self.count();
        Vec::new()
    }

    fn read_table(&self, _name: &str) -> Result<Vec<signex_library::ComponentRow>, LibraryError> {
        self.count();
        Ok(Vec::new())
    }

    fn list_symbols(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        self.count();
        Ok(Vec::new())
    }

    fn list_footprints(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        self.count();
        Ok(Vec::new())
    }

    fn list_sims(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        self.count();
        Ok(Vec::new())
    }
}

/// An adapter whose symbol listing is whatever the test last put behind
/// the handle — lets a test tell two reads of the same library apart.
pub(crate) struct ScriptedSymbolsAdapter {
    manifest: Manifest,
    symbols: std::sync::Arc<std::sync::Mutex<Vec<PrimitiveSummary>>>,
}

impl ScriptedSymbolsAdapter {
    pub(crate) fn new(
        library_id: Uuid,
    ) -> (
        Self,
        std::sync::Arc<std::sync::Mutex<Vec<PrimitiveSummary>>>,
    ) {
        let symbols = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let adapter = Self {
            manifest: FailingListingAdapter::new(library_id).manifest,
            symbols: symbols.clone(),
        };
        (adapter, symbols)
    }
}

impl LibraryAdapter for ScriptedSymbolsAdapter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn list_tables(&self) -> Result<Vec<String>, LibraryError> {
        Ok(Vec::new())
    }

    fn list_symbols(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        Ok(self.symbols.lock().expect("symbols lock").clone())
    }

    fn list_footprints(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        Ok(Vec::new())
    }

    fn list_sims(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        Ok(Vec::new())
    }
}

/// A `PrimitiveSummary` standing in for an entry already on disk and
/// already cached — the thing a failed listing used to erase.
pub(crate) fn cached_summary(name: &str) -> PrimitiveSummary {
    PrimitiveSummary {
        uuid: Uuid::new_v4(),
        name: name.to_string(),
        kind: signex_library::PrimitiveKind::Symbol,
        used_by_count: 0,
    }
}

/// A server-backed library in miniature: it holds one revision counter and,
/// like the library server, writes a row only when the caller's
/// `server_revision` matches it, bumping it on every update.
pub(crate) struct RevisionedRowAdapter {
    manifest: Manifest,
    revision: std::sync::atomic::AtomicU64,
    /// The stored row, as `read_row` and `iter_rows` return it.
    row: std::sync::Mutex<Option<signex_library::ComponentRow>>,
}

impl RevisionedRowAdapter {
    pub(crate) fn new(library_id: Uuid, revision: u64) -> Self {
        Self {
            manifest: FailingListingAdapter::new(library_id).manifest,
            revision: std::sync::atomic::AtomicU64::new(revision),
            row: std::sync::Mutex::new(None),
        }
    }

    /// Store `row` at the current revision, as another user's save would.
    pub(crate) fn store(&self, mut row: signex_library::ComponentRow) {
        row.server_revision = Some(self.revision.load(std::sync::atomic::Ordering::SeqCst));
        *self.row.lock().expect("stub row") = Some(row);
    }

    /// Advance the stored revision if it is still `base`.
    fn bump_from(&self, base: Option<u64>) -> Result<u64, LibraryError> {
        let current = self.revision.load(std::sync::atomic::Ordering::SeqCst);
        if base != Some(current) {
            return Err(LibraryError::RowChanged {
                current_revision: current,
            });
        }
        self.revision
            .store(current + 1, std::sync::atomic::Ordering::SeqCst);
        Ok(current + 1)
    }
}

impl LibraryAdapter for RevisionedRowAdapter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn list_tables(&self) -> Result<Vec<String>, LibraryError> {
        Ok(vec!["R".to_string()])
    }

    fn read_table(&self, _name: &str) -> Result<Vec<signex_library::ComponentRow>, LibraryError> {
        Ok(self
            .row
            .lock()
            .expect("stub row")
            .clone()
            .into_iter()
            .collect())
    }

    fn list_symbols(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        Ok(Vec::new())
    }

    fn list_footprints(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        Ok(Vec::new())
    }

    fn list_sims(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        Ok(Vec::new())
    }

    fn update_row(
        &self,
        _table: &str,
        row: signex_library::ComponentRow,
        _msg: &str,
    ) -> Result<Option<u64>, LibraryError> {
        let revision = self.bump_from(row.server_revision)?;
        self.store(row);
        Ok(Some(revision))
    }

    fn read_row(
        &self,
        _table: &str,
        row_id: signex_library::RowId,
    ) -> Result<signex_library::ComponentRow, LibraryError> {
        self.row
            .lock()
            .expect("stub row")
            .clone()
            .filter(|row| signex_library::RowId::from_uuid(row.row_id) == row_id)
            .ok_or_else(|| LibraryError::NotFound(row_id.to_string()))
    }

    fn iter_rows(&self) -> Result<Vec<(String, signex_library::ComponentRow)>, LibraryError> {
        Ok(self
            .row
            .lock()
            .expect("stub row")
            .clone()
            .map(|row| vec![("R".to_string(), row)])
            .unwrap_or_default())
    }

    fn delete_row(
        &self,
        _table: &str,
        _row_id: signex_library::RowId,
        base_revision: Option<u64>,
        _msg: &str,
    ) -> Result<(), LibraryError> {
        self.bump_from(base_revision).map(|_| ())
    }
}

/// One table `"R"` with no rows but one cached-price entry, to observe
/// whether a reload carries prices onto the mounted library.
pub(crate) struct PricedAdapter {
    manifest: Manifest,
    pub(crate) priced_row: Uuid,
    rows: Vec<signex_library::ComponentRow>,
}

impl PricedAdapter {
    pub(crate) fn new(library_id: Uuid) -> Self {
        Self {
            manifest: FailingListingAdapter::new(library_id).manifest,
            priced_row: Uuid::new_v4(),
            rows: Vec::new(),
        }
    }

    /// Serve `rows` from the one table.
    pub(crate) fn with_rows(mut self, rows: Vec<signex_library::ComponentRow>) -> Self {
        self.rows = rows;
        self
    }
}

impl LibraryAdapter for PricedAdapter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn list_tables(&self) -> Result<Vec<String>, LibraryError> {
        Ok(vec!["R".to_string()])
    }

    fn read_table(&self, _name: &str) -> Result<Vec<signex_library::ComponentRow>, LibraryError> {
        Ok(self.rows.clone())
    }

    fn table_pricing(
        &self,
        _table: &str,
    ) -> Result<std::collections::BTreeMap<Uuid, signex_library::RowPricing>, LibraryError> {
        Ok([(self.priced_row, signex_library::RowPricing::new())].into())
    }
}
