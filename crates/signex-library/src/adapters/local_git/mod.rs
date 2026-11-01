//! Local + git storage adapter — `.snxlib` file backed by libgit2.
//!
//! Per `v0.9-snxlib-as-file-plan.md`, a Signex library on disk is a
//! *directory* containing a `.snxlib` file (the user-facing entry
//! point) and sibling `symbols/` / `footprints/` / `sims/` /
//! `assets/datasheets/` directories. The `.git/` repo lives at the parent
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
//! ├── assets/datasheets/<hash>.pdf (pinned datasheets)
//! ├── .gitattributes               (written when [`LibraryInitOptions::use_lfs`])
//! ├── .gitignore                   (the managed block, with version control)
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

pub mod project;
mod symbol_index;
mod symbol_problems;

pub use symbol_problems::SymbolFileProblem;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use uuid::Uuid;

use crate::adapter::{CommitOutcome, HistoryEntry, LibraryAdapter, LibraryError, PrimitiveSummary};
use crate::component::ComponentRow;
use crate::git_identity::identity_for_repo;
use crate::identity::{InternalPn, RowId};
use crate::library_file::{LibraryFile, LibraryRow, LibraryTable, SnxlibManifest};
use crate::manifest::{LibraryMeta, Manifest};
use crate::primitive::{Footprint, PrimitiveKind, SimFile, SimModel, Symbol, SymbolFile};
use crate::tables::{
    ALTERNATE_SYMBOLS_COLUMN, TABLE_HEADER, alternate_symbols_cell, record_to_row, row_to_record,
};

const SYMBOLS_DIR: &str = "symbols";
const FOOTPRINTS_DIR: &str = "footprints";
const SIMS_DIR: &str = "sims";

