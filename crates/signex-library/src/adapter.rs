//! `LibraryAdapter` — the trait every storage flavour implements.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::component::{Component, Revision};
use crate::identity::{ComponentId, InternalPn, Version};
use crate::lifecycle::LifecycleState;
use crate::manifest::Manifest;

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("locked by {holder}: {field_set}")]
    Locked { holder: String, field_set: String },
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("backend: {0}")]
    Backend(String),
}

/// Field-sets per LIBRARY_PLAN §8 — locking granularity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FieldSet {
    Symbol,
    Footprint,
    Model3d,
    SharedParams,
    SharedSupplyChain,
    SharedSimulation,
    Lifecycle,
}

/// A query into the library — partial match on internal_pn or mpn, plus facets.
#[derive(Clone, Debug, Default)]
pub struct LibraryQuery {
    pub text: Option<String>,
    pub category: Option<String>,
    pub facets: Vec<(String, String)>,
}

/// One result row from a library query — header info, NOT full revisions.
///
/// M5: `internal_pn` is `InternalPn`, matching the rest of the identity layer.
/// `serde(transparent)` keeps the wire format a bare string, so existing
/// payloads round-trip unchanged.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComponentSummary {
    pub uuid: ComponentId,
    pub internal_pn: InternalPn,
    pub mpn: String,
    pub head: Version,
    pub state: LifecycleState,
    pub description: String,
}

/// Storage backend abstraction. All flavours (LocalGit, Database, Plm) implement this.
pub trait LibraryAdapter: Send + Sync {
    fn manifest(&self) -> &Manifest;

    fn search(&self, query: &LibraryQuery) -> Result<Vec<ComponentSummary>, LibraryError>;

    fn get_component(&self, id: ComponentId) -> Result<Component, LibraryError>;

    fn get_revision(&self, id: ComponentId, version: Version) -> Result<Revision, LibraryError>;

    /// Save a new revision. Backend chooses commit vs review-request based on workflow.
    fn save_revision(
        &self,
        id: ComponentId,
        revision: Revision,
        message: &str,
    ) -> Result<(), LibraryError>;

    fn try_lock(&self, id: ComponentId, field_set: FieldSet) -> Result<(), LibraryError>;

    fn release_lock(&self, id: ComponentId, field_set: FieldSet) -> Result<(), LibraryError>;

    /// For local-git, the `.snxlib/` directory; for DB, `None`.
    fn root_path(&self) -> Option<PathBuf> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_adapter_is_object_safe() {
        // Compile-time check — if this compiles, the trait is dyn-compatible.
        fn _accepts_dyn(_a: &dyn LibraryAdapter) {}
    }

    #[test]
    fn library_query_default_is_empty() {
        let q = LibraryQuery::default();
        assert!(q.text.is_none());
        assert!(q.facets.is_empty());
    }
}
