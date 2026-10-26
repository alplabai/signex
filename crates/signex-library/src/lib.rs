//! Signex component library subsystem (v0.9-refactor-2 — DBLib model).
//!
//! Per `v0.9-refactor-2-plan.md`, components are **rows in TSV/JSONB
//! tables**, not files. Each row references reusable primitives (`Symbol`,
//! `Footprint`, `SimModel`) by `(library_id, uuid)` tuples ([`PrimitiveRef`])
//! instead of embedding their geometry. Symbols / footprints / sims stay as
//! standalone editable primitive files; the row binds them with metadata.

pub mod adapter;
pub mod adapters;
#[cfg(feature = "ai-stub")]
pub mod ai_stub;
pub mod cascade;
pub mod component;
pub mod dangling;
pub mod diff;
pub mod distributor;
#[cfg(feature = "distributors-community")]
pub mod distributors;
#[cfg(feature = "local-git")]
pub mod git_identity;
pub mod hash;
pub mod identity;
pub mod library_file;
pub mod lifecycle;
pub mod manifest;
pub mod manufacturer;
pub mod param;
pub mod pricing;
pub mod pricing_refresh;
pub mod primitive;
pub mod search;
#[cfg(feature = "search-tantivy")]
pub mod search_index;
pub mod tables;
pub mod templates;
#[cfg(feature = "local-git")]
mod vcs_init_files;
pub mod where_used;

pub use adapter::{
    ComponentSummary, FileRewrite, HistoryEntry, LibraryAdapter, LibraryError, LibraryQuery,
    PrimitiveSummary, rewrite_file,
};
pub use adapters::library_set::{LibrarySet, UnresolvedRefs};
pub use adapters::read_only::ReadOnlyAdapter;
#[cfg(feature = "ai-stub")]
pub use ai_stub::{PinGuess, PinoutGuess, extract_pinout};
pub use cascade::{
    CascadeKind, CascadeReport, PrimitiveSave, cascade_after_footprint_save, cascade_after_saves,
    cascade_after_sim_save, cascade_after_symbol_save, patch_bump,
};
pub use component::{ComponentRow, DatasheetDoc, DatasheetRef, PinPadOverride, PlmReserved};
pub use dangling::{DanglingRef, RefSlot, dangling_row_refs};
pub use diff::{
    BumpKind, LifecycleDiff, ListDiff, ParameterDiff, PinMapDiff, RowDiff, auto_bump_kind,
    diff_rows,
};
pub use distributor::{DistributorAdapter, DistributorError, DistributorPart, DistributorSource};
#[cfg(feature = "distributors-community")]
pub use distributors::{
    DigiKeyAdapter, DistributorCache, JlcpcbAdapter, KeyringStore, LcscAdapter, MouserAdapter,
    PRICING_SOURCES, source_supports_pricing,
};
pub use hash::{ensure_row_params_finite, hash_row_content};
pub use identity::{ComponentClass, InternalPn, Mpn, RowId};
pub use library_file::{
    ClassEntry, FORMAT_TOKEN, LibraryFile, LibraryFileError, LibraryRow, LibrarySection,
    LibraryTable, SnxlibManifest,
};
pub use lifecycle::LifecycleState;
pub use manifest::{
    LibraryMeta, LibraryMode, Manifest, TableConfig, UserEntry, UsersConfig, WorkflowConfig,
    WorkflowMode,
};
pub use manufacturer::{AlternateStatus, DistributorListing, ManufacturerPart};
pub use param::{ParamMap, ParamValue};
pub use pricing::{
    CachedPricing, DroppedUpdate, PricingError, PricingUpdate, RowPricing, StoreOutcome, age_label,
    current_pricing, distributor_key, format_price,
};
pub use pricing_refresh::{
    ClientResult, ClientUnavailable, FailureKind, PricingRefreshReport, RowFailure,
    refresh_library_pricing, refresh_pricing_for_rows,
};
pub use primitive::{
    Body3D, BodyShape, CHAIN_ARC_SAMPLES, CHAIN_ENDPOINT_EPSILON_MM, ChainError, ChainSegment,
    ComponentType, ContainerMerge, Drill, DuplicatedUuid, Footprint, FootprintFile,
    FootprintFileError, FpGraphic, FpGraphicKind, LayerId, LoadRepair, Pad, PadKind, PadShape,
    PinDirection, PinOrientation, PinSymbolKind, Polygon, PrimitiveKind, PrimitiveRef, SimKind,
    SimModel, StepAttachment, Symbol, SymbolFile, SymbolGraphic, SymbolGraphicKind, SymbolPin,
    chain_into_closed_contour, normalize_arc_endpoints_deg,
};
pub use search::{Facet, FacetOp, SearchIndex, SearchQuery};
#[cfg(feature = "search-tantivy")]
pub use search_index::{TantivyIndexError, TantivySearchIndex};
pub use tables::{
    TABLE_HEADER, TableSchema, append_row, delete_row, read_table, update_row, write_table,
};
pub use templates::{ParamKind, ParamSlot, ParameterTemplate, TemplateRegistry, TemplateViolation};
pub use where_used::{UseSite, WhereUsedIndex};

