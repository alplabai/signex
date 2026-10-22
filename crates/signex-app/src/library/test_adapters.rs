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
