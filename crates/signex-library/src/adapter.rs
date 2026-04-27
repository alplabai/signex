//! `LibraryAdapter` — the trait every storage flavour implements.
//!
//! Per `v0.9-refactor-2-plan.md` §7, the trait is row-shaped: the legacy
//! `get_component` / `get_revision` / `save_revision` methods are gone,
//! replaced by table CRUD that targets [`ComponentRow`] (the DBLib model).
//! Every adapter — LocalGit (TSV) or Database (JSONB) — answers the same
//! row-oriented surface.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::component::ComponentRow;
use crate::identity::{InternalPn, RowId};
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

/// One result row from a library query — header info derived from a
/// [`ComponentRow`].
///
/// Used to be tied to the per-component head revision; now it's just a
/// thin projection of a row's display fields. Kept around for UI grids
/// that don't want to materialise full row payloads.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComponentSummary {
    pub row_id: Uuid,
    pub internal_pn: InternalPn,
    pub mpn: String,
    pub state: LifecycleState,
    pub description: String,
}

/// Header row for a primitive listing — name + uuid + kind tag, plus a hint
/// of how many rows depend on it (for the library editor's "in use" badge).
/// The `used_by_count` is a snapshot the adapter computes from its own
/// state; it's not authoritative across an open `LibrarySet` (resolver
/// aggregation is the caller's job).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrimitiveSummary {
    pub uuid: Uuid,
    pub name: String,
    pub kind: PrimitiveKind,
    #[serde(default)]
    pub used_by_count: usize,
}

/// Storage backend abstraction. All flavours (LocalGit, Database, Plm)
/// implement this.
///
/// **Default impls:** every method has a `LibraryError::Backend("not impl")`
/// default so individual adapters can override only the surface they
/// actually support. WS-2 (LocalGit) and WS-3 (Database) supply real
/// implementations; this WS (WS-1) ships only the trait shape.
pub trait LibraryAdapter: Send + Sync {
    /// Stable UUID of this library, sourced from `library.toml::library.library_id`.
    ///
    /// Used by [`crate::adapters::library_set::LibrarySet`] to key resolution
    /// of cross-library [`crate::primitive::PrimitiveRef`]s. The default
    /// implementation pulls it from `manifest().library.library_id` so any
    /// adapter whose manifest is honest gets it for free.
    fn library_id(&self) -> Uuid {
        self.manifest().library.library_id
    }

    fn manifest(&self) -> &Manifest;

    // ── Tables (WS-2 / WS-3) ────────────────────────────────────────────
    //
    // The unit of storage is now a row inside a category table. WS-2 lands
    // the LocalGit (TSV) implementation; WS-3 lands the Database (JSONB)
    // implementation. Until then every method default-errors with
    // `Backend("not impl")` so adapter authors can layer in pieces.

    /// List the names of every table this library exposes (filename stem,
    /// no extension).
    fn list_tables(&self) -> Result<Vec<String>, LibraryError> {
        Err(LibraryError::Backend(
            "list_tables not implemented for this adapter".into(),
        ))
    }

    /// Read every row from the named table.
    fn read_table(&self, _name: &str) -> Result<Vec<ComponentRow>, LibraryError> {
        Err(LibraryError::Backend(
            "read_table not implemented for this adapter".into(),
        ))
    }

    /// Iterate every row across every table — `(table_name, row)` pairs.
    /// Used by `WhereUsedIndex::rebuild_from_rows` and the search index.
    fn iter_rows(&self) -> Result<Vec<(String, ComponentRow)>, LibraryError> {
        Err(LibraryError::Backend(
            "iter_rows not implemented for this adapter".into(),
        ))
    }

    // ── Row CRUD ────────────────────────────────────────────────────────

    fn read_row(&self, _table: &str, _row_id: RowId) -> Result<ComponentRow, LibraryError> {
        Err(LibraryError::Backend(
            "read_row not implemented for this adapter".into(),
        ))
    }

    fn read_row_by_pn(&self, _pn: &InternalPn) -> Result<(String, ComponentRow), LibraryError> {
        Err(LibraryError::Backend(
            "read_row_by_pn not implemented for this adapter".into(),
        ))
    }

    fn insert_row(
        &self,
        _table: &str,
        _row: ComponentRow,
        _msg: &str,
    ) -> Result<(), LibraryError> {
        Err(LibraryError::Backend(
            "insert_row not implemented for this adapter".into(),
        ))
    }

    fn update_row(
        &self,
        _table: &str,
        _row: ComponentRow,
        _msg: &str,
    ) -> Result<(), LibraryError> {
        Err(LibraryError::Backend(
            "update_row not implemented for this adapter".into(),
        ))
    }

    fn delete_row(
        &self,
        _table: &str,
        _row_id: RowId,
        _msg: &str,
    ) -> Result<(), LibraryError> {
        Err(LibraryError::Backend(
            "delete_row not implemented for this adapter".into(),
        ))
    }

    // ── Locks (advisory) ────────────────────────────────────────────────

    fn try_lock(&self, _row_id: RowId, _field_set: FieldSet) -> Result<(), LibraryError> {
        Err(LibraryError::Backend(
            "try_lock not implemented for this adapter".into(),
        ))
    }

    fn release_lock(&self, _row_id: RowId, _field_set: FieldSet) -> Result<(), LibraryError> {
        Err(LibraryError::Backend(
            "release_lock not implemented for this adapter".into(),
        ))
    }

    // ── Primitive CRUD (unchanged from v0.9-original) ───────────────────
    //
    // Symbols, footprints, sim models stay as standalone editable primitive
    // files (v0.9-refactor-2 only changes the *component* storage; primitives
    // are already row-shaped). Default impls error so adapters layer in
    // their own implementation when ready.

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
