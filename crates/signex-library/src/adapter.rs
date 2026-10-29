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
    /// The stored row is no longer at the base revision the write was
    /// based on: someone else saved it in between. `current_revision` is
    /// a server counter or, for a local library, a content hash.
    #[error("changed by someone else since you read it; reload to see their version")]
    RowChanged { current_revision: u64 },
    /// Like [`Self::RowChanged`], for a symbol, footprint or sim model.
    /// `subject` names it (`symbol <uuid>`) so the caller can keep the
    /// local edit and offer save-as as well as reload.
    #[error(
        "{subject} was changed by someone else since you read it; save your edit as a new copy, \
         or reload to see their version"
    )]
    PrimitiveChanged {
        subject: String,
        current_revision: u64,
    },
    /// The version-control guard refused to start a repository in a folder
    /// that holds more than the project (#1205). The text says why.
    #[error("{0}")]
    VersionControlRefused(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("backend: {0}")]
    Backend(String),
    /// A write reached a library that is mounted read-only. A property
    /// of the mount, not of filesystem permissions — it is raised
    /// before any byte is touched, so it reads the same whether or not
    /// the directory happens to be writable.
    #[error("the library \"{library}\" ships with Signex and is not editable")]
    ReadOnly { library: String },
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

#[cfg(feature = "local-git")]
impl HistoryEntry {
    /// Project a `git2::Commit` onto an entry with no diff stats. The
    /// author timestamp is preferred over the committer's so rebases and
    /// cherry-picks do not skew the "12 minutes ago" labels; the subject
    /// is the first line and the body the rest, minus the blank separator.
    pub(crate) fn from_commit(commit: &git2::Commit<'_>) -> Self {
        let author = commit.author();
        let time =
            DateTime::<Utc>::from_timestamp(author.when().seconds(), 0).unwrap_or_else(Utc::now);
        let full = commit.message().unwrap_or_default();
        let (subject, body) = match full.split_once('\n') {
            Some((s, rest)) => (
                s.trim_end().to_string(),
                rest.trim_start_matches(['\r', '\n']).to_string(),
            ),
            None => (full.trim_end().to_string(), String::new()),
        };
        Self {
            sha: commit.id().to_string(),
            author_name: author.name().unwrap_or_default().to_string(),
            author_email: author.email().unwrap_or_default().to_string(),
            time,
            subject,
            body,
            parent_shas: commit.parent_ids().map(|id| id.to_string()).collect(),
            files_changed: Vec::new(),
            additions: 0,
            deletions: 0,
        }
    }
}

/// A primitive together with the revision this copy was read at — the
/// primitive tier's counterpart of `ComponentRow::server_revision`. The
/// caller that holds the copy keeps the revision with it and hands it back
/// as the base of its save, so the base is always "what this copy was read
/// at", never "what someone last read". `None` for a backend that does not
/// version primitives (a local library) and for a primitive being created.
#[derive(Clone, Debug, PartialEq)]
pub struct Versioned<T> {
    pub value: T,
    pub revision: Option<u64>,
}

/// Refuse a base revision on a backend that hands none out: silently
/// dropping it would turn a conditional save into an unconditional one.
fn refuse_unversioned_base(base: Option<u64>) -> Result<(), LibraryError> {
    match base {
        None => Ok(()),
        Some(_) => Err(LibraryError::Backend(
            "this library does not version primitives; save without a base revision".into(),
        )),
    }
}

/// Storage backend abstraction. All flavours (LocalGit, Database, Plm)
/// implement this.
///
/// **Default impls:** every method has a `LibraryError::Backend("not impl")`
/// default so individual adapters can override only the surface they
/// actually support. `LocalGitAdapter` and `DatabaseAdapter` supply
/// real implementations.
pub trait LibraryAdapter: Send + Sync {
    /// Stable UUID of this library, sourced from the `library_id` at the root of the `.snxlib` document.
    ///
    /// Used by [`crate::adapters::library_set::LibrarySet`] to key resolution
    /// of cross-library [`crate::primitive::PrimitiveRef`]s. The default
    /// implementation pulls it from `manifest().library.library_id` so any
    /// adapter whose manifest is honest gets it for free.
    fn library_id(&self) -> Uuid {
        self.manifest().library.library_id
    }

    fn manifest(&self) -> &Manifest;

