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
