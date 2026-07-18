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

pub mod project;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

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
    /// HI-11: serialise concurrent git operations. Each `commit_path`
    /// call opens a fresh `git2::Repository` and stages an entry; if
    /// two threads race here, libgit2's index `LOCK` file races on
    /// itself and the second commit fails with `git add: error`.
    /// Holding this mutex across `commit_path` makes the in-process
    /// view of `.git/` linearisable. (Cross-process locking is still
    /// libgit2's responsibility; this only addresses the in-process
    /// race that `RwLock<LibraryFile>` does NOT cover.)
    git_lock: Mutex<()>,
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
            git_lock: Mutex::new(()),
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
            git_lock: Mutex::new(()),
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
            git_lock: Mutex::new(()),
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
mod helpers;
mod primitives;

use helpers::*;

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
