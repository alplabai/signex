//! Free helper functions for the local-git adapter.

use super::*;

// ── Free helpers ───────────────────────────────────────────────────────────

/// How long a commit waits for another process's `.git/index.lock` (or a
/// ref lock) to go away before giving up (#893).
const GIT_LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// A lock file older than this (`index.lock`, or a branch ref's `.lock`)
/// is taken for one a crashed git left behind: the commit is refused at
/// once with a hint, instead of waiting out [`GIT_LOCK_WAIT`] on every save.
const GIT_LOCK_STALE: std::time::Duration = std::time::Duration::from_secs(10);

/// Commit the working-tree file at `rel_path` with `message`, on top of
/// the current HEAD. Returns `None` when the file is unchanged from HEAD's
/// (an unchanged save, #149).
///
/// The tree is built from the parent commit's tree plus this file's blob,
/// never from the shared on-disk index: another process can rewrite that
/// index between our read and our write, and a tree built from it would
/// silently revert their file (#900 review). The commit names its parent
/// explicitly, so if HEAD moved meanwhile libgit2 refuses it with
/// `ErrorCode::Modified` and [`retry_on_git_lock`] rebuilds on the new tip.
/// The on-disk index entry is updated afterwards, best effort, so the
/// working copy does not show the file as changed.
///
/// `removed_rel_path` is dropped from the new tree (and the index) in the
/// same commit, so a file moved to `rel_path` is recorded as a rename
/// rather than left tracked at its old name.
pub(super) fn stage_and_commit(
    repo: &git2::Repository,
    removed_rel_path: Option<&Path>,
    rel_path: &Path,
    signature: &git2::Signature<'_>,
    message: &str,
) -> Result<Option<git2::Oid>, git2::Error> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| git2::Error::from_str("repository has no working tree"))?;
    let blob = repo.blob_path(&workdir.join(rel_path))?;
    // An unborn HEAD (fresh repo) is the only legitimate "no parent" case.
    let parent = match repo.head() {
        Ok(head) => Some(head.peel_to_commit()?),
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
        Err(e) => return Err(e),
    };
    let base = parent.as_ref().map(|c| c.tree()).transpose()?;
    let head_tree = base.clone();
    let base = match (removed_rel_path, base) {
        (Some(removed), Some(tree)) => {
            let components: Vec<&std::ffi::OsStr> = removed.iter().collect();
            match tree_without_path(repo, &tree, &components)? {
                Some(id) => Some(repo.find_tree(id)?),
                None => None,
            }
        }
        (_, base) => base,
    };
    let components: Vec<&std::ffi::OsStr> = rel_path.iter().collect();
    let tree_id = tree_with_blob(repo, base.as_ref(), &components, blob)?;
    if head_tree.as_ref().is_some_and(|b| b.id() == tree_id) {
        return Ok(None);
    }
    let tree = repo.find_tree(tree_id)?;
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    let oid = repo.commit(Some("HEAD"), signature, signature, message, &tree, &parents)?;
    // Best effort: a failure here leaves the index stale for this one path
    // (shown as a change by `git status`), never a wrong commit.
    if let Err(e) = retry_on_git_lock(repo.path(), || {
        let mut index = repo.index()?;
        index.read(true)?;
        if let Some(removed) = removed_rel_path
            && index.get_path(removed, 0).is_some()
        {
            index.remove_path(removed)?;
        }
        index.add_path(rel_path)?;
        index.write()
    }) {
        tracing::warn!(
            target: "signex::library",
            path = %rel_path.display(),
            error = %e,
            "committed, but the git index could not be updated; `git status` may show the file as changed"
        );
    }
    Ok(Some(oid))
}

/// `base` without the entry at `path`, written as new trees up to the
/// root. `None` when removing it leaves the tree empty; `base` unchanged
/// when the entry does not exist.
fn tree_without_path(
    repo: &git2::Repository,
    base: &git2::Tree<'_>,
    path: &[&std::ffi::OsStr],
) -> Result<Option<git2::Oid>, git2::Error> {
    let name = path[0].to_string_lossy();
    let Some(entry) = base.get_name(&name) else {
        return Ok(Some(base.id()));
    };
    let mut builder = repo.treebuilder(Some(base))?;
    if path.len() == 1 {
        builder.remove(&*name)?;
    } else {
        let Ok(sub) = repo.find_tree(entry.id()) else {
            return Ok(Some(base.id()));
        };
        match tree_without_path(repo, &sub, &path[1..])? {
            Some(id) => {
                builder.insert(&*name, id, 0o040000)?;
            }
            None => {
                builder.remove(&*name)?;
            }
        }
    }
    if builder.is_empty() {
        return Ok(None);
    }
    builder.write().map(Some)
}