/// The folders beside a `.snxlib` that belong to the library. The one list
/// the Track picker and Re-init both read.
pub const LIBRARY_SUBFOLDERS: [&str; 5] =
    ["classes", SYMBOLS_DIR, FOOTPRINTS_DIR, SIMS_DIR, "3dmodels"];
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
    /// HI-11: serialise concurrent git operations. Each `commit_path`
    /// call opens a fresh `git2::Repository` and stages an entry; if
    /// two threads race here, libgit2's index `LOCK` file races on
    /// itself and the second commit fails with `git add: error`.
    /// Holding this mutex across `commit_path` makes the in-process
    /// view of `.git/` linearisable. (Cross-process locking is still
    /// libgit2's responsibility; this only addresses the in-process
    /// race that `RwLock<LibraryFile>` does NOT cover.)
    git_lock: Mutex<()>,
    /// `.snxsym` containers the last symbol scan could not read (#690).
    symbol_problems: Mutex<symbol_problems::SymbolProblemLedger>,
    /// uuid -> `.snxsym` container path, a hint only: every use re-reads
    /// the file and falls back to a rescan on mismatch (#1362).
    symbol_index: symbol_index::SymbolIndex,
    /// `.snxfpt` files already warned about, keyed by (path, mtime) (#924).
    skipped_footprints: Mutex<std::collections::HashSet<(PathBuf, std::time::SystemTime)>>,
    /// Stamp and content hash of the `.snxlib` text behind `library_file`
    /// (#531). `None` forces a reload on the next access. Lock order:
    /// `library_file` before `disk_stamp`.
    disk_stamp: Mutex<Option<disk_sync::KnownDisk>>,
    /// Last on-disk state whose failed reload was already logged, so a
    /// persistently unreadable or missing file warns once, not on every
    /// read.
    warned_state: Mutex<Option<disk_sync::DiskState>>,
    /// The "cannot lock on this filesystem" warning was logged (#832).
    write_lock_warned: std::sync::atomic::AtomicBool,
    /// How many plain-text database token warnings this adapter logged: 0
    /// or 1 (see `plain_token`).
    plain_token_warnings: std::sync::atomic::AtomicUsize,
    /// Encoded row lines from the last row write (#416). Lock order: after
    /// `library_file` and `disk_stamp`.
    row_lines: Mutex<crate::library_file::RowLines>,
    /// Files saved whose commit failed (#858); see `uncommitted`.
    uncommitted: Mutex<uncommitted::UncommittedLedger>,
    /// Converted rows served to readers (#1437). Lock order: after
    /// `library_file`.
    row_reads: read_cache::RowReads,
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
        if opts.enable_git {
            // Fail before anything lands on disk when no user identity
            // resolves, so a refused init leaves no half-made library.
            crate::git_identity::identity_for_default_config()?;
        }
        fs::create_dir_all(&root_dir)?;

        let library_file = LibraryFile {
            layout: Default::default(),
            manifest: snx_manifest,
            tables: std::collections::BTreeMap::new(),
        };
        let text = library_file.write()?;
        signex_types::atomic_io::atomic_write(&file_path, text.as_bytes())?;
        let disk_stamp = disk_sync::DiskStamp::of(&file_path)?;

        // Git scaffolding — only when the user opted in. Without
        // `enable_git` the library lives on disk as plain files and
        // every mutation operation is a best-effort `commit_path`
        // that no-ops when no `.git/` is present.
        if opts.enable_git {
            // LFS attributes go in BEFORE `git init` so the initial commit
            // already has `.gitattributes` staged; otherwise libgit2's
            // index update sequence gets fiddly.
            if opts.use_lfs {
                // Keep a `.gitattributes` the folder already has: only the
                // missing LFS rules are added (#1740).
                ensure_lfs_attributes(&root_dir)?;
            }

            let repo = git2::Repository::init(&root_dir)
                .map_err(|e| LibraryError::Backend(format!("git init: {e}")))?;
            commit_initial(&repo, &file_path, opts.use_lfs, None)?;
        }

        let manifest_synth = synthesize_manifest(&library_file.manifest);
        Ok(Self {
            file_path,
            root_dir,
            library_file: RwLock::new(library_file),
            manifest_synth,
            git_lock: Mutex::new(()),
            symbol_problems: Mutex::default(),
            symbol_index: Default::default(),
            skipped_footprints: Mutex::default(),
            disk_stamp: Mutex::new(Some(disk_sync::KnownDisk::new(disk_stamp, &text))),
            warned_state: Mutex::new(None),
            write_lock_warned: Default::default(),
            plain_token_warnings: Default::default(),
            row_lines: Mutex::default(),
            uncommitted: Mutex::default(),
            row_reads: Default::default(),
        })
    }

    /// Finish the version-control scaffolding a failed [`Self::init`] left
    /// behind: a `.snxlib` that is already on disk, but whose `.git/` is
    /// missing or has no commit, or whose LFS rules are missing. Only what
    /// `opts` asks for is touched; with `enable_git` off nothing is. Never
    /// creates the `.snxlib` itself.
    ///
    /// Refuses when the directory is a repository that has history but does
    /// not track this `.snxlib` (the project's own repo, say): the library's
    /// commit must never land in, or sweep up, another repo's work. A parent
    /// is used only to add LFS rules on top of the library's own first commit.
    pub fn complete_init(
        file_path: impl AsRef<Path>,
        opts: LibraryInitOptions,
    ) -> Result<(), LibraryError> {
        let file_path = file_path.as_ref().to_path_buf();
        Self::validate_file_path(&file_path)?;
        if !opts.enable_git {
            return Ok(());
        }
        let root_dir = parent_dir(&file_path)?;
        let repo = git2::Repository::init(&root_dir)
            .map_err(|e| LibraryError::Backend(format!("git init: {e}")))?;
        let head = reinit_tree::refuse_foreign_history(&repo, &file_path, &root_dir)?;
        let wrote_attributes = opts.use_lfs && ensure_lfs_attributes(&root_dir)?;
        match &head {
            None => commit_initial(&repo, &file_path, opts.use_lfs, None),
            Some(head) if wrote_attributes => {
                commit_initial(&repo, &file_path, opts.use_lfs, Some(head))
            }
            Some(_) => Ok(()),
        }
    }

    /// Open an existing library by `.snxlib` file path. The file's
    /// parent directory must already host a `.git/` — recovery from
    /// a deleted git repo lives in [`Self::recover_init`].
    pub fn open(file_path: impl AsRef<Path>) -> Result<Self, LibraryError> {
        let file_path = file_path.as_ref().to_path_buf();
        Self::validate_file_path(&file_path)?;
        require_library_file(&file_path)?;
        // Stamp first: a write racing the read then costs one extra reload.
        let disk_stamp = disk_sync::DiskStamp::of(&file_path)?;
        let text = fs::read_to_string(&file_path)?;
        let library_file =
            LibraryFile::parse_with(&text, &crate::tables::repair_short_component_row)?;
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
        let adapter = Self {
            file_path,
            root_dir,
            library_file: RwLock::new(library_file),
            manifest_synth,
            git_lock: Mutex::new(()),
            symbol_problems: Mutex::default(),
            symbol_index: Default::default(),
            skipped_footprints: Mutex::default(),
            disk_stamp: Mutex::new(Some(disk_sync::KnownDisk::new(disk_stamp, &text))),
            warned_state: Mutex::new(None),
            write_lock_warned: Default::default(),
            plain_token_warnings: Default::default(),
            row_lines: Mutex::default(),
            uncommitted: Mutex::default(),
            row_reads: Default::default(),
        };
        adapter.seed_uncommitted_from_working_tree();
        Ok(adapter)
    }

    /// The commit of [`Self::recover_init`]: HEAD's tree plus the library's
    /// own files, on top of HEAD when there is one.
    ///
    /// `head_recreated` is true when this Re-init made `.git/HEAD` anew (it
    /// was gone before `git init`): only then is the branch chosen for the
    /// user. A HEAD that was there, even on an unborn orphan branch, stays.
    fn reinit_commit(
        repo: &git2::Repository,
        file_path: &Path,
        root_dir: &Path,
        head_recreated: bool,
    ) -> Result<(), LibraryError> {
        let (sig_name, sig_email) = identity_for_repo(repo)?;
        let sig = git2::Signature::now(&sig_name, &sig_email)
            .map_err(|e| LibraryError::Backend(format!("git signature: {e}")))?;

        // Built in memory from HEAD's tree (empty when unborn), so what the
        // user has staged in this repo is neither committed nor touched.
        // A re-made HEAD points at `init.defaultBranch`, which may or may not
        // exist. Whatever it resolves to, the documented priority picks the
        // branch that tracks the library. The foreign-history check is skipped
        // for it, as a tracking tip cannot be foreign; when none tracks, the
        // check runs (the repo may only now be readable, after `git init`
        // put a `.git/` right that would not open before).
        let tracking = if head_recreated {
            reinit_tree::tracking_branch(repo, file_path)?
        } else {
            None
        };
        let (branch_ref, parent) = if let Some((refname, tip)) = tracking {
            (Some(refname), Some(tip))
        } else {
            reinit_tree::refuse_foreign_history(repo, file_path, root_dir)?;
            let parent = match repo.head() {
                Ok(h) => h
                    .peel_to_commit()
                    .map_err(|e| LibraryError::Backend(format!("git peel to commit: {e}")))
                    .map(Some)?,
                Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
                Err(e) => return Err(LibraryError::Backend(format!("git head: {e}"))),
            };
            (None, parent)
        };
        let snapshot = reinit_tree::library_tree(repo, file_path, root_dir, parent.as_ref())?;
        if let Some(refname) = &branch_ref {
            repo.set_head(refname)
                .map_err(|e| LibraryError::Backend(format!("git set head: {e}")))?;
        }
        let tree = repo
            .find_tree(snapshot.tree)
            .map_err(|e| LibraryError::Backend(format!("git find tree: {e}")))?;
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
        // The commit has landed: a failure here must not read as a failed
        // Re-init (a retry would stack a second commit).
        if let Err(e) = reinit_tree::record_in_index(repo, &snapshot) {
            tracing::warn!(
                target: "signex::library",
                "Re-init committed, but the git index could not be updated: {e}"
            );
        }
        Ok(())
    }

    /// Recover a library whose `.git/` directory was deleted
    /// out-from-under-it. Re-runs `git init` at the parent directory
    /// and commits the library's own files (see `reinit_tree`) as a
    /// fresh "snxlib re-init" commit; other files in the folder stay
    /// untouched. Refuses a folder that is another repo with history that
    /// does not track this library. Past history is lost — the recovery
    /// dialog (Stage 10) is responsible for warning the user before
    /// landing here.
    pub fn recover_init(file_path: impl AsRef<Path>) -> Result<Self, LibraryError> {
        let file_path = file_path.as_ref().to_path_buf();
        Self::validate_file_path(&file_path)?;
        require_library_file(&file_path)?;
        let root_dir = parent_dir(&file_path)?;

        // Refuse before `git init` touches disk: an unresolvable user
        // identity must not leave a fresh, unborn `.git/` behind (that
        // would hide the GitMissing recovery on the next open).
        // When a repo already exists (re-init of a repo whose `.git` was not
        // deleted) its own config counts, as it does for every later commit.
        let preflight = match git2::Repository::open(&root_dir) {
            Ok(existing) => identity_for_repo(&existing),
            Err(_) => crate::git_identity::identity_for_default_config(),
        };
        preflight?;

        // Another repo's history is never written into: refuse before
        // `git init` or any staging touches the folder (#1741).
        if let Ok(existing) = git2::Repository::open(&root_dir) {
            reinit_tree::refuse_foreign_history(&existing, &file_path, &root_dir)?;
        }

        // `git2::Repository::init` is idempotent on an already-init'd
        // working tree, so this also handles the "git was *not*
        // deleted" case as a no-op + re-commit-everything. The recovery
        // dialog is the gate; the adapter just does what it's asked.
        // Only a `.git/` this call makes is ever removed again: one that is
        // already there but will not open is the user's recoverable history.
        let git_meta = fs::symlink_metadata(root_dir.join(".git"));
        let created_repo = git_meta.is_err();
        // `git init` makes a missing `HEAD` anew; a present one is left alone.
        let head_recreated = match &git_meta {
            Ok(meta) if meta.is_dir() => fs::symlink_metadata(root_dir.join(".git/HEAD")).is_err(),
            Ok(_) => false,
            Err(_) => true,
        };
        let repo = git2::Repository::init(&root_dir)
            .map_err(|e| LibraryError::Backend(format!("git re-init: {e}")))?;
        // A fresh, unborn `.git/` left by a refused Re-init would hide the
        // GitMissing recovery on the next open: take it away again.
        Self::reinit_commit(&repo, &file_path, &root_dir, head_recreated).inspect_err(|_| {
            if created_repo && let Err(e) = fs::remove_dir_all(root_dir.join(".git")) {
                tracing::warn!(
                    target: "signex::library",
                    "Re-init could not remove the unborn .git it made in {}: {e}",
                    root_dir.display()
                );
            }
        })?;
        // Stamp first: a write racing the read then costs one extra reload.
        let disk_stamp = disk_sync::DiskStamp::of(&file_path)?;
        let text = fs::read_to_string(&file_path)?;
        let library_file =
            LibraryFile::parse_with(&text, &crate::tables::repair_short_component_row)?;
        let manifest_synth = synthesize_manifest(&library_file.manifest);
        Ok(Self {
            file_path,
            root_dir,
            library_file: RwLock::new(library_file),
            manifest_synth,
            git_lock: Mutex::new(()),
            symbol_problems: Mutex::default(),
            symbol_index: Default::default(),
            skipped_footprints: Mutex::default(),
            disk_stamp: Mutex::new(Some(disk_sync::KnownDisk::new(disk_stamp, &text))),
            warned_state: Mutex::new(None),
            write_lock_warned: Default::default(),
            plain_token_warnings: Default::default(),
            row_lines: Mutex::default(),
            uncommitted: Mutex::default(),
            row_reads: Default::default(),
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
}

mod adapter;
mod commit_lock;
mod disk_sync;
/// `add_all` callback: stage everything except a write-lock file (#832).
pub(crate) fn skip_write_lock_files(path: &Path, _spec: &[u8]) -> i32 {
    i32::from(is_write_lock_file(path))
}

pub use disk_sync::DiskStamp;
pub use write_lock::{
    WRITE_LOCK_WAIT, enclosing_library_file, is_write_lock_file, rewrite_file_under_library_lock,
    write_lock_path,
};
mod helpers;
mod ignore_rules;
mod reinit_tree;
pub use helpers::component_to_library_row;
#[cfg(test)]
mod ledger_hygiene_tests;
pub(crate) mod lfs;
mod pinned_assets;
mod primitives;
mod read_cache;
mod row_edit;
mod uncommitted;
#[cfg(test)]
mod uncommitted_tests;
mod write_lock;

use helpers::*;
use row_edit::RowChange;

#[cfg(test)]
mod write_oracle_tests;

#[cfg(test)]
mod insert_row_tests;
mod plain_token;
#[cfg(test)]
mod plain_token_tests;
#[cfg(test)]
mod read_cache_tests;

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

    /// #924 — a skipped footprint is reported once per (path, mtime), and
    /// again after the file changes.
    #[test]
    fn skipped_footprint_warns_once_per_file_version() {
        crate::git_identity::test_identity::install();
        let dir = tempfile::tempdir().unwrap();
        let path = fixture_snxlib_path(&dir, "Dedup");
        let adapter = LocalGitAdapter::init(
            &path,
            fixture_snx_manifest("Dedup"),
            LibraryInitOptions {
                enable_git: true,
                use_lfs: false,
            },
        )
        .unwrap();
        let bad = dir.path().join("bad.snxfpt");
        std::fs::write(&bad, "a").unwrap();
        assert!(adapter.warn_footprint_skipped_once(&bad, "e"));
        assert!(!adapter.warn_footprint_skipped_once(&bad, "e"));
        let f = std::fs::OpenOptions::new().write(true).open(&bad).unwrap();
        f.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5))
            .unwrap();
        assert!(adapter.warn_footprint_skipped_once(&bad, "e"));
    }

    #[test]
    fn init_creates_snxlib_file_and_repo() {
        crate::git_identity::test_identity::install();
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

    /// #149: committing a path whose content matches HEAD makes no commit.
    #[test]
    fn commit_path_skips_unchanged_tree() {
        crate::git_identity::test_identity::install();
        let dir = tempfile::tempdir().unwrap();
        let path = fixture_snxlib_path(&dir, "Same");
        let adapter = LocalGitAdapter::init(
            &path,
            fixture_snx_manifest("Same"),
            LibraryInitOptions {
                enable_git: true,
                use_lfs: false,
            },
        )
        .unwrap();
        let repo = git2::Repository::open(adapter.root()).unwrap();
        let head = || repo.head().unwrap().target().unwrap();

        fs::write(adapter.root().join("note.txt"), "a").unwrap();
        assert!(
            adapter
                .commit_path("note.txt", "", "add note")
                .is_recorded()
        );
        let first = head();
        assert!(adapter.commit_path("note.txt", "", "again").is_recorded());
        assert_eq!(head(), first, "unchanged tree must not commit");

        fs::write(adapter.root().join("note.txt"), "b").unwrap();
        assert!(
            adapter
                .commit_path("note.txt", "", "edit note")
                .is_recorded()
        );
        assert_ne!(head(), first, "a real change still commits");
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
        crate::git_identity::test_identity::install();
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

    /// #1740: a `.gitattributes` the folder already has keeps its rules; only
    /// the missing LFS lines are added, each once.
    #[test]
    fn lfs_init_keeps_existing_gitattributes_rules() {
        crate::git_identity::test_identity::install();
        let dir = tempfile::tempdir().unwrap();
        let path = fixture_snxlib_path(&dir, "LfsKeep");
        let custom = "*.sch text eol=lf\n*.glb filter=lfs diff=lfs merge=lfs -text\n";
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path.parent().unwrap().join(GITATTRIBUTES_FILE), custom).unwrap();
        let adapter = LocalGitAdapter::init(
            &path,
            fixture_snx_manifest("LfsKeep"),
            LibraryInitOptions {
                enable_git: true,
                use_lfs: true,
            },
        )
        .unwrap();
        let text = fs::read_to_string(adapter.root().join(GITATTRIBUTES_FILE)).unwrap();
        assert!(text.lines().any(|l| l == "*.sch text eol=lf"), "{text}");
        for ext in LFS_EXTENSIONS {
            let rule = format!("*.{ext} filter=lfs diff=lfs merge=lfs -text");
            assert_eq!(
                text.lines().filter(|l| l.trim() == rule).count(),
                1,
                "{rule}"
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