#[cfg(feature = "local-git")]
pub use adapters::local_git::{
    DiskStamp, LocalGitAdapter, WRITE_LOCK_WAIT, enclosing_library_file, is_write_lock_file,
    rewrite_file_under_library_lock, write_lock_path,
};

/// Project-level "Enable Version Control" helper. Runs
/// `git2::Repository::init` at `project_dir`, merges Signex's managed
/// block into `.gitignore` (and, when `use_lfs`, into `.gitattributes`),
/// then stages every tracked file and creates the initial commit
/// "chore: enable version control".
///
/// Existing `.gitignore` / `.gitattributes` files are merged, never
/// replaced: only the block between the `# >>> signex (managed) >>>` /
/// `# <<< signex (managed) <<<` markers is Signex's. `managed_gitignore`
/// turns the `.gitignore` block on (project scope); library scope passes
/// `false` and gets only the rows. Links and read-only files are refused,
/// and so are hard-linked files on Unix only (stable std has no link count
/// on Windows). Any failure after the files are written restores them and
/// removes the `.git` this call made. `gitignore` is the
/// per-project tracking-scope picker's unticked rows; they are appended
/// outside the block, skipping lines the file already carries. Duplicate
/// or unbalanced markers refuse with [`adapter::LibraryError::Conflict`]
/// and leave the files untouched.
///
/// Used by `signex-app` so the per-project Enable Version Control
/// flow doesn't need to pull `git2` in directly. Errors propagate
/// through the existing [`adapter::LibraryError`] variants so the UI
/// can surface them in one place.
#[cfg(feature = "local-git")]
pub fn enable_project_version_control(
    project_dir: &std::path::Path,
    use_lfs: bool,
    managed_gitignore: bool,
    gitignore: Option<&str>,
) -> Result<(), adapter::LibraryError> {
    use adapter::LibraryError;

    // First statement, before anything touches disk: a refusal must leave a
    // user's existing .gitignore / .gitattributes exactly as they were.
    let (sig_name, sig_email) = git_identity::identity_for_default_config()?;

    // Any entry named `.git` (even a dangling link) is not ours to replace.
    if std::fs::symlink_metadata(project_dir.join(".git")).is_ok() {
        return Err(LibraryError::Conflict(format!(
            "{} already has a .git directory",
            project_dir.display()
        )));
    }
    std::fs::create_dir_all(project_dir).map_err(|e| {
        LibraryError::Backend(format!("create project dir {}: {e}", project_dir.display()))
    })?;

    // Merge the managed blocks into the user's files in memory first, so a
    // malformed marker pair or a link refuses before anything touches disk.
    let mut files =
        vcs_init_files::VcsFilePlan::new(project_dir, use_lfs, managed_gitignore, gitignore)?;
    if let Err(e) = files.write() {
        return Err(with_rollback_notes(e, files.roll_back()));
    }

    // From here on any failure restores the user's files, and removes the
    // `.git` this call created (the check above guarantees there was none).
    if let Err(e) = init_and_commit(project_dir, use_lfs, &sig_name, &sig_email) {
        let mut notes = files.roll_back();
        notes.extend(vcs_init_files::remove_new_git_dir(project_dir));
        return Err(with_rollback_notes(e, notes));
    }
    Ok(())
}

