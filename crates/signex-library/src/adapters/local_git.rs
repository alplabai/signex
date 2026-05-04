//! Local + git storage adapter — `.snxlib` file backed by libgit2.
//!
//! Per `v0.9-snxlib-as-file-plan.md`, a Signex library on disk is a
//! *directory* containing a `.snxlib` file (the user-facing entry
//! point) and sibling `symbols/` / `footprints/` / `sims/` /
//! `models/` directories. The `.git/` repo lives at the parent
//! directory so per-primitive `git log -- symbols/<name>.snxsym`
//! works line-by-line — that's the load-bearing reason the layout
//! is multi-file.
//!
//! ```text
//! mylib/                           (root_dir — git working tree)
//! ├── mylib.snxlib                 (file_path — TOML manifest + [tables.<name>] TSV)
//! ├── symbols/<slug>.snxsym
//! ├── footprints/<uuid>.snxfpt
//! ├── sims/<uuid>.snxsim
//! ├── models/                      (3D models, optionally LFS-tracked)
//! ├── .gitattributes               (written when [`LibraryInitOptions::use_lfs`])
//! └── .git/
//! ```
//!
//! Tables (component rows) live *inside* the `.snxlib` file under
//! `[tables.<name>]` blocks — there are no separate `tables/*.tsv`
//! files anymore. The trait still talks in [`ComponentRow`] for v0.9
//! compatibility; the adapter converts between the legacy 16-column
//! [`crate::tables::TABLE_HEADER`] schema and the new
//! [`LibraryRow`] cell-map at the boundary. Stage 12 will retire
//! the legacy schema in favour of user-defined columns.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::adapter::{HistoryEntry, LibraryAdapter, LibraryError, PrimitiveSummary};
use crate::component::ComponentRow;
use crate::identity::{InternalPn, RowId};
use crate::library_file::{LibraryFile, LibraryRow, LibraryTable, SnxlibManifest};
use crate::manifest::{LibraryMeta, Manifest};
use crate::primitive::{Footprint, PrimitiveKind, SimFile, SimModel, Symbol, SymbolFile};
use crate::tables::{TABLE_HEADER, record_to_row, row_to_record};

const SYMBOLS_DIR: &str = "symbols";
const FOOTPRINTS_DIR: &str = "footprints";
const SIMS_DIR: &str = "sims";
const SYMBOL_EXT: &str = "snxsym";
const FOOTPRINT_EXT: &str = "snxfpt";
const SIM_EXT: &str = "snxsim";
const SNXLIB_EXT: &str = "snxlib";
const GITATTRIBUTES_FILE: &str = ".gitattributes";

/// File extensions tracked by Git LFS when [`LibraryInitOptions::use_lfs`] is on.
const LFS_EXTENSIONS: &[&str] = &["step", "stp", "wrl", "iges"];

/// Library-create options threaded through [`LocalGitAdapter::init`].
///
/// `enable_git` defaults to **off** — fresh libraries land on disk as
/// plain files with no `.git/` directory. Users opt in via the
/// "Enable version control" checkbox on the New Library Options
/// modal; flipping it on runs `git init` + records an initial commit
/// just like the legacy behaviour. LFS only matters when version
/// control is on (no point in `.gitattributes` without a repo).
#[derive(Debug, Default, Clone, Copy)]
pub struct LibraryInitOptions {
    /// Run `git init` at the parent directory and stage the
    /// freshly-written `.snxlib` (plus `.gitattributes` when
    /// `use_lfs`) as the first commit. Off by default — opting in is
    /// an explicit user choice.
    pub enable_git: bool,
    /// Write a `.gitattributes` opting `*.step` / `*.stp` / `*.wrl` /
    /// `*.iges` into Git LFS at init time. Only meaningful when
    /// `enable_git` is also true.
    pub use_lfs: bool,
}

/// Adapter over a `.snxlib`-file-rooted directory + git repo.
#[derive(Debug)]
pub struct LocalGitAdapter {
    /// Absolute path to the `.snxlib` file.
    file_path: PathBuf,
    /// Absolute path to the directory holding the `.snxlib` file —
    /// the per-library git repo root and parent of `symbols/` etc.
    root_dir: PathBuf,
    /// In-memory parsed view of the `.snxlib` file. Mutated on row
    /// CRUD and persisted before the matching `git commit`. The
    /// `RwLock` lets the trait keep `&self` semantics on mutations
    /// while still letting parallel readers grab snapshots.
    library_file: RwLock<LibraryFile>,
    /// Synthesised legacy [`Manifest`] view for callers reaching for
    /// `manifest()`. Header data only — never carries the `tables`
    /// list. Stable for the adapter's lifetime; mutations to
    /// `library_file.tables` do not alter it.
    manifest_synth: Manifest,
}

impl LocalGitAdapter {
    /// Initialise a fresh library at `file_path`. The path must end
    /// in `.snxlib`; its parent directory becomes the git working tree.
    ///
    /// Fails with `Conflict` if a `.snxlib` already exists at
    /// `file_path`. Creates the parent directory if it doesn't
    /// already exist.
    pub fn init(
        file_path: impl AsRef<Path>,
        snx_manifest: SnxlibManifest,
        opts: LibraryInitOptions,
    ) -> Result<Self, LibraryError> {
        let file_path = file_path.as_ref().to_path_buf();
        Self::validate_file_path(&file_path)?;
        if file_path.exists() {
            return Err(LibraryError::Conflict(format!(
                "library file already exists at {}",
                file_path.display()
            )));
        }

        let root_dir = parent_dir(&file_path)?;
        fs::create_dir_all(&root_dir)?;

        let library_file = LibraryFile {
            manifest: snx_manifest,
            tables: std::collections::BTreeMap::new(),
        };
        let text = library_file.write()?;
        fs::write(&file_path, &text)?;

        // Git scaffolding — only when the user opted in. Without
        // `enable_git` the library lives on disk as plain files and
        // every mutation operation is a best-effort `commit_path`
        // that no-ops when no `.git/` is present.
        if opts.enable_git {
            // LFS attributes go in BEFORE `git init` so the initial commit
            // already has `.gitattributes` staged; otherwise libgit2's
            // index update sequence gets fiddly.
            if opts.use_lfs {
                write_lfs_attributes(&root_dir)?;
            }

            let repo = git2::Repository::init(&root_dir)
                .map_err(|e| LibraryError::Backend(format!("git init: {e}")))?;
            let (sig_name, sig_email) = identity_for_repo(&repo);
            let sig = git2::Signature::now(&sig_name, &sig_email)
                .map_err(|e| LibraryError::Backend(format!("git signature: {e}")))?;

            let mut index = repo
                .index()
                .map_err(|e| LibraryError::Backend(format!("git index: {e}")))?;
            let snxlib_rel = file_name_str(&file_path)?;
            index
                .add_path(Path::new(&snxlib_rel))
                .map_err(|e| LibraryError::Backend(format!("git add snxlib: {e}")))?;
            if opts.use_lfs {
                index
                    .add_path(Path::new(GITATTRIBUTES_FILE))
                    .map_err(|e| LibraryError::Backend(format!("git add gitattributes: {e}")))?;
            }
            index
                .write()
                .map_err(|e| LibraryError::Backend(format!("git index write: {e}")))?;
            let tree_oid = index
                .write_tree()
                .map_err(|e| LibraryError::Backend(format!("git write tree: {e}")))?;
            let tree = repo
                .find_tree(tree_oid)
                .map_err(|e| LibraryError::Backend(format!("git find tree: {e}")))?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "chore: initialise library",
                &tree,
                &[],
            )
            .map_err(|e| LibraryError::Backend(format!("git initial commit: {e}")))?;
        }