    /// True when this library is mounted read-only (the stock library
    /// that ships with Signex). Every write entry point of such an
    /// adapter fails with [`LibraryError::ReadOnly`]; the UI uses this
    /// to disable write affordances up front. Defaults to `false`.
    fn is_read_only(&self) -> bool {
        false
    }

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

    /// Re-point a file-backed adapter at `new_file` after the `.snxlib`
    /// was renamed on disk. Pure bookkeeping — the adapter's parsed view
    /// stays valid, so nothing is re-read. Without it, the next row write
    /// persists to the old path and recreates the library there.
    ///
    /// The caller owns the file move and calls this once the file is at
    /// `new_file`. A git-backed adapter records the rename in the library's
    /// own repository (old name dropped, new name added, one commit); a
    /// failure leaves the adapter on its old path. Adapters with no on-disk
    /// file keep the default, which refuses.
    fn relocate_library_file(&mut self, _new_file: &std::path::Path) -> Result<(), LibraryError> {
        Err(LibraryError::Backend(
            "relocate_library_file not supported for this adapter".into(),
        ))
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

    /// Create an empty table named `name`. Used by the New Component
    /// modal's `+ New Table…` flow so users can mint destination
    /// tables without dropping into the manifest by hand. Adapters
    /// that don't support manifest mutation surface a `Backend`
    /// error; the UI should disable the create flow when this fails.
    /// `msg` is the commit message for storage backends that require
    /// one (LocalGit). Returns `Conflict` when a table with the same
    /// name already exists.
    fn create_empty_table(&self, _name: &str, _msg: &str) -> Result<(), LibraryError> {
        Err(LibraryError::Backend(
            "create_empty_table not implemented for this adapter".into(),
        ))
    }

    /// Delete the table `name` from the library. Adapters MUST refuse
    /// when the table still contains rows — Manage Tables uses this
    /// surface and the user can drop rows individually first. Returns
    /// `NotFound` when the table doesn't exist, `Conflict` when it's
    /// non-empty.
    fn delete_empty_table(&self, _name: &str, _msg: &str) -> Result<(), LibraryError> {
        Err(LibraryError::Backend(
            "delete_empty_table not implemented for this adapter".into(),
        ))
    }

    /// Rename a table — `old` → `new`, preserving every row inside.
    /// Returns `NotFound` when `old` doesn't exist, `Conflict` when
    /// `new` already does (no silent overwrite). The new name is
    /// validated for filename-safe characters at the adapter
    /// boundary.
    fn rename_table(&self, _old: &str, _new: &str, _msg: &str) -> Result<(), LibraryError> {
        Err(LibraryError::Backend(
            "rename_table not implemented for this adapter".into(),
        ))
    }

    /// Per-library class registry — the source of truth for the New
    /// Component / Edit Component class dropdowns. Adapters that
    /// don't carry a manifest return an empty list.
    ///
    /// # Hazard for implementors
    ///
    /// The signature has no way to say "I could not read the
    /// registry", so an empty return is indistinguishable from a fresh
    /// library with no classes yet. That matters more than it looks,
    /// because [`Self::add_library_class`],
    /// [`Self::remove_library_class`] and
    /// [`Self::rename_library_class`] all default to a read-modify-write
    /// round-trip over this method: an implementation that answers
    /// `Vec::new()` on a read failure and then inherits those defaults
    /// would write the empty list straight back and **erase the
    /// registry on disk**.
    ///
    /// `LocalGitAdapter` is safe on both counts — it overrides all
    /// three mutators with single-borrow read-modify-writes, and its
    /// `library_classes` reports a read failure instead of substituting
    /// an empty list. A new adapter must do one or the other before
    /// relying on the defaults.
    fn library_classes(&self) -> Vec<crate::library_file::ClassEntry> {
        Vec::new()
    }

    /// Persist the class registry. The UI calls this from the
    /// Library Properties pane (Add / Rename / Delete class). Default
    /// implementation surfaces a `Backend` error so adapters that
    /// don't support manifest mutation are detectable.
    fn update_library_classes(
        &self,
        _classes: Vec<crate::library_file::ClassEntry>,
        _msg: &str,
    ) -> Result<(), LibraryError> {
        Err(LibraryError::Backend(
            "update_library_classes not implemented for this adapter".into(),
        ))
    }

    /// Atomic helper — append a class. The default implementation
    /// is a `library_classes` + `update_library_classes` round-trip
    /// (which acquires the lock twice and can't be made atomic
    /// without backend support). Adapters that own the manifest
    /// in-memory (e.g. `LocalGitAdapter` via `mutate_library_file`)
    /// should override with a single-borrow read-modify-write.
    /// Errors `Conflict` when a class with the same key already
    /// exists.
    fn add_library_class(
        &self,
        entry: crate::library_file::ClassEntry,
        msg: &str,
    ) -> Result<(), LibraryError> {
        let mut classes = self.library_classes();
        if classes.iter().any(|c| c.key == entry.key) {
            return Err(LibraryError::Conflict(format!(
                "class with key {:?} already exists",
                entry.key
            )));
        }
        classes.push(entry);
        self.update_library_classes(classes, msg)
    }

    /// Atomic helper — remove a class by key. Default goes through
    /// the same two-step pattern as `add_library_class`. No-op when
    /// the key isn't present.
    fn remove_library_class(&self, key: &str, msg: &str) -> Result<(), LibraryError> {
        let mut classes = self.library_classes();
        let before = classes.len();
        classes.retain(|c| c.key != key);
        if classes.len() == before {
            return Ok(());
        }
        self.update_library_classes(classes, msg)
    }

    /// Atomic helper — rename a class. `old_key` must currently
    /// exist; `new_entry.key` must not collide with any *other*
    /// class. Default pattern matches the other helpers.
    fn rename_library_class(
        &self,
        old_key: &str,
        new_entry: crate::library_file::ClassEntry,
        msg: &str,
    ) -> Result<(), LibraryError> {
        let mut classes = self.library_classes();
        if !classes.iter().any(|c| c.key == old_key) {
            return Err(LibraryError::NotFound(format!(
                "class {old_key:?} not found"
            )));
        }
        if new_entry.key != old_key && classes.iter().any(|c| c.key == new_entry.key) {
            return Err(LibraryError::Conflict(format!(
                "class with key {:?} already exists",
                new_entry.key
            )));
        }
        for c in classes.iter_mut() {
            if c.key == old_key {
                *c = new_entry.clone();
                break;
            }
        }
        self.update_library_classes(classes, msg)
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

    /// Move every pin of one row that binds `save`'s primitive (default or
    /// alternate symbol, footprint, sim) onto `save.version` and patch-bump
    /// the row's own version, re-reading the row at write time so the
    /// write keeps any other field changed since a caller's snapshot.
    /// Returns whether the row changed: a row already pinned at
    /// `save.version` is left alone. The default reads and then updates;
    /// an adapter with a write lock overrides it to do both under the lock.
    fn bind_row_primitive_version(
        &self,
        table: &str,
        row_id: RowId,
        save: &crate::cascade::PrimitiveSave,
        message: &str,
    ) -> Result<bool, LibraryError> {
        let mut row = self.read_row(table, row_id)?;
        if !crate::cascade::needs_bump(&row, save) {
            return Ok(false);
        }
        crate::cascade::apply_cascade_bump(&mut row, save);
        self.update_row(table, row, message)?;
        Ok(true)
    }

    /// Apply `edit` to every row in `targets` (`(table, row)` pairs) as one
    /// batch, returning one outcome per target in order: `Ok(true)` when
    /// the edit changed the row, `Ok(false)` when it left it alone, `Err`
    /// when that row could not be read or written.
    ///
    /// Each row is re-read at write time, so the edit keeps any other
    /// field changed since a caller's snapshot. The edit must not change a
    /// row's identity or part number: a batch is for re-pinning versions,
    /// so it skips the part-number uniqueness check `update_row` runs on a
    /// changed part number. An `Err` for the whole call means nothing was
    /// written.
    ///
    /// The default is `read_row` + `update_row` per target, in order,
    /// stopping at the first failure: the targets after it are reported as
    /// not attempted. An adapter that can write the whole batch at once
    /// (one file write and one commit, one transaction) overrides it.
    fn update_rows(
        &self,
        targets: &[(String, RowId)],
        edit: &mut RowEdit<'_>,
        message: &RowsMessage<'_>,
    ) -> Result<Vec<RowEditOutcome>, LibraryError> {
        let mut outcomes: Vec<RowEditOutcome> = Vec::with_capacity(targets.len());
        let mut failed: Option<String> = None;
        for (table, row_id) in targets {
            if let Some(first) = &failed {
                outcomes.push((
                    *row_id,
                    Err(LibraryError::Backend(format!(
                        "not attempted: an earlier row failed ({first})"
                    ))),
                ));
                continue;
            }
            let outcome = self.read_row(table, *row_id).and_then(|mut row| {
                if !edit(&mut row) {
                    return Ok(false);
                }
                self.update_row(table, row, &message(&[*row_id]))?;
                Ok(true)
            });
            if let Err(e) = &outcome {
                failed = Some(e.to_string());
            }
            outcomes.push((*row_id, outcome));
        }
        Ok(outcomes)
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

    /// Replace a row. Writes only when the stored row is still at the base
    /// revision `row.server_revision` (a server counter, or for a local
    /// library a content hash, see `local_git::helpers::row_base`) and
    /// returns the row's new base; an adapter without bases returns `None`.
    fn update_row(
        &self,
        _table: &str,
        _row: ComponentRow,
        _msg: &str,
    ) -> Result<Option<u64>, LibraryError> {
        Err(LibraryError::Backend(
            "update_row not implemented for this adapter".into(),
        ))
    }

    /// [`update_row`](Self::update_row), committing the working-tree files
    /// `asset_rel_paths` (relative to the library root, already written)
    /// in the same commit as the row that references them. An adapter that
    /// keeps no working tree cannot hold those files, so with any asset it
    /// refuses instead of saving a row whose pinned file nobody else can
    /// resolve.
    fn update_row_with_assets(
        &self,
        table: &str,
        row: ComponentRow,
        msg: &str,
        asset_rel_paths: &[String],
    ) -> Result<Option<u64>, LibraryError> {
        if !asset_rel_paths.is_empty() {
            return Err(LibraryError::Backend(
                "this library cannot store pinned datasheet PDFs: it keeps no files beside its \
                 library file. Use a URL datasheet instead"
                    .into(),
            ));
        }
        self.update_row(table, row, msg)
    }

    /// Delete a row. `base_revision` is the `server_revision` of the copy
    /// the caller is deleting (server counter or local content hash); the
    /// delete is refused when the stored row has moved past it.
    fn delete_row(
        &self,
        _table: &str,
        _row_id: RowId,
        _base_revision: Option<u64>,
        _msg: &str,
    ) -> Result<(), LibraryError> {
        Err(LibraryError::Backend(
            "delete_row not implemented for this adapter".into(),
        ))
    }

    // ── Cached distributor pricing ──────────────────────────────────────

    /// Cached distributor prices of the named table, keyed by `row_id`.
    /// Adapters without a price cache report none.
    fn table_pricing(
        &self,
        _table: &str,
    ) -> Result<std::collections::BTreeMap<Uuid, crate::pricing::RowPricing>, LibraryError> {
        Ok(std::collections::BTreeMap::new())
    }

    /// Write fetched prices back as ONE save and ONE commit. `cancel` is
    /// re-checked under the write lock, after the reload and before the
    /// write: set, nothing is saved or committed. Each update is
    /// re-checked against the row as it is at write time: a row deleted, or
    /// whose SKU for that distributor changed, since the fetch started is
    /// reported in [`crate::StoreOutcome::dropped`] and not written.
    fn store_pricing(
        &self,
        _updates: Vec<crate::pricing::PricingUpdate>,
        _cancel: &std::sync::atomic::AtomicBool,
        _msg: &str,
    ) -> Result<crate::pricing::StoreOutcome, LibraryError> {
        Err(LibraryError::Backend(
            "store_pricing not implemented for this adapter".into(),
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

    // ── Versioned primitive reads / saves ───────────────────────────────
    //
    // A shared library server versions every primitive. `get_*_versioned`
    // returns the revision the copy was read at; `save_*_versioned` sends it
    // back as the base, and the save is refused with
    // `LibraryError::RowChanged` when the stored primitive has moved on.
    // On a versioned backend a `None` base creates the primitive
    // (`Conflict` if it exists). The saved revision comes back so the next
    // save of the same copy can use it. The defaults carry `None` both ways
    // and call the plain `save_*`, so a `None`-base save is only create-only
    // when the plain `save_*` is. A shared backend must therefore make its
    // plain `save_*` create-only (as `DatabaseAdapter` does); a local
    // single-writer backend such as `LocalGitAdapter` overwrites, which is
    // fine there because nobody else writes the copy.

    fn get_symbol_versioned(&self, uuid: Uuid) -> Result<Versioned<Symbol>, LibraryError> {
        self.get_symbol(uuid).map(|value| Versioned {
            value,
            revision: None,
        })
    }

    fn get_footprint_versioned(&self, uuid: Uuid) -> Result<Versioned<Footprint>, LibraryError> {
        self.get_footprint(uuid).map(|value| Versioned {
            value,
            revision: None,
        })
    }

    fn get_sim_versioned(&self, uuid: Uuid) -> Result<Versioned<SimModel>, LibraryError> {
        self.get_sim(uuid).map(|value| Versioned {
            value,
            revision: None,
        })
    }

    fn save_symbol_versioned(
        &self,
        sym: Symbol,
        base: Option<u64>,
        message: &str,
    ) -> Result<Option<u64>, LibraryError> {
        refuse_unversioned_base(base)?;
        self.save_symbol(sym, message).map(|()| None)
    }

    fn save_footprint_versioned(
        &self,
        fp: Footprint,
        base: Option<u64>,
        message: &str,
    ) -> Result<Option<u64>, LibraryError> {
        refuse_unversioned_base(base)?;
        self.save_footprint(fp, message).map(|()| None)
    }

    fn save_sim_versioned(
        &self,
        sm: SimModel,
        base: Option<u64>,
        message: &str,
    ) -> Result<Option<u64>, LibraryError> {
        refuse_unversioned_base(base)?;
        self.save_sim(sm, message).map(|()| None)
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

    /// True when the most recent `list_symbols` succeeded but left some
    /// symbols out because a container file could not be read or parsed.
    /// Such a listing says nothing about what is *missing*, so callers
    /// that report absence must not trust it.
    fn symbol_listing_is_partial(&self) -> bool {
        false
    }

    /// For local-git, the `.snxlib/` directory; for DB, `None`.
    fn root_path(&self) -> Option<PathBuf> {
        None
    }

    /// Read, rewrite and write back one file of this library while holding
    /// the library's cross-process write lock, so another Signex process
    /// cannot write the file between the read and the write (#902).
    ///
    /// `rewrite` receives the file's current bytes (`None` when it does not
    /// exist) and returns the bytes to write, or `None` to leave the file as
    /// it is. Returns whether the file was written. A written file is
    /// committed with `commit_message` before the lock is released, so
    /// another process cannot replace it while git reads it. A failed
    /// commit is logged, not returned: the file is already saved, and the
    /// next commit picks it up.
    ///
    /// The default has no lock to take and no version control, and only
    /// does the read and write; see [`rewrite_file`].
    fn rewrite_file_locked(
        &self,
        abs_path: &std::path::Path,
        _commit_message: &str,
        rewrite: &mut crate::adapter::FileRewrite<'_>,
    ) -> Result<bool, LibraryError> {
        rewrite_file(abs_path, rewrite)
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
    /// Returns up to 50 entries from `git log --max-count 50
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

/// The edit of [`LibraryAdapter::update_rows`]: changes the row it is given
/// and returns whether it did.
pub type RowEdit<'a> = dyn FnMut(&mut ComponentRow) -> bool + 'a;

/// The commit message of [`LibraryAdapter::update_rows`], built once the
/// batch knows which rows the edit actually changed: it is given exactly
/// those rows, so the message never names a row that was gone or already
/// moved.
pub type RowsMessage<'a> = dyn Fn(&[RowId]) -> String + 'a;

/// One target's outcome from [`LibraryAdapter::update_rows`].
pub type RowEditOutcome = (RowId, Result<bool, LibraryError>);

/// The rewrite step of [`LibraryAdapter::rewrite_file_locked`]: given the
/// file's current bytes (`None` when it does not exist), the bytes to write,
/// or `None` to leave it as it is.
pub type FileRewrite<'a> = dyn FnMut(Option<&[u8]>) -> Result<Option<Vec<u8>>, LibraryError> + 'a;

/// The read and write of [`LibraryAdapter::rewrite_file_locked`], with no
/// lock. Used directly for a primitive file that belongs to no mounted
/// library. The write is atomic.
pub fn rewrite_file(
    abs_path: &Path,
    rewrite: &mut crate::adapter::FileRewrite<'_>,
) -> Result<bool, LibraryError> {
    let current = match std::fs::read(abs_path) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    let Some(bytes) = rewrite(current.as_deref())? else {
        return Ok(false);
    };
    signex_types::atomic_io::atomic_write(abs_path, &bytes)?;
    Ok(true)
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