/// Append rollback failures to `error` so the user hears about them.
#[cfg(feature = "local-git")]
fn with_rollback_notes(error: adapter::LibraryError, notes: Vec<String>) -> adapter::LibraryError {
    use adapter::LibraryError;
    if notes.is_empty() {
        return error;
    }
    let notes = notes.join("; ");
    match error {
        LibraryError::Conflict(m) => LibraryError::Conflict(format!("{m}; {notes}")),
        LibraryError::Backend(m) => LibraryError::Backend(format!("{m}; {notes}")),
        other => LibraryError::Backend(format!("{other}; {notes}")),
    }
}

/// `git init`, stage the tree and make the initial commit.
#[cfg(feature = "local-git")]
fn init_and_commit(
    project_dir: &std::path::Path,
    use_lfs: bool,
    sig_name: &str,
    sig_email: &str,
) -> Result<(), adapter::LibraryError> {
    use adapter::LibraryError;

    let repo = git2::Repository::init(project_dir)
        .map_err(|e| LibraryError::Backend(format!("git init: {e}")))?;

    let sig = git2::Signature::now(sig_name, sig_email)
        .map_err(|e| LibraryError::Backend(format!("git signature: {e}")))?;

    let mut index = repo
        .index()
        .map_err(|e| LibraryError::Backend(format!("git index: {e}")))?;
    index
        .add_all(
            ["."].iter(),
            git2::IndexAddOption::DEFAULT,
            Some(&mut adapters::local_git::skip_write_lock_files),
        )
        .map_err(|e| LibraryError::Backend(format!("git add .: {e}")))?;
    // libgit2 runs no `git-lfs` filter: store the `filter=lfs` matches as
    // LFS objects and commit their pointers, as `git add` would (#1121).
    let converted = adapters::local_git::lfs::convert_index(&repo, &mut index)
        .map_err(|e| LibraryError::Backend(format!("git lfs: {e}")))?;
    // Asking for LFS installs the push hook even with no model yet, so a
    // model committed later still has its object uploaded.
    if use_lfs || converted > 0 {
        adapters::local_git::lfs::install_pre_push_hook(&repo)
            .map_err(|e| LibraryError::Backend(format!("git lfs hook: {e}")))?;
    }
    index
        .write()
        .map_err(|e| LibraryError::Backend(format!("git index write: {e}")))?;
    // Refuse to land a commit with nothing real in it. The ignore and
    // attributes files Signex just wrote do not count: a `git status` after
    // such a commit would treat every later-added file as untracked. The
    // realistic path is "user clicked Enable Version Control on a
    // freshly-minted project that hasn't been saved yet" - surface that as
    // an actionable error.
    let has_project_files = index.iter().any(|entry| {
        let path = String::from_utf8_lossy(&entry.path);
        path != ".gitignore" && path != ".gitattributes"
    });
    if !has_project_files {
        return Err(LibraryError::Backend(
            "project directory has no files to commit — save the project first, \
             then re-enable version control"
                .into(),
        ));
    }
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
        "chore: enable version control",
        &tree,
        &[],
    )
    .map_err(|e| LibraryError::Backend(format!("git initial commit: {e}")))?;
    Ok(())
}

