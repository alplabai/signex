//! `LibraryAdapter` — the trait every storage flavour implements.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::component::{Component, Revision};
use crate::identity::{ComponentId, InternalPn, Version};
use crate::lifecycle::LifecycleState;
use crate::manifest::Manifest;
use crate::primitive::{Footprint, PrimitiveKind, SimModel, Symbol};

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

/// Header row for a primitive listing — name + uuid + kind tag, plus a hint
/// of how many components depend on it (for the library editor's "in use"
/// badge). The `used_by_count` is a snapshot the adapter computes from its
/// own state; it's not authoritative across an open `LibrarySet` (resolver
/// aggregation is the caller's job).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrimitiveSummary {
    pub uuid: Uuid,
    pub name: String,
    pub kind: PrimitiveKind,
    #[serde(default)]
    pub used_by_count: usize,
}

/// Storage backend abstraction. All flavours (LocalGit, Database, Plm) implement this.
pub trait LibraryAdapter: Send + Sync {
    /// Stable UUID of this library, sourced from `library.toml::library.library_id`.
    ///
    /// Used by [`crate::adapters::library_set::LibrarySet`] to key resolution
    /// of cross-library [`crate::primitive::PrimitiveRef`]s. The default
    /// implementation pulls it from `manifest().library.library_id` so any
    /// adapter whose manifest is honest gets it for free; adapters that
    /// fabricate a placeholder manifest (e.g. remote DB shim before login)
    /// should override.
    fn library_id(&self) -> Uuid {
        self.manifest().library.library_id
    }

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

    // ── Primitive CRUD (WS-C) ────────────────────────────────────────────
    //
    // Reusable shape primitives addressed by the adapter's `library_id` plus
    // a primitive UUID. Default impls return `LibraryError::Backend` so older
    // adapters compile while WS-D / WS-E / WS-F / WS-G / WS-H land — every
    // production adapter SHOULD override.

    fn get_symbol(&self, _uuid: Uuid) -> Result<Symbol, LibraryError> {
        Err(LibraryError::Backend(
            "get_symbol not implemented for this adapter".into(),
        ))
    }

    fn get_footprint(&self, _uuid: Uuid) -> Result<Footprint, LibraryError> {
        Err(LibraryError::Backend(
            "get_footprint not implemented for this adapter".into(),
        ))
    }

    fn get_sim(&self, _uuid: Uuid) -> Result<SimModel, LibraryError> {
        Err(LibraryError::Backend(
            "get_sim not implemented for this adapter".into(),
        ))
    }

    fn save_symbol(&self, _sym: Symbol, _message: &str) -> Result<(), LibraryError> {
        Err(LibraryError::Backend(
            "save_symbol not implemented for this adapter".into(),
        ))
    }

    fn save_footprint(&self, _fp: Footprint, _message: &str) -> Result<(), LibraryError> {
        Err(LibraryError::Backend(
            "save_footprint not implemented for this adapter".into(),
        ))
    }

    fn save_sim(&self, _sm: SimModel, _message: &str) -> Result<(), LibraryError> {
        Err(LibraryError::Backend(
            "save_sim not implemented for this adapter".into(),
        ))
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