        let manifest_synth = synthesize_manifest(&library_file.manifest);
        Ok(Self {
            file_path,
            root_dir,
            library_file: RwLock::new(library_file),
            manifest_synth,
        })
    }

    /// Open an existing library by `.snxlib` file path. The file's
    /// parent directory must already host a `.git/` — recovery from
    /// a deleted git repo lives in [`Self::recover_init`].
    pub fn open(file_path: impl AsRef<Path>) -> Result<Self, LibraryError> {
        let file_path = file_path.as_ref().to_path_buf();
        Self::validate_file_path(&file_path)?;
        if !file_path.exists() {
            return Err(LibraryError::NotFound(format!(
                "no .snxlib at {}",
                file_path.display()
            )));
        }
        let text = fs::read_to_string(&file_path)?;
        let library_file = LibraryFile::parse(&text)?;
        let root_dir = parent_dir(&file_path)?;
        // Validate the repo opens — but treat a missing `.git/` as a
        // clean "no version control" state rather than a hard error.
        // Libraries created with `LibraryInitOptions::enable_git =
        // false` ship as plain files; opening them must not require
        // a repo. Other open failures (corrupt repo, partial init)
        // still surface so the recovery dialog can route the user
        // to `recover_init`.
        if root_dir.join(".git").exists() {
            git2::Repository::open(&root_dir)
                .map_err(|e| LibraryError::Backend(format!("git open: {e}")))?;
        }
        let manifest_synth = synthesize_manifest(&library_file.manifest);
        Ok(Self {
            file_path,
            root_dir,
            library_file: RwLock::new(library_file),
            manifest_synth,
        })
    }

    /// Recover a library whose `.git/` directory was deleted
    /// out-from-under-it. Re-runs `git init` at the parent directory
    /// and stages the current working tree as a fresh
    /// "snxlib re-init" commit. Past history is lost — the recovery
    /// dialog (Stage 10) is responsible for warning the user before
    /// landing here.
    pub fn recover_init(file_path: impl AsRef<Path>) -> Result<Self, LibraryError> {
        let file_path = file_path.as_ref().to_path_buf();
        Self::validate_file_path(&file_path)?;
        if !file_path.exists() {
            return Err(LibraryError::NotFound(format!(
                "cannot recover-init: no .snxlib at {}",
                file_path.display()
            )));
        }
        let root_dir = parent_dir(&file_path)?;

        // `git2::Repository::init` is idempotent on an already-init'd
        // working tree, so this also handles the "git was *not*
        // deleted" case as a no-op + re-commit-everything. The recovery
        // dialog is the gate; the adapter just does what it's asked.
        let repo = git2::Repository::init(&root_dir)
            .map_err(|e| LibraryError::Backend(format!("git re-init: {e}")))?;
        let (sig_name, sig_email) = identity_for_repo(&repo);
        let sig = git2::Signature::now(&sig_name, &sig_email)
            .map_err(|e| LibraryError::Backend(format!("git signature: {e}")))?;

        let mut index = repo
            .index()
            .map_err(|e| LibraryError::Backend(format!("git index: {e}")))?;
        index
            .add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)
            .map_err(|e| LibraryError::Backend(format!("git add all: {e}")))?;
        index
            .write()
            .map_err(|e| LibraryError::Backend(format!("git index write: {e}")))?;
        let tree_oid = index
            .write_tree()
            .map_err(|e| LibraryError::Backend(format!("git write tree: {e}")))?;
        let tree = repo
            .find_tree(tree_oid)
            .map_err(|e| LibraryError::Backend(format!("git find tree: {e}")))?;
        // Use the existing HEAD commit as parent if any — a re-init
        // case keeps prior history intact under the rewritten ref.
        let parent = match repo.head() {
            Ok(h) => h
                .peel_to_commit()
                .map_err(|e| LibraryError::Backend(format!("git peel to commit: {e}")))
                .map(Some)?,
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
            Err(e) => return Err(LibraryError::Backend(format!("git head: {e}"))),
        };
        let parents: Vec<&git2::Commit> = parent.as_ref().map(|c| vec![c]).unwrap_or_default();
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "chore: snxlib re-init",
            &tree,
            &parents,
        )
        .map_err(|e| LibraryError::Backend(format!("git recover commit: {e}")))?;

        let text = fs::read_to_string(&file_path)?;
        let library_file = LibraryFile::parse(&text)?;
        let manifest_synth = synthesize_manifest(&library_file.manifest);
        Ok(Self {
            file_path,
            root_dir,
            library_file: RwLock::new(library_file),
            manifest_synth,
        })
    }

    /// Reject paths whose extension isn't `.snxlib`. We refuse rather
    /// than silently rewriting; the file picker should be filtering,
    /// but defensively block anyway so a misnamed path doesn't end up
    /// committed to history.
    fn validate_file_path(p: &Path) -> Result<(), LibraryError> {
        let ext_ok = p
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case(SNXLIB_EXT))
            .unwrap_or(false);
        if !ext_ok {
            return Err(LibraryError::Backend(format!(
                "library path must end with `.{SNXLIB_EXT}`: {}",
                p.display()
            )));
        }
        Ok(())
    }

    // ── Path accessors ─────────────────────────────────────────────────────

    /// Borrow the directory holding the `.snxlib` file (the git working tree).
    pub fn root(&self) -> &Path {
        &self.root_dir
    }

    /// Borrow the absolute path to the `.snxlib` file itself.
    pub fn file_path_buf(&self) -> &Path {
        &self.file_path
    }

    fn primitive_dir(&self, kind: PrimitiveKind) -> PathBuf {
        self.root_dir.join(primitive_subdir(kind))
    }

    fn primitive_path(&self, kind: PrimitiveKind, uuid: Uuid) -> PathBuf {
        self.primitive_dir(kind)
            .join(format!("{uuid}.{}", primitive_ext(kind)))
    }

    /// Read a primitive JSON file at `<root>/<subdir>/<uuid>.<ext>`.
    fn read_primitive<T: DeserializeOwned>(
        &self,
        kind: PrimitiveKind,
        uuid: Uuid,
    ) -> Result<T, LibraryError> {
        let path = self.primitive_path(kind, uuid);
        if !path.exists() {
            return Err(LibraryError::NotFound(format!(
                "{} {uuid}",
                primitive_kind_str(kind)
            )));
        }
        let bytes = fs::read(&path)?;
        // v0.18.4 — `.snxfpt` and `.snxsym` ship as TOML+TSV
        // envelopes. v0.18.5 — `.snxsim` ships as TOML envelope.
        // For each primitive kind that's been migrated, build the
        // file envelope, pull the first contained primitive, then
        // serde-round-trip through JSON to recover the generic T.
        if matches!(kind, PrimitiveKind::Footprint) {
            let file = crate::primitive::FootprintFile::from_bytes(&bytes)
                .map_err(|e| LibraryError::Backend(format!("read .snxfpt: {e}")))?;
            let fp = file
                .footprints
                .into_iter()
                .next()
                .ok_or_else(|| LibraryError::Backend("empty FootprintFile".into()))?;
            let buf = serde_json::to_vec(&fp)
                .map_err(|e| LibraryError::Backend(format!("re-serialise footprint: {e}")))?;
            let value: T = serde_json::from_slice(&buf)
                .map_err(|e| LibraryError::Backend(format!("read primitive: {e}")))?;
            return Ok(value);
        }
        if matches!(kind, PrimitiveKind::Sim) {
            let file = SimFile::from_bytes(&bytes)
                .map_err(|e| LibraryError::Backend(format!("read .snxsim: {e}")))?;
            let model = file
                .models
                .into_iter()
                .next()
                .ok_or_else(|| LibraryError::Backend("empty SimFile".into()))?;
            let buf = serde_json::to_vec(&model)
                .map_err(|e| LibraryError::Backend(format!("re-serialise sim: {e}")))?;
            let value: T = serde_json::from_slice(&buf)
                .map_err(|e| LibraryError::Backend(format!("read primitive: {e}")))?;
            return Ok(value);
        }
        let value: T = serde_json::from_slice(&bytes)
            .map_err(|e| LibraryError::Backend(format!("read primitive: {e}")))?;
        Ok(value)
    }

    /// Persist a primitive file under `<root>/<subdir>/<uuid>.<ext>`,
    /// stage + commit it via libgit2 with the supplied message.
    ///
    /// `.snxfpt` files emit as TOML+TSV envelope (v0.18.4); `.snxsim`
    /// files emit as TOML envelope (v0.18.5). `.snxsym` is handled
    /// outside this generic path via `save_symbol_in_container` so
    /// multi-symbol containers are preserved.
    fn write_primitive<T: Serialize>(
        &self,
        kind: PrimitiveKind,
        uuid: Uuid,
        value: &T,
        message: &str,
    ) -> Result<(), LibraryError> {
        let dir = self.primitive_dir(kind);
        fs::create_dir_all(&dir)?;
        let rel_path = format!("{}/{uuid}.{}", primitive_subdir(kind), primitive_ext(kind));
        let abs_path = self.root_dir.join(&rel_path);
        let bytes = if matches!(kind, PrimitiveKind::Footprint) {
            // T is Footprint here — round-trip through JSON to obtain
            // the typed value, wrap into FootprintFile, then emit TOML.
            let buf = serde_json::to_vec(value)
                .map_err(|e| LibraryError::Backend(format!("re-serialise footprint: {e}")))?;
            let fp: crate::primitive::Footprint = serde_json::from_slice(&buf)
                .map_err(|e| LibraryError::Backend(format!("write primitive: {e}")))?;
            let file = crate::primitive::FootprintFile::from_footprint(fp);
            file.to_toml_string()
                .map_err(|e| LibraryError::Backend(format!("emit .snxfpt: {e}")))?
                .into_bytes()
        } else if matches!(kind, PrimitiveKind::Sim) {
            // T is SimModel here — same JSON round-trip recovery
            // pattern, then wrap into SimFile and emit TOML so the
            // SPICE / Verilog-A `body` field lands as a literal
            // multi-line string.
            let buf = serde_json::to_vec(value)
                .map_err(|e| LibraryError::Backend(format!("re-serialise sim: {e}")))?;
            let model: SimModel = serde_json::from_slice(&buf)
                .map_err(|e| LibraryError::Backend(format!("write primitive: {e}")))?;
            let file = SimFile::from_model(model);
            file.to_toml_string()
                .map_err(|e| LibraryError::Backend(format!("emit .snxsim: {e}")))?
                .into_bytes()
        } else {
            serde_json::to_vec_pretty(value)
                .map_err(|e| LibraryError::Backend(format!("write primitive: {e}")))?
        };
        fs::write(&abs_path, bytes)?;

        let fallback = format!("save {} {uuid}", primitive_kind_str(kind));
        self.commit_path(&rel_path, message, &fallback)
    }

    /// Stage `rel_path` and create a new commit. Used by primitive saves
    /// (`*.snx*` files) and table writes (the `.snxlib` itself). When
    /// the parent directory has no `.git/`, this is a no-op — the file
    /// has already been written to disk by the caller, and the user
    /// opted out of version control at create time. They can opt in
    /// later via the (forthcoming) Enable Version Control flow.
    fn commit_path(
        &self,
        rel_path: &str,
        message: &str,
        fallback_message: &str,
    ) -> Result<(), LibraryError> {
        if !self.root_dir.join(".git").exists() {
            return Ok(());
        }
        let repo = git2::Repository::open(&self.root_dir)
            .map_err(|e| LibraryError::Backend(format!("git open: {e}")))?;
        let (sig_name, sig_email) = identity_for_repo(&repo);
        let sig = git2::Signature::now(&sig_name, &sig_email)
            .map_err(|e| LibraryError::Backend(format!("git signature: {e}")))?;

        let mut index = repo
            .index()
            .map_err(|e| LibraryError::Backend(format!("git index: {e}")))?;
        index
            .add_path(Path::new(rel_path))
            .map_err(|e| LibraryError::Backend(format!("git add: {e}")))?;
        index
            .write()
            .map_err(|e| LibraryError::Backend(format!("git index write: {e}")))?;
        let tree_oid = index
            .write_tree()
            .map_err(|e| LibraryError::Backend(format!("git write tree: {e}")))?;
        let tree = repo
            .find_tree(tree_oid)
            .map_err(|e| LibraryError::Backend(format!("git find tree: {e}")))?;

        // Resolve the parent commit. An unborn HEAD (fresh repo, no commits
        // yet) is the only legitimate "no parent" case — every other error
        // (corrupt ref, locked ref) propagates so we don't silently produce
        // an orphan commit on a broken repo.
        let parent = match repo.head() {
            Ok(h) => h
                .peel_to_commit()
                .map_err(|e| LibraryError::Backend(format!("git peel to commit: {e}")))
                .map(Some)?,
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
            Err(e) => return Err(LibraryError::Backend(format!("git head: {e}"))),
        };
        let parents: Vec<&git2::Commit> = parent.as_ref().map(|c| vec![c]).unwrap_or_default();
        let commit_message = if message.is_empty() {
            fallback_message.to_string()
        } else {
            message.to_string()
        };
        repo.commit(Some("HEAD"), &sig, &sig, &commit_message, &tree, &parents)
            .map_err(|e| LibraryError::Backend(format!("git commit: {e}")))?;
        Ok(())
    }

    // ── Table helpers ──────────────────────────────────────────────────────

    fn snxlib_rel_path(&self) -> Result<String, LibraryError> {
        file_name_str(&self.file_path)
    }

    /// Persist the in-memory `library_file` to disk. Caller already holds
    /// the appropriate read/write lock.
    fn persist_library_file(&self, lf: &LibraryFile) -> Result<(), LibraryError> {
        let text = lf.write()?;
        fs::write(&self.file_path, text)?;
        Ok(())
    }

    /// Mutate the in-memory `library_file`, persist it, and commit the
    /// `.snxlib` with the supplied message.
    fn mutate_library_file<F>(
        &self,
        f: F,
        message: &str,
        fallback: &str,
    ) -> Result<(), LibraryError>
    where
        F: FnOnce(&mut LibraryFile) -> Result<(), LibraryError>,
    {
        let mut guard = self
            .library_file
            .write()
            .map_err(|_| LibraryError::Backend("library_file write lock poisoned".into()))?;
        f(&mut guard)?;
        self.persist_library_file(&guard)?;
        let rel = self.snxlib_rel_path()?;
        self.commit_path(&rel, message, fallback)
    }

    /// Read the table named `table` as a snapshot of [`ComponentRow`]s,
    /// scoped to whatever the legacy [`TABLE_HEADER`] columns are.
    /// Returns an empty vec for unknown tables (matches the old
    /// `tables/<name>.tsv` "missing file = empty" semantics).
    fn snapshot_table(&self, name: &str) -> Result<Vec<ComponentRow>, LibraryError> {
        let guard = self
            .library_file
            .read()
            .map_err(|_| LibraryError::Backend("library_file read lock poisoned".into()))?;
        let Some(table) = guard.tables.get(name) else {
            return Ok(Vec::new());
        };
        // Verify the on-disk header matches the legacy schema. Once
        // Stage 12 lifts the fixed-column constraint this guard goes
        // away, but for v0.9 we want a loud error if a hand-edited
        // .snxlib drifts the schema.
        validate_legacy_header(name, &table.columns)?;
        let mut out = Vec::with_capacity(table.rows.len());
        for row in &table.rows {
            out.push(library_row_to_component(row)?);
        }
        Ok(out)
    }

    // ── Symbol container helpers (v0.9 phase 2 multi-symbol files) ────────

    fn scan_symbol_files(&self) -> Result<Vec<(PathBuf, SymbolFile)>, LibraryError> {
        let dir = self.primitive_dir(PrimitiveKind::Symbol);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let suffix = format!(".{SYMBOL_EXT}");
        let mut out: Vec<(PathBuf, SymbolFile)> = Vec::new();
        for entry in walkdir::WalkDir::new(&dir)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if !name.ends_with(&suffix) {
                continue;
            }
            let bytes = fs::read(path)?;
            // v0.18.4 — auto-detect TOML vs legacy JSON via
            // `SymbolFile::from_bytes`. Old `.snxsym` files (JSON)
            // continue to load; new files emit TOML.
            let file = SymbolFile::from_bytes(&bytes)
                .map_err(|e| LibraryError::Backend(format!("read symbol file {name}: {e}")))?;
            out.push((path.to_path_buf(), file));
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(out)
    }

    fn save_symbol_in_container(&self, sym: Symbol, message: &str) -> Result<(), LibraryError> {
        let dir = self.primitive_dir(PrimitiveKind::Symbol);
        fs::create_dir_all(&dir)?;

        let target_path = match self.locate_symbol_file(sym.uuid)? {
            Some((path, mut file)) => {
                if !file.upsert(sym.clone()) {
                    file.symbols.push(sym.clone());
                    file.updated = chrono::Utc::now();
                }
                // v0.18.4 — emit TOML envelope.
                let text = file
                    .to_toml_string()
                    .map_err(|e| LibraryError::Backend(format!("write symbol container: {e}")))?;
                fs::write(&path, text.as_bytes())?;
                path
            }
            None => {
                let file = SymbolFile::from_symbol(sym.clone());
                let path = self.fresh_symbol_file_path(&dir, &file)?;
                // v0.18.4 — emit TOML envelope.
                let text = file
                    .to_toml_string()
                    .map_err(|e| LibraryError::Backend(format!("write symbol container: {e}")))?;
                fs::write(&path, text.as_bytes())?;
                path
            }
        };

        let rel_path = target_path
            .strip_prefix(&self.root_dir)
            .map_err(|_| {
                LibraryError::Backend(format!(
                    "could not relativise {} against root",
                    target_path.display()
                ))
            })?
            .to_string_lossy()
            .replace('\\', "/");
        let fallback = format!("save symbol {} into {rel_path}", sym.uuid);
        self.commit_path(&rel_path, message, &fallback)
    }

    fn locate_symbol_file(
        &self,
        uuid: Uuid,
    ) -> Result<Option<(PathBuf, SymbolFile)>, LibraryError> {
        for (path, file) in self.scan_symbol_files()? {
            if file.symbols.iter().any(|s| s.uuid == uuid) {
                return Ok(Some((path, file)));
            }
        }
        Ok(None)
    }

    fn fresh_symbol_file_path(
        &self,
        dir: &Path,
        file: &SymbolFile,
    ) -> Result<PathBuf, LibraryError> {
        let raw = if !file.display_name.is_empty() {
            file.display_name.as_str()
        } else if let Some(first) = file.symbols.first() {
            first.name.as_str()
        } else {
            "Untitled"
        };
        let slug = slugify(raw);
        let candidate = dir.join(format!("{slug}.{SYMBOL_EXT}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
        // Collision — fall back to the file uuid which is guaranteed unique.
        Ok(dir.join(format!("{}.{SYMBOL_EXT}", file.file_uuid)))
    }

    fn list_primitive_summaries<T>(
        &self,
        kind: PrimitiveKind,
        name_of: impl Fn(&T) -> &str,
    ) -> Result<Vec<PrimitiveSummary>, LibraryError>
    where
        T: DeserializeOwned,
    {
        let dir = self.primitive_dir(kind);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let suffix = format!(".{}", primitive_ext(kind));
        let mut out: Vec<PrimitiveSummary> = Vec::new();
        for entry in walkdir::WalkDir::new(&dir)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if !name.ends_with(&suffix) {
                continue;
            }
            let stem = &name[..name.len() - suffix.len()];
            let Ok(uuid) = stem.parse::<Uuid>() else {
                continue;
            };
            let bytes = fs::read(path)?;
            // v0.18.4/v0.18.5 — `.snxfpt` and `.snxsim` migrated to
            // TOML envelopes. Read the envelope, pull the first
            // contained primitive, then JSON-round-trip into the
            // generic T (= Footprint or = SimModel).
            let value: T = if matches!(kind, PrimitiveKind::Footprint) {
                let file = crate::primitive::FootprintFile::from_bytes(&bytes)
                    .map_err(|e| LibraryError::Backend(format!("list primitive {name}: {e}")))?;
                let fp = file.footprints.into_iter().next().ok_or_else(|| {
                    LibraryError::Backend(format!("empty .snxfpt {name}"))
                })?;
                let buf = serde_json::to_vec(&fp).map_err(|e| {
                    LibraryError::Backend(format!("re-serialise .snxfpt {name}: {e}"))
                })?;
                serde_json::from_slice(&buf).map_err(|e| {
                    LibraryError::Backend(format!("list primitive {name}: {e}"))
                })?
            } else if matches!(kind, PrimitiveKind::Sim) {
                let file = SimFile::from_bytes(&bytes)
                    .map_err(|e| LibraryError::Backend(format!("list primitive {name}: {e}")))?;
                let model = file.models.into_iter().next().ok_or_else(|| {
                    LibraryError::Backend(format!("empty .snxsim {name}"))
                })?;
                let buf = serde_json::to_vec(&model).map_err(|e| {
                    LibraryError::Backend(format!("re-serialise .snxsim {name}: {e}"))
                })?;
                serde_json::from_slice(&buf).map_err(|e| {
                    LibraryError::Backend(format!("list primitive {name}: {e}"))
                })?
            } else {
                serde_json::from_slice(&bytes)
                    .map_err(|e| LibraryError::Backend(format!("list primitive {name}: {e}")))?
            };
            out.push(PrimitiveSummary {
                uuid,
                name: name_of(&value).to_string(),
                kind,
                used_by_count: 0,
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }
}

impl LibraryAdapter for LocalGitAdapter {
    fn manifest(&self) -> &Manifest {
        &self.manifest_synth
    }

    fn library_file(&self) -> Option<&LibraryFile> {
        // Returning `&LibraryFile` from inside a `RwLock` would require a
        // self-referential guard, which the trait shape can't express.
        // Stage 5 will replace this with a closure-based accessor; for
        // now the trait method stays `None` for git-backed adapters and
        // callers wanting the parsed view go through other surfaces
        // (`list_tables` / `read_table`). The DB adapter returns `None`
        // because it has no `.snxlib` on disk.
        None
    }

    fn root_dir(&self) -> Option<&Path> {
        Some(&self.root_dir)
    }

    fn library_file_path(&self) -> Option<&Path> {
        Some(&self.file_path)
    }

    // ── Tables ─────────────────────────────────────────────────────────────

    fn list_tables(&self) -> Result<Vec<String>, LibraryError> {
        let guard = self
            .library_file
            .read()
            .map_err(|_| LibraryError::Backend("library_file read lock poisoned".into()))?;
        let mut names: Vec<String> = guard.tables.keys().cloned().collect();
        names.sort();
        Ok(names)
    }

    fn rename_table(&self, old: &str, new: &str, msg: &str) -> Result<(), LibraryError> {
        let old_owned = old.to_string();
        let new_trimmed = new.trim().to_string();
        if new_trimmed.is_empty() {
            return Err(LibraryError::Backend("table name cannot be empty".into()));
        }
        if new_trimmed.chars().any(|c| {
            matches!(
                c,
                '/' | '\\' | '.' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            )
        }) {
            return Err(LibraryError::Backend(format!(
                "table name {new_trimmed:?} contains illegal characters"
            )));
        }
        if old_owned == new_trimmed {
            return Ok(());
        }
        let new_owned = new_trimmed;
        let fallback = format!("rename table {old_owned} → {new_owned}");
        self.mutate_library_file(
            move |lf| {
                if !lf.tables.contains_key(&old_owned) {
                    return Err(LibraryError::NotFound(format!(
                        "table {old_owned:?} not found"
                    )));
                }
                if lf.tables.contains_key(&new_owned) {
                    return Err(LibraryError::Conflict(format!(
                        "table {new_owned:?} already exists"
                    )));
                }
                let entry = lf.tables.remove(&old_owned).expect("contains_key checked");
                lf.tables.insert(new_owned, entry);
                Ok(())
            },
            msg,
            &fallback,
        )
    }

    fn delete_empty_table(&self, name: &str, msg: &str) -> Result<(), LibraryError> {
        let owned = name.to_string();
        let fallback = format!("delete table {owned}");
        self.mutate_library_file(
            move |lf| {
                // Single-pass check + remove via `entry()` — avoids
                // the read-then-write borrow pattern that NLL only
                // tolerates because the first borrow ends at the
                // `if`. Using `entry()` keeps the BTreeMap touch
                // atomic and is robust under future reorganisation.
                use std::collections::btree_map::Entry;
                match lf.tables.entry(owned.clone()) {
                    Entry::Vacant(_) => {
                        Err(LibraryError::NotFound(format!("table {owned:?} not found")))
                    }
                    Entry::Occupied(occ) if !occ.get().rows.is_empty() => {
                        Err(LibraryError::Conflict(format!(
                            "table {owned:?} is not empty ({} rows)",
                            occ.get().rows.len()
                        )))
                    }
                    Entry::Occupied(occ) => {
                        occ.remove();
                        Ok(())
                    }
                }
            },
            msg,
            &fallback,
        )
    }

    fn library_classes(&self) -> Vec<crate::library_file::ClassEntry> {
        let guard = match self.library_file.read() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        guard.manifest.classes.clone()
    }

    fn update_library_classes(
        &self,
        classes: Vec<crate::library_file::ClassEntry>,
        msg: &str,
    ) -> Result<(), LibraryError> {
        self.mutate_library_file(
            move |lf| {
                lf.manifest.classes = classes;
                Ok(())
            },
            msg,
            "update class registry",
        )
    }

    fn add_library_class(
        &self,
        entry: crate::library_file::ClassEntry,
        msg: &str,
    ) -> Result<(), LibraryError> {
        // Atomic override of the trait default — read + check +
        // append all happen inside one `mutate_library_file` borrow
        // so concurrent callers can't interleave a duplicate add.
        let fallback = format!("add class {}", entry.key);
        self.mutate_library_file(
            move |lf| {
                if lf.manifest.classes.iter().any(|c| c.key == entry.key) {
                    return Err(LibraryError::Conflict(format!(
                        "class with key {:?} already exists",
                        entry.key
                    )));
                }
                lf.manifest.classes.push(entry);
                Ok(())
            },
            msg,
            &fallback,
        )
    }

    fn remove_library_class(&self, key: &str, msg: &str) -> Result<(), LibraryError> {
        let owned_key = key.to_string();
        let fallback = format!("remove class {owned_key}");
        self.mutate_library_file(
            move |lf| {
                let before = lf.manifest.classes.len();
                lf.manifest.classes.retain(|c| c.key != owned_key);
                // Never error when the key is missing — keeps the
                // UI's "× delete" idempotent.
                let _ = before;
                Ok(())
            },
            msg,
            &fallback,
        )
    }

    fn rename_library_class(
        &self,
        old_key: &str,
        new_entry: crate::library_file::ClassEntry,
        msg: &str,
    ) -> Result<(), LibraryError> {
        let owned_old = old_key.to_string();
        let fallback = format!("rename class {owned_old} → {}", new_entry.key);
        self.mutate_library_file(
            move |lf| {
                if !lf.manifest.classes.iter().any(|c| c.key == owned_old) {
                    return Err(LibraryError::NotFound(format!(
                        "class {owned_old:?} not found"
                    )));
                }
                if new_entry.key != owned_old
                    && lf.manifest.classes.iter().any(|c| c.key == new_entry.key)
                {
                    return Err(LibraryError::Conflict(format!(
                        "class with key {:?} already exists",
                        new_entry.key
                    )));
                }
                for c in lf.manifest.classes.iter_mut() {
                    if c.key == owned_old {
                        *c = new_entry.clone();
                        break;
                    }
                }
                Ok(())
            },
            msg,
            &fallback,
        )
    }

    fn create_empty_table(&self, name: &str, msg: &str) -> Result<(), LibraryError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(LibraryError::Backend("table name cannot be empty".into()));
        }
        // Reject the same characters we reject elsewhere for filenames /
        // identifiers — keeps round-trips through the TOML key safe and
        // avoids surprising the user with a name they can't see in
        // their file browser.
        if trimmed.chars().any(|c| {
            matches!(
                c,
                '/' | '\\' | '.' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            )
        }) {
            return Err(LibraryError::Backend(format!(
                "table name {trimmed:?} contains illegal characters"
            )));
        }
        let owned = trimmed.to_string();
        let fallback = format!("create empty table {owned}");
        self.mutate_library_file(
            move |lf| {
                if lf.tables.contains_key(&owned) {
                    return Err(LibraryError::Conflict(format!(
                        "table {owned:?} already exists"
                    )));
                }
                lf.tables.insert(
                    owned,
                    LibraryTable {
                        columns: legacy_columns(),
                        rows: Vec::new(),
                        column_types: std::collections::BTreeMap::new(),
                    },
                );
                Ok(())
            },
            msg,
            &fallback,
        )
    }

    fn read_table(&self, name: &str) -> Result<Vec<ComponentRow>, LibraryError> {
        self.snapshot_table(name)
    }

    fn iter_rows(&self) -> Result<Vec<(String, ComponentRow)>, LibraryError> {
        let mut out: Vec<(String, ComponentRow)> = Vec::new();
        for name in self.list_tables()? {
            for row in self.snapshot_table(&name)? {
                out.push((name.clone(), row));
            }
        }
        Ok(out)
    }

    fn read_row(&self, table: &str, row_id: RowId) -> Result<ComponentRow, LibraryError> {
        let target = row_id.as_uuid();
        self.snapshot_table(table)?
            .into_iter()
            .find(|r| r.row_id == target)
            .ok_or_else(|| LibraryError::NotFound(format!("row {row_id} in table {table}")))
    }

    /// Linear scan across every table — O(total rows). Acceptable at
    /// v0.9 scale (libraries are O(thousands)). When the search index
    /// lands the call should redirect through it.
    fn read_row_by_pn(&self, pn: &InternalPn) -> Result<(String, ComponentRow), LibraryError> {
        for (table, row) in self.iter_rows()? {
            if &row.internal_pn == pn {
                return Ok((table, row));
            }
        }
        Err(LibraryError::NotFound(format!("internal_pn {pn}")))
    }

    fn insert_row(&self, table: &str, row: ComponentRow, msg: &str) -> Result<(), LibraryError> {
        let table_owned = table.to_string();
        let row_id = row.row_id;
        let lib_row = component_to_library_row(&row)?;
        let fallback = format!("insert row {row_id} into {table_owned}");
        self.mutate_library_file(
            move |lf| {
                let entry = lf
                    .tables
                    .entry(table_owned.clone())
                    .or_insert_with(|| LibraryTable {
                        columns: legacy_columns(),
                        rows: Vec::new(),
                        column_types: std::collections::BTreeMap::new(),
                    });
                validate_legacy_header(&table_owned, &entry.columns)?;
                entry.rows.push(lib_row);
                Ok(())
            },
            msg,
            &fallback,
        )
    }

    fn update_row(&self, table: &str, row: ComponentRow, msg: &str) -> Result<(), LibraryError> {
        let table_owned = table.to_string();
        let row_id = row.row_id;
        let lib_row = component_to_library_row(&row)?;
        let fallback = format!("update row {row_id} in {table_owned}");
        self.mutate_library_file(
            move |lf| {
                let entry = lf.tables.get_mut(&table_owned).ok_or_else(|| {
                    LibraryError::NotFound(format!("table {table_owned} not in library"))
                })?;
                validate_legacy_header(&table_owned, &entry.columns)?;
                let row_id_s = row_id.to_string();
                let target = entry.rows.iter_mut().find(|r| {
                    r.cells.get(LEGACY_ROW_ID_COL).map(String::as_str) == Some(row_id_s.as_str())
                });
                match target {
                    Some(slot) => {
                        *slot = lib_row;
                        Ok(())
                    }
                    None => Err(LibraryError::NotFound(format!(
                        "row {row_id} in table {table_owned}"
                    ))),
                }
            },
            msg,
            &fallback,
        )
    }

    fn delete_row(&self, table: &str, row_id: RowId, msg: &str) -> Result<(), LibraryError> {
        let table_owned = table.to_string();
        let fallback = format!("delete row {row_id} from {table_owned}");
        self.mutate_library_file(
            move |lf| {
                let entry = lf.tables.get_mut(&table_owned).ok_or_else(|| {
                    LibraryError::NotFound(format!("table {table_owned} not in library"))
                })?;
                let target = row_id.as_uuid().to_string();
                let before = entry.rows.len();
                entry.rows.retain(|r| {
                    r.cells.get(LEGACY_ROW_ID_COL).map(String::as_str) != Some(target.as_str())
                });
                if entry.rows.len() == before {
                    return Err(LibraryError::NotFound(format!(
                        "row {row_id} in table {table_owned}"
                    )));
                }
                Ok(())
            },
            msg,
            &fallback,
        )
    }

    fn get_symbol(&self, uuid: Uuid) -> Result<Symbol, LibraryError> {
        for (_, file) in self.scan_symbol_files()? {
            if let Some(sym) = file.get_symbol(uuid) {
                return Ok(sym.clone());
            }
        }
        Err(LibraryError::NotFound(format!("symbol {uuid}")))
    }

    fn get_footprint(&self, uuid: Uuid) -> Result<Footprint, LibraryError> {
        self.read_primitive::<Footprint>(PrimitiveKind::Footprint, uuid)
    }

    fn get_sim(&self, uuid: Uuid) -> Result<SimModel, LibraryError> {
        self.read_primitive::<SimModel>(PrimitiveKind::Sim, uuid)
    }

    fn save_symbol(&self, sym: Symbol, message: &str) -> Result<(), LibraryError> {
        let uuid = sym.uuid;
        let new_version = sym.version.clone();
        self.save_symbol_in_container(sym, message)?;
        // Stage 15 cascade: propagate the new symbol version to bound
        // ComponentRows. Personal mode silently auto-bumps everything;
        // Team mode auto-bumps non-released rows + leaves released
        // rows flagged as stale (the Library Browser surface picks
        // them up via the existing stale-binding indicator).
        let mode = self.manifest_synth.workflow.mode;
        let _report = crate::cascade::cascade_after_symbol_save(self, uuid, &new_version, mode)?;
        Ok(())
    }

    fn save_footprint(&self, fp: Footprint, message: &str) -> Result<(), LibraryError> {
        let uuid = fp.uuid;
        let new_version = fp.version.clone();
        self.write_primitive(PrimitiveKind::Footprint, fp.uuid, &fp, message)?;
        let mode = self.manifest_synth.workflow.mode;
        let _report = crate::cascade::cascade_after_footprint_save(self, uuid, &new_version, mode)?;
        Ok(())
    }

    fn save_sim(&self, sm: SimModel, message: &str) -> Result<(), LibraryError> {
        let uuid = sm.uuid;
        let new_version = sm.version.clone();
        self.write_primitive(PrimitiveKind::Sim, sm.uuid, &sm, message)?;
        let mode = self.manifest_synth.workflow.mode;
        let _report = crate::cascade::cascade_after_sim_save(self, uuid, &new_version, mode)?;
        Ok(())
    }

    fn list_symbols(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        let mut out: Vec<PrimitiveSummary> = Vec::new();
        for (_, file) in self.scan_symbol_files()? {
            for sym in &file.symbols {
                out.push(PrimitiveSummary {
                    uuid: sym.uuid,
                    name: sym.name.clone(),
                    kind: PrimitiveKind::Symbol,
                    used_by_count: 0,
                });
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    fn list_footprints(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        self.list_primitive_summaries::<Footprint>(PrimitiveKind::Footprint, |f| &f.name)
    }

    fn list_sims(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        self.list_primitive_summaries::<SimModel>(PrimitiveKind::Sim, |s| &s.name)
    }

    fn root_path(&self) -> Option<PathBuf> {
        Some(self.root_dir.clone())
    }

    fn commit_external_change(&self, abs_path: &Path, message: &str) -> Result<(), LibraryError> {
        let rel_path = abs_path
            .strip_prefix(&self.root_dir)
            .map_err(|_| {
                LibraryError::Backend(format!(
                    "commit_external_change: {} is not under {}",
                    abs_path.display(),
                    self.root_dir.display(),
                ))
            })?
            .to_string_lossy()
            .replace('\\', "/");
        let fallback = format!("save {rel_path}");
        self.commit_path(&rel_path, message, &fallback)
    }

    fn history(&self, primitive_path: &Path) -> Result<Vec<HistoryEntry>, LibraryError> {
        // Stage 17 scaffold: walk the repo's commit graph newest-first
        // and keep commits whose tree differs from at least one parent
        // at `primitive_path`. Mirrors `git log --follow --max-count 50
        // -- <path>` semantics (per `v0.9-snxlib-as-file-plan.md` §3
        // "Performance"), minus the rename-follow heuristic — git2
        // doesn't expose `--follow` directly so a future stage layers
        // it on. For now plain pathspec match is enough; the file
        // names are uuid-keyed so renames are rare in practice.
        const MAX_ENTRIES: usize = 50;

        let rel_path = if primitive_path.is_absolute() {
            primitive_path
                .strip_prefix(&self.root_dir)
                .map_err(|_| {
                    LibraryError::NotFound(format!(
                        "history: {} is not under {}",
                        primitive_path.display(),
                        self.root_dir.display(),
                    ))
                })?
                .to_path_buf()
        } else {
            primitive_path.to_path_buf()
        };
        let rel_str = rel_path.to_string_lossy().replace('\\', "/");

        // Libraries created with `enable_git = false` have no `.git/`
        // and therefore no history to walk. Surface that as an empty
        // log rather than a hard error so the History panel can show
        // an "(no version control)" placeholder.
        if !self.root_dir.join(".git").exists() {
            return Ok(Vec::new());
        }
        let repo = git2::Repository::open(&self.root_dir)
            .map_err(|e| LibraryError::Backend(format!("git open: {e}")))?;

        let mut walk = repo
            .revwalk()
            .map_err(|e| LibraryError::Backend(format!("git revwalk: {e}")))?;
        // Topological + time so that on equal-second timestamps (the
        // common case in tests + back-to-back saves on Windows where
        // git2 stamps to one-second precision) child commits still
        // come before parents — matches `git log`'s default visual
        // order.
        walk.set_sorting(git2::Sort::TIME | git2::Sort::TOPOLOGICAL)
            .map_err(|e| LibraryError::Backend(format!("git revwalk sort: {e}")))?;

        // Unborn HEAD (fresh repo, no commits yet) is a legitimate
        // "no history" answer rather than an error — the editor
        // should render an empty list, not a red banner.
        match walk.push_head() {
            Ok(()) => {}
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => return Ok(Vec::new()),
            Err(e) => return Err(LibraryError::Backend(format!("git push head: {e}"))),
        }

        let mut entries: Vec<HistoryEntry> = Vec::new();
        for oid_res in walk {
            if entries.len() >= MAX_ENTRIES {
                break;
            }
            let oid =
                oid_res.map_err(|e| LibraryError::Backend(format!("git revwalk oid: {e}")))?;
            let commit = repo
                .find_commit(oid)
                .map_err(|e| LibraryError::Backend(format!("git find commit: {e}")))?;

            if !commit_touches_path(&repo, &commit, &rel_str)? {
                continue;
            }

            entries.push(commit_to_history_entry(&commit));
        }
        Ok(entries)
    }
}

// ── Free helpers ───────────────────────────────────────────────────────────

/// Project a `git2::Commit` onto the trait-level [`HistoryEntry`].
///
/// Diff-stat fields (`additions`, `deletions`, `files_changed`) stay
/// at the scaffold defaults — Stage 17 ships the list shape without
/// the lazy diff plumbing. The author timestamp is preferred over
/// the committer's so rebases/cherry-picks don't visually skew the
/// "12 minutes ago" labels.
fn commit_to_history_entry(commit: &git2::Commit<'_>) -> HistoryEntry {
    let author = commit.author();
    let secs = author.when().seconds();
    let time =
        chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0).unwrap_or_else(chrono::Utc::now);
    let raw = commit.message().unwrap_or("");
    let (subject, body) = match raw.find("\n\n") {
        Some(i) => (raw[..i].trim_end().to_string(), raw[i + 2..].to_string()),
        None => (raw.trim_end().to_string(), String::new()),
    };
    HistoryEntry {
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

/// True if `commit` modified `rel_path` relative to *any* of its
/// parents (or, for the root commit, if the path exists in its
/// tree). Mirrors the behaviour of `git log -- <path>` for the simple
/// non-rename case the scaffold targets.
fn commit_touches_path(
    repo: &git2::Repository,
    commit: &git2::Commit<'_>,
    rel_path: &str,
) -> Result<bool, LibraryError> {
    let new_tree = commit
        .tree()
        .map_err(|e| LibraryError::Backend(format!("git commit tree: {e}")))?;

    if commit.parent_count() == 0 {
        // Root commit: include if the path exists in this tree at all.
        return Ok(new_tree.get_path(Path::new(rel_path)).is_ok());
    }

    let mut diff_opts = git2::DiffOptions::new();
    diff_opts.pathspec(rel_path);

    for parent in commit.parents() {
        let old_tree = parent
            .tree()
            .map_err(|e| LibraryError::Backend(format!("git parent tree: {e}")))?;
        let diff = repo
            .diff_tree_to_tree(Some(&old_tree), Some(&new_tree), Some(&mut diff_opts))
            .map_err(|e| LibraryError::Backend(format!("git diff: {e}")))?;
        if diff.deltas().len() > 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

const LEGACY_ROW_ID_COL: &str = "row_id";

/// Header expected for the v0.9 fixed-schema `[tables.<name>]` blocks
/// — same column ordering as the pre-refactor `tables/*.tsv` files
/// so the conversion through [`row_to_record`] / [`record_to_row`]
/// stays bit-exact. Stage 12 lifts the fixed-schema constraint.
fn legacy_columns() -> Vec<String> {
    TABLE_HEADER.iter().map(|s| (*s).to_string()).collect()
}

/// Verify a `[tables.<name>]` block's column order matches the
/// legacy schema. Mismatches are loud — a hand-edited `.snxlib`
/// where someone reordered or renamed columns would silently
/// miscolumn data otherwise.
fn validate_legacy_header(table: &str, columns: &[String]) -> Result<(), LibraryError> {
    if columns.len() != TABLE_HEADER.len() {
        return Err(LibraryError::Backend(format!(
            "table {table:?} schema mismatch: {} columns, expected {}",
            columns.len(),
            TABLE_HEADER.len()
        )));
    }
    for (got, want) in columns.iter().zip(TABLE_HEADER.iter()) {
        if got.as_str() != *want {
            return Err(LibraryError::Backend(format!(
                "table {table:?} schema mismatch: column {got:?}, expected {want:?}"
            )));
        }
    }
    Ok(())
}

fn component_to_library_row(row: &ComponentRow) -> Result<LibraryRow, LibraryError> {
    let cells = row_to_record(row)?;
    let mut lib_row = LibraryRow::default();
    for (col, val) in TABLE_HEADER.iter().zip(cells) {
        lib_row.cells.insert((*col).to_string(), val);
    }
    Ok(lib_row)
}

fn library_row_to_component(row: &LibraryRow) -> Result<ComponentRow, LibraryError> {
    let mut record = csv::StringRecord::new();
    for col in TABLE_HEADER.iter() {
        let val = row.cells.get(*col).map(String::as_str).unwrap_or("");
        record.push_field(val);
    }
    record_to_row(&record)
}

fn synthesize_manifest(snx: &SnxlibManifest) -> Manifest {
    Manifest {
        library: LibraryMeta {
            name: snx.library.name.clone(),
            library_id: snx.library_id,
            description: snx.library.description.clone(),
        },
        mode: snx.mode.clone(),
        workflow: snx.workflow.clone(),
        users: snx.users.clone(),
        // The new model stores tables inside `LibraryFile.tables`, not
        // in the manifest header — leave the legacy field empty so
        // `Manifest::table_for_class` falls back to the mechanical
        // plural until Stage 8/12 retires that caller surface.
        tables: Vec::new(),
    }
}

fn parent_dir(p: &Path) -> Result<PathBuf, LibraryError> {
    p.parent().map(Path::to_path_buf).ok_or_else(|| {
        LibraryError::Backend(format!(
            "library file {} has no parent directory",
            p.display()
        ))
    })
}

fn file_name_str(p: &Path) -> Result<String, LibraryError> {
    p.file_name()
        .and_then(|s| s.to_str())
        .map(str::to_string)
        .ok_or_else(|| {
            LibraryError::Backend(format!(
                "library file path {} has no UTF-8 file name",
                p.display()
            ))
        })
}

fn write_lfs_attributes(root_dir: &Path) -> Result<(), LibraryError> {
    let path = root_dir.join(GITATTRIBUTES_FILE);
    let mut text = String::new();
    text.push_str(
        "# Git LFS attributes for Signex 3D model binaries.\n\
         # Written at library-create time when LFS opt-in was selected.\n",
    );
    for ext in LFS_EXTENSIONS {
        text.push_str(&format!("*.{ext} filter=lfs diff=lfs merge=lfs -text\n"));
    }
    fs::write(&path, text)?;
    Ok(())
}

/// Slugify a human-facing name into a safe filename component.
/// Lowercased, ASCII-only, runs of non-alphanumeric chars collapsed to
/// `-`. Empty result falls back to `"untitled"`.
fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_dash = true;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "untitled".to_string()
    } else {
        out
    }
}

fn primitive_subdir(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Symbol => SYMBOLS_DIR,
        PrimitiveKind::Footprint => FOOTPRINTS_DIR,
        PrimitiveKind::Sim => SIMS_DIR,
    }
}

fn primitive_ext(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Symbol => SYMBOL_EXT,
        PrimitiveKind::Footprint => FOOTPRINT_EXT,
        PrimitiveKind::Sim => SIM_EXT,
    }
}

fn primitive_kind_str(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Symbol => "symbol",
        PrimitiveKind::Footprint => "footprint",
        PrimitiveKind::Sim => "sim",
    }
}

fn identity_for_repo(repo: &git2::Repository) -> (String, String) {
    let cfg = repo.config().ok();
    let name = cfg
        .as_ref()
        .and_then(|c| c.get_string("user.name").ok())
        .unwrap_or_else(|| "Signex Library".to_string());
    let email = cfg
        .as_ref()
        .and_then(|c| c.get_string("user.email").ok())
        .unwrap_or_else(|| "library@signex.local".to_string());
    (name, email)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library_file::{FORMAT_TOKEN, LibrarySection};
    use crate::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
    use crate::primitive::Symbol;
    use uuid::Uuid;

    fn fixture_snx_manifest(name: &str) -> SnxlibManifest {
        SnxlibManifest {
            format: FORMAT_TOKEN.into(),
            library_id: Uuid::now_v7(),
            library: LibrarySection {
                name: name.into(),
                description: None,
            },
            mode: LibraryMode::default(),
            workflow: WorkflowConfig::default(),
            users: UsersConfig::default(),
            classes: Vec::new(),
        }
    }

    fn fixture_snxlib_path(dir: &tempfile::TempDir, name: &str) -> PathBuf {
        dir.path().join(name).join(format!("{name}.{SNXLIB_EXT}"))
    }

    fn fixture_symbol(name: &str) -> Symbol {
        Symbol::empty(name)
    }

    #[test]
    fn init_creates_snxlib_file_and_repo() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture_snxlib_path(&dir, "Test");
        // Default `LibraryInitOptions` flips `enable_git` off; this
        // test predates that change and asserts `.git/` presence.
        // Pass an explicit opts struct with version control on so the
        // assertion still describes meaningful behaviour. The
        // `LibraryInitOptions::default()` path (git off) is now the
        // dominant case but doesn't have a `.git` to check.
        let _adapter = LocalGitAdapter::init(
            &path,
            fixture_snx_manifest("Test"),
            LibraryInitOptions {
                enable_git: true,
                use_lfs: false,
            },
        )
        .expect("init succeeds");
        assert!(path.exists(), "expected .snxlib file at {path:?}");
        assert!(path.parent().unwrap().join(".git").exists());
    }

    #[test]
    fn save_then_load_symbol_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture_snxlib_path(&dir, "Sym");
        let adapter = LocalGitAdapter::init(
            &path,
            fixture_snx_manifest("Sym"),
            LibraryInitOptions::default(),
        )
        .unwrap();
        let sym = fixture_symbol("R");
        let uuid = sym.uuid;
        adapter.save_symbol(sym.clone(), "save R").unwrap();
        let back = adapter.get_symbol(uuid).unwrap();
        assert_eq!(back.uuid, uuid);
        assert_eq!(back.name, "R");
    }

    #[test]
    fn lfs_opt_in_writes_gitattributes() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture_snxlib_path(&dir, "Lfs");
        let adapter = LocalGitAdapter::init(
            &path,
            fixture_snx_manifest("Lfs"),
            LibraryInitOptions {
                enable_git: true,
                use_lfs: true,
            },
        )
        .unwrap();
        let attrs = adapter.root().join(GITATTRIBUTES_FILE);
        assert!(attrs.exists(), "expected .gitattributes when LFS is on");
        let text = fs::read_to_string(&attrs).unwrap();
        for ext in LFS_EXTENSIONS {
            assert!(
                text.contains(&format!("*.{ext} filter=lfs")),
                "missing LFS rule for *.{ext}"
            );
        }
    }

    #[test]
    fn lfs_off_skips_gitattributes() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture_snxlib_path(&dir, "NoLfs");
        let adapter = LocalGitAdapter::init(
            &path,
            fixture_snx_manifest("NoLfs"),
            LibraryInitOptions::default(),
        )
        .unwrap();
        assert!(!adapter.root().join(GITATTRIBUTES_FILE).exists());
    }

    #[test]
    fn rejects_non_snxlib_extension() {
        let dir = tempfile::tempdir().unwrap();
        let bogus = dir.path().join("nope.txt");
        let err = LocalGitAdapter::init(
            &bogus,
            fixture_snx_manifest("Bad"),
            LibraryInitOptions::default(),
        )
        .unwrap_err();
        assert!(matches!(err, LibraryError::Backend(_)));
    }
}