/// Walk the commit graph at `project_dir` and return up to 50
/// commits that touched `rel_path` (relative to `project_dir`).
///
/// Mirrors [`crate::adapters::local_git::LocalGitAdapter::history`]
/// but works on **any** git repository — not just library-rooted
/// ones. Used by `signex-app`'s right-dock History panel to show
/// the active tab's file history regardless of whether the file
/// lives inside a `.snxlib` or in a plain Signex project.
///
/// Returns `Ok(vec![])` when the path has no commits yet (fresh
/// repo, untracked file, unborn HEAD). Returns
/// `Err(LibraryError::NotFound)` when `project_dir` has no `.git/`.
#[cfg(feature = "local-git")]
pub fn project_file_history(
    project_dir: &std::path::Path,
    rel_path: &std::path::Path,
) -> Result<Vec<adapter::HistoryEntry>, adapter::LibraryError> {
    use adapter::{HistoryEntry, LibraryError};
    use std::path::Path;

    const MAX_ENTRIES: usize = 50;

    if !project_dir.join(".git").exists() {
        return Err(LibraryError::NotFound(format!(
            "no .git/ at {}",
            project_dir.display()
        )));
    }

    // Normalise the pathspec: strip a leading `project_dir` prefix
    // when the caller passed an absolute path, otherwise take
    // `rel_path` verbatim. Use forward slashes so libgit2's pathspec
    // matches on Windows the same way it does on POSIX.
    let rel_buf = if rel_path.is_absolute() {
        rel_path
            .strip_prefix(project_dir)
            .map_err(|_| {
                LibraryError::NotFound(format!(
                    "history: {} is not under {}",
                    rel_path.display(),
                    project_dir.display(),
                ))
            })?
            .to_path_buf()
    } else {
        rel_path.to_path_buf()
    };
    let rel_str = rel_buf.to_string_lossy().replace('\\', "/");

    let repo = git2::Repository::open(project_dir)
        .map_err(|e| LibraryError::Backend(format!("git open: {e}")))?;

    let mut walk = repo
        .revwalk()
        .map_err(|e| LibraryError::Backend(format!("git revwalk: {e}")))?;
    walk.set_sorting(git2::Sort::TIME | git2::Sort::TOPOLOGICAL)
        .map_err(|e| LibraryError::Backend(format!("git revwalk sort: {e}")))?;

    // Unborn HEAD (fresh repo, no commits yet) is "no history",
    // not an error. libgit2 reports this as either UnbornBranch
    // (when HEAD points at an unborn ref) or NotFound (when HEAD
    // itself can't be peeled). Both surfaces fold to an empty Vec
    // so the History panel can render a "No commits yet" card.
    match walk.push_head() {
        Ok(()) => {}
        Err(e)
            if matches!(
                e.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) || e.class() == git2::ErrorClass::Reference =>
        {
            // Unborn HEAD: libgit2 reports either UnbornBranch (POSIX
            // path), NotFound (some macOS variants), or a generic
            // Reference-class error on Windows where HEAD points at
            // `refs/heads/master` which doesn't exist yet. All three
            // surfaces fold to an empty Vec so the History panel can
            // render a "No commits yet" card.
            return Ok(Vec::new());
        }
        Err(e) => return Err(LibraryError::Backend(format!("git push head: {e}"))),
    }

    let mut entries: Vec<HistoryEntry> = Vec::new();
    for oid_res in walk {
        if entries.len() >= MAX_ENTRIES {
            break;
        }
        let oid = oid_res.map_err(|e| LibraryError::Backend(format!("git revwalk oid: {e}")))?;
        let commit = repo
            .find_commit(oid)
            .map_err(|e| LibraryError::Backend(format!("git find commit: {e}")))?;

        if !commit_touches_path(&repo, &commit, &rel_str)? {
            continue;
        }

        entries.push(commit_to_history_entry(&commit));
    }

    // Local helpers — duplicated from local_git.rs deliberately so
    // this routine doesn't depend on adapter internals (the adapter
    // version is library-rooted; this one walks any repo).
    fn commit_to_history_entry(commit: &git2::Commit<'_>) -> HistoryEntry {
        let author = commit.author();
        let secs = author.when().seconds();
        let time = chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0)
            .unwrap_or_else(chrono::Utc::now);
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

    fn commit_touches_path(
        repo: &git2::Repository,
        commit: &git2::Commit<'_>,
        rel_path: &str,
    ) -> Result<bool, LibraryError> {
        let new_tree = commit
            .tree()
            .map_err(|e| LibraryError::Backend(format!("git commit tree: {e}")))?;

        if commit.parent_count() == 0 {
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

    Ok(entries)
}

#[cfg(test)]
mod smoke {
    #[test]
    fn crate_compiles() {
        // Smoke test: ensures the crate builds with all module declarations.
    }
}