/// `base` with the file at `path` set to `blob`, written as new trees up to
/// the root. Keeps an existing entry's file mode.
fn tree_with_blob(
    repo: &git2::Repository,
    base: Option<&git2::Tree<'_>>,
    path: &[&std::ffi::OsStr],
    blob: git2::Oid,
) -> Result<git2::Oid, git2::Error> {
    let mut builder = repo.treebuilder(base)?;
    let name = path[0];
    let existing = base
        .and_then(|t| t.get_name(&name.to_string_lossy()))
        .map(|e| (e.id(), e.filemode()));
    if path.len() == 1 {
        let mode = existing.map_or(0o100644, |(_, mode)| mode);
        builder.insert(name, blob, mode)?;
    } else {
        let sub = existing.and_then(|(id, _)| repo.find_tree(id).ok());
        let sub_id = tree_with_blob(repo, sub.as_ref(), &path[1..], blob)?;
        builder.insert(name, sub_id, 0o040000)?;
    }
    builder.write()
}

/// Run a git write, retrying while another process holds the index or a
/// ref lock, or moved HEAD under us (`Modified`). Another Signex instance
/// committing to the same repository (another library in a project-scope
/// repo, or the project itself) holds those locks for the length of one
/// commit; without the retry the second save fails although nothing
/// conflicts. `op` must redo its whole write from the current state.
pub(super) fn retry_on_git_lock<T>(
    git_dir: &Path,
    mut op: impl FnMut() -> Result<T, git2::Error>,
) -> Result<T, git2::Error> {
    let deadline = std::time::Instant::now() + GIT_LOCK_WAIT;
    loop {
        match op() {
            Err(e)
                if matches!(
                    e.code(),
                    git2::ErrorCode::Locked | git2::ErrorCode::Modified
                ) =>
            {
                if let Some(stale) = stale_lock(git_dir) {
                    return Err(git2::Error::from_str(&format!(
                        "{} is older than {} s; if no git process is running, remove it",
                        stale.display(),
                        GIT_LOCK_STALE.as_secs()
                    )));
                }
                if std::time::Instant::now() >= deadline {
                    return Err(e);
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            other => return other,
        }
    }
}

/// A lock file a crashed git left behind: `index.lock` or a branch ref's
/// `.lock`, older than [`GIT_LOCK_STALE`].
fn stale_lock(git_dir: &Path) -> Option<std::path::PathBuf> {
    let is_stale = |path: &Path| {
        std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > GIT_LOCK_STALE)
    };
    let index_lock = git_dir.join("index.lock");
    if is_stale(&index_lock) {
        return Some(index_lock);
    }
    std::fs::read_dir(git_dir.join("refs").join("heads"))
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|e| e == "lock") && is_stale(path))
}

