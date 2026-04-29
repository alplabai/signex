//! `LibraryAdapter` — the trait every storage flavour implements.
//!
//! Per `v0.9-refactor-2-plan.md` §7, the trait is row-shaped: the legacy
//! `get_component` / `get_revision` / `save_revision` methods are gone,
//! replaced by table CRUD that targets [`ComponentRow`] (the DBLib model).
//! Every adapter — LocalGit (TSV) or Database (JSONB) — answers the same
//! row-oriented surface.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::component::ComponentRow;
use crate::identity::{InternalPn, RowId};
use crate::library_file::LibraryFile;
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

/// Field-sets per v0.9-library-plan.md §8 — locking granularity.
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

/// One entry in the per-primitive git history feed.
///
/// Per `v0.9-snxlib-as-file-plan.md` §3 ("History panel inside the
/// per-primitive editor"), the SCH Library / Footprint / Sim editors
/// and the Library Browser tab all bind a [`HistoryEntry`] list to the
/// shared `signex_widgets::history_pane::HistoryPane` widget. Stage 17
/// scaffolds the API + data shape; later stages layer the graph lane,
/// diff stats, and revert/reset affordances on top.
///
/// `parent_shas`, `files_changed`, `additions`, `deletions` are kept on
/// the struct so future polish stages (lazy diff stats, merge-graph
/// rendering) can land without a schema bump. The scaffold's
/// [`LocalGitAdapter`](crate::adapters::local_git::LocalGitAdapter)
/// implementation only fills `sha`, `author_name`, `author_email`,
/// `time`, `subject`, `body`, and `parent_shas`; the rest stay empty
/// until lazy-diff support arrives.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Full 40-char hex SHA-1 of the commit.
    pub sha: String,
    pub author_name: String,
    pub author_email: String,
    /// Author timestamp (NOT committer) — matches what `git log` shows
    /// by default. UTC.
    pub time: DateTime<Utc>,
    /// First line of the commit message.
    pub subject: String,
    /// Everything after the subject (sans the blank-line separator).
    /// Empty if the commit message is single-line.
    #[serde(default)]
    pub body: String,
    /// Parent commit SHAs — empty for the root commit, one for a
    /// linear commit, two-or-more for merges. The widget uses this to
    /// render the graph lane in later stages.
    #[serde(default)]
    pub parent_shas: Vec<String>,
    /// Files touched by this commit, relative to the library root.
    /// Empty in the scaffold; populated by lazy diff support later.
    #[serde(default)]
    pub files_changed: Vec<String>,
    /// Inserted lines/keys for this commit (lazy — 0 in the scaffold).
    #[serde(default)]
    pub additions: u32,
    /// Removed lines/keys for this commit (lazy — 0 in the scaffold).
    #[serde(default)]
    pub deletions: u32,
}

/// Storage backend abstraction. All flavours (LocalGit, Database, Plm)
/// implement this.
///
/// **Default impls:** every method has a `LibraryError::Backend("not impl")`
/// default so individual adapters can override only the surface they
/// actually support. `LocalGitAdapter` and `DatabaseAdapter` supply
/// real implementations.
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

    /// Borrow the parsed `.snxlib` view if this adapter is backed by an
    /// on-disk file. Returns `None` for non-file adapters (e.g. the DB
    /// backend) — they don't have a `[tables.<name>]` document on disk.
    ///
    /// Stage 2 introduces this hook so future adapters and callers can
    /// reach the new format without going through the legacy
    /// `manifest()` synthesis. The default `None` keeps existing
    /// adapters compiling unchanged.
    fn library_file(&self) -> Option<&LibraryFile> {
        None
    }

    /// Absolute path to the directory that *contains* the `.snxlib` file
    /// — i.e. the per-library git repo root and parent of `symbols/`,
    /// `footprints/`, `sims/`. Returns `None` for non-file adapters.
    ///
    /// Differs from [`Self::root_path`]: under v0.9, `root_path` and
    /// `root_dir` happen to coincide for `LocalGitAdapter` (both point
    /// at the directory holding the `.snxlib` file), but the new name
    /// makes the parent-of-file relationship explicit per
    /// `v0.9-snxlib-as-file-plan.md` §2 Stage B.
    fn root_dir(&self) -> Option<&std::path::Path> {
        None
    }

    /// Absolute path to the `.snxlib` file itself. `None` for non-file
    /// adapters.
    fn library_file_path(&self) -> Option<&std::path::Path> {
        None
    }

    // ── Tables ──────────────────────────────────────────────────────────
    //
    // The unit of storage is a row inside a category table. The
    // LocalGit adapter persists tables as TSV files; the Database
    // adapter forwards to `/tables` + `/rows` HTTP routes. Adapter
    // authors that haven't wired this surface yet keep the default
    // `Backend("not impl")` errors below.

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

    fn insert_row(&self, _table: &str, _row: ComponentRow, _msg: &str) -> Result<(), LibraryError> {
        Err(LibraryError::Backend(
            "insert_row not implemented for this adapter".into(),
        ))
    }

    fn update_row(&self, _table: &str, _row: ComponentRow, _msg: &str) -> Result<(), LibraryError> {
        Err(LibraryError::Backend(
            "update_row not implemented for this adapter".into(),
        ))
    }

    fn delete_row(&self, _table: &str, _row_id: RowId, _msg: &str) -> Result<(), LibraryError> {
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

    /// Stage and commit a file the caller already wrote to disk, using
    /// the adapter's own version-control surface. `abs_path` must live
    /// under [`Self::root_path`]; adapters whose `root_path` is `None`
    /// (e.g. the database backend) treat this as a no-op.
    ///
    /// Used by the standalone primitive editor tabs (`.snxsym` /
    /// `.snxfpt`): the editor writes the full container at the user's
    /// chosen path (preserving multi-symbol semantics that the
    /// adapter's per-primitive `save_*` would lose), then asks the
    /// adapter to commit so the edit lands in git history. Without
    /// this hook, standalone-tab edits would leave the working tree
    /// permanently dirty until the next `save_*` call paths.
    ///
    /// Default impl returns `Ok(())` so non-git backends transparently
    /// skip the commit step.
    fn commit_external_change(
        &self,
        _abs_path: &std::path::Path,
        _message: &str,
    ) -> Result<(), LibraryError> {
        Ok(())
    }

    /// Per-primitive git history.
    ///
    /// Returns up to 50 entries from `git log --follow --max-count 50
    /// -- <primitive_path>`, newest first. `primitive_path` may be
    /// absolute or relative to [`Self::root_dir`]; absolute paths
    /// outside `root_dir` are rejected with
    /// [`LibraryError::NotFound`].
    ///
    /// Per `v0.9-snxlib-as-file-plan.md` §3 this is the *single*
    /// hook the SCH Library / Footprint / Sim editors and the
    /// Library Browser tab call to populate the
    /// `signex_widgets::history_pane::HistoryPane` widget — there's
    /// no second code path. Adapters that aren't backed by git
    /// (e.g. the database adapter) keep the default
    /// `Backend("history not implemented")` response so the UI can
    /// gracefully degrade to "history unavailable" without aborting.
    fn history(&self, _primitive_path: &Path) -> Result<Vec<HistoryEntry>, LibraryError> {
        Err(LibraryError::Backend(
            "history not implemented for this adapter".into(),
        ))
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