/// Project a `git2::Commit` onto the trait-level [`HistoryEntry`].
///
/// Diff-stat fields (`additions`, `deletions`, `files_changed`) stay
/// at the scaffold defaults — Stage 17 ships the list shape without
/// the lazy diff plumbing. The author timestamp is preferred over
/// the committer's so rebases/cherry-picks don't visually skew the
/// "12 minutes ago" labels.
pub(super) fn commit_to_history_entry(commit: &git2::Commit<'_>) -> HistoryEntry {
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
pub(super) fn commit_touches_path(
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

pub(super) const LEGACY_ROW_ID_COL: &str = "row_id";

/// Header expected for the v0.9 fixed-schema `[tables.<name>]` blocks
/// — same column ordering as the pre-refactor `tables/*.tsv` files
/// so the conversion through [`row_to_record`] / [`record_to_row`]
/// stays bit-exact. Stage 12 lifts the fixed-schema constraint.
pub(super) fn legacy_columns() -> Vec<String> {
    TABLE_HEADER.iter().map(|s| (*s).to_string()).collect()
}

/// Verify a `[tables.<name>]` block's column order matches the
/// legacy schema. Mismatches are loud — a hand-edited `.snxlib`
/// where someone reordered or renamed columns would silently
/// miscolumn data otherwise.
pub(super) fn validate_legacy_header(table: &str, columns: &[String]) -> Result<(), LibraryError> {
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

pub(super) fn component_to_library_row(row: &ComponentRow) -> Result<LibraryRow, LibraryError> {
    let cells = row_to_record(row)?;
    let mut lib_row = LibraryRow::default();
    for (col, val) in TABLE_HEADER.iter().zip(cells) {
        lib_row.cells.insert((*col).to_string(), val);
    }
    Ok(lib_row)
}

pub(super) fn library_row_to_component(row: &LibraryRow) -> Result<ComponentRow, LibraryError> {
    let mut record = csv::StringRecord::new();
    for col in TABLE_HEADER.iter() {
        let val = row.cells.get(*col).map(String::as_str).unwrap_or("");
        record.push_field(val);
    }
    record_to_row(&record)
}

pub(super) fn synthesize_manifest(snx: &SnxlibManifest) -> Manifest {
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

pub(super) fn parent_dir(p: &Path) -> Result<PathBuf, LibraryError> {
    p.parent().map(Path::to_path_buf).ok_or_else(|| {
        LibraryError::Backend(format!(
            "library file {} has no parent directory",
            p.display()
        ))
    })
}

pub(super) fn file_name_str(p: &Path) -> Result<String, LibraryError> {
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

pub(super) fn write_lfs_attributes(root_dir: &Path) -> Result<(), LibraryError> {
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
pub(super) fn slugify(name: &str) -> String {
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

pub(super) fn primitive_subdir(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Symbol => SYMBOLS_DIR,
        PrimitiveKind::Footprint => FOOTPRINTS_DIR,
        PrimitiveKind::Sim => SIMS_DIR,
    }
}

pub(super) fn primitive_ext(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Symbol => SYMBOL_EXT,
        PrimitiveKind::Footprint => FOOTPRINT_EXT,
        PrimitiveKind::Sim => SIM_EXT,
    }
}

pub(super) fn primitive_kind_str(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Symbol => "symbol",
        PrimitiveKind::Footprint => "footprint",
        PrimitiveKind::Sim => "sim",
    }
}

/// Snapshot the component-class registry out of `lock`.
///
/// A poisoned lock used to return `Vec::new()`, which is byte-identical
/// to a fresh library's empty registry — so the browser sidebar
/// rendered no class rows and a lookup by key found nothing, with no
/// signal anywhere that the registry had simply not been read. Poison
/// is now reported and the registry read through, because the data
/// behind the flag is still there: a panicking writer is what the
/// poison flag exists to announce, not to censor.
///
/// The sibling `list_tables` maps poison onto
/// [`LibraryError::Backend`] instead; it can, because its signature
/// returns a `Result`. `library_classes()` returns a bare `Vec`, so
/// the log record is the only place the failure can surface.
pub(super) fn classes_or_report(
    lock: &RwLock<LibraryFile>,
    file_path: &Path,
) -> Vec<crate::library_file::ClassEntry> {
    match lock.read() {
        Ok(guard) => guard.manifest.classes.clone(),
        Err(poisoned) => {
            let recovered = poisoned.into_inner();
            tracing::error!(
                target: "signex::library",
                path = %file_path.display(),
                classes = recovered.manifest.classes.len(),
                "component-class registry lock is poisoned — a writer panicked mid-update; the registry was read through, but this library's in-memory state should be considered suspect until it is reopened"
            );
            recovered.manifest.classes.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library_file::ClassEntry;

    fn library_file_with_classes(keys: &[&str]) -> LibraryFile {
        let mut lf = LibraryFile {
            manifest: crate::library_file::SnxlibManifest {
                format: crate::library_file::FORMAT_TOKEN.into(),
                library_id: Uuid::now_v7(),
                library: crate::library_file::LibrarySection {
                    name: "poison-probe".into(),
                    description: None,
                },
                mode: crate::manifest::LibraryMode::default(),
                workflow: crate::manifest::WorkflowConfig::default(),
                users: crate::manifest::UsersConfig::default(),
                classes: Vec::new(),
            },
            tables: std::collections::BTreeMap::new(),
        };
        lf.manifest.classes = keys
            .iter()
            .map(|k| ClassEntry {
                key: (*k).to_string(),
                label: (*k).to_uppercase(),
            })
            .collect();
        lf
    }

    fn poison(lock: &RwLock<LibraryFile>) {
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = lock.write().expect("lock is clean before the probe");
            panic!("probe: writer panics holding the library lock");
        }));
        assert!(caught.is_err(), "the probe must actually panic");
        assert!(lock.read().is_err(), "the lock must now be poisoned");
    }

    /// A poisoned lock used to read as an empty registry, which is what
    /// a brand-new library looks like — so nothing anywhere in the UI
    /// could tell the two apart.
    #[test]
    fn a_poisoned_lock_does_not_read_as_an_empty_class_registry() {
        let lock = RwLock::new(library_file_with_classes(&["resistor", "capacitor"]));
        poison(&lock);

        let classes = classes_or_report(&lock, Path::new("/tmp/probe/probe.snxlib"));
        let keys: Vec<&str> = classes.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(
            keys,
            ["resistor", "capacitor"],
            "a poisoned lock was reported as a library with no classes"
        );
    }

    #[test]
    fn a_clean_lock_reads_the_registry_unchanged() {
        let lock = RwLock::new(library_file_with_classes(&["ic"]));
        let classes = classes_or_report(&lock, Path::new("/tmp/probe/probe.snxlib"));
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].key, "ic");
        assert_eq!(classes[0].label, "IC");
    }
}
