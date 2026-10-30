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
/// `removed_rel_paths` are dropped from the new tree (and the index) in the
/// same commit, so a file moved to `rel_path` is recorded as a rename
/// rather than left tracked at its old name.
///
/// `content` is the exact bytes to commit for `rel_path`; `None` commits the
/// working-tree file as it is when this runs.
pub(super) fn stage_and_commit(
    repo: &git2::Repository,
    removed_rel_paths: &[&Path],
    rel_path: &Path,
    content: Option<&[u8]>,
    signature: &git2::Signature<'_>,
    message: &str,
) -> Result<Option<git2::Oid>, git2::Error> {
    stage_and_commit_with(
        repo,
        removed_rel_paths,
        Some(rel_path),
        content,
        &[],
        signature,
        message,
    )
}

/// [`stage_and_commit`] that also puts `extra_rel_paths` (working-tree
/// files, e.g. the pinned datasheet a row references) into the same commit.
/// `rel_path` is `None` for a commit of only `extra_rel_paths` and removals
/// (the retry of files whose earlier commit failed, #858).
pub(super) fn stage_and_commit_with(
    repo: &git2::Repository,
    removed_rel_paths: &[&Path],
    rel_path: Option<&Path>,
    content: Option<&[u8]>,
    extra_rel_paths: &[&Path],
    signature: &git2::Signature<'_>,
    message: &str,
) -> Result<Option<git2::Oid>, git2::Error> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| git2::Error::from_str("repository has no working tree"))?;
    // Hash the file once (#416): for a plain path the index entry
    // `add_path` builds already carries the blob to commit, and its stat is
    // what the index needs afterwards. That blob is the file as `git add`
    // stores it: the disk bytes after the clean filters (`core.autocrlf`,
    // `text`/`eol` attributes), as before. Built in memory only; the tree
    // is still made from HEAD's, never the index. Supplied bytes are never
    // staged from the working file, which may have moved on since.
    let main_entry = match (content, rel_path) {
        (None, Some(rel_path)) => staged_entry(repo, rel_path).ok(),
        _ => None,
    };
    let blob = match rel_path {
        None => None,
        Some(rel_path) => {
            let staged_blob = match &main_entry {
                Some(entry) if !super::lfs::is_lfs_path(repo, rel_path)? => Some(entry.id),
                _ => None,
            };
            Some(match (content, staged_blob) {
                (Some(bytes), _) => super::lfs::blob_for_bytes(repo, rel_path, bytes)?,
                (None, Some(id)) => id,
                (None, None) => super::lfs::blob_for_path(repo, workdir, rel_path)?,
            })
        }
    };
    // An unborn HEAD (fresh repo) is the only legitimate "no parent" case.
    let parent = match repo.head() {
        Ok(head) => Some(head.peel_to_commit()?),
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
        Err(e) => return Err(e),
    };
    let base = parent.as_ref().map(|c| c.tree()).transpose()?;
    let head_tree = base.clone();
    let mut base = base;
    for removed in removed_rel_paths {
        let Some(tree) = base.take() else { break };
        let components: Vec<&std::ffi::OsStr> = removed.iter().collect();
        base = match tree_without_path(repo, &tree, &components)? {
            Some(id) => Some(repo.find_tree(id)?),
            None => None,
        };
    }
    let mut staged: Vec<(&Path, git2::Oid)> = Vec::new();
    let mut tree_id = match (rel_path, blob) {
        (Some(rel_path), Some(blob)) => {
            let components: Vec<&std::ffi::OsStr> = rel_path.iter().collect();
            staged.push((rel_path, blob));
            tree_with_blob(repo, base.as_ref(), &components, blob)?
        }
        _ => match &base {
            Some(tree) => tree.id(),
            // Nothing to record on a repository with no commit.
            None if extra_rel_paths.is_empty() => return Ok(None),
            None => repo.treebuilder(None)?.write()?,
        },
    };
    for extra in extra_rel_paths {
        let extra_blob = super::lfs::blob_for_path(repo, workdir, extra)?;
        let extra_components: Vec<&std::ffi::OsStr> = extra.iter().collect();
        let with_row = repo.find_tree(tree_id)?;
        tree_id = tree_with_blob(repo, Some(&with_row), &extra_components, extra_blob)?;
        staged.push((extra, extra_blob));
    }
    if head_tree.as_ref().is_some_and(|b| b.id() == tree_id) {
        return Ok(None);
    }
    let tree = repo.find_tree(tree_id)?;
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    #[cfg(test)]
    run_before_commit_hook();
    let oid = repo.commit(Some("HEAD"), signature, signature, message, &tree, &parents)?;
    // Best effort: a failure here leaves the index stale for this one path
    // (shown as a change by `git status`), never a wrong commit.
    // The index is re-read on every attempt, so an entry another process
    // staged while we waited for its `index.lock` is kept (#900).
    let mut main_entry = main_entry;
    if let (Some(entry), Some(blob)) = (main_entry.as_mut(), blob) {
        // An LFS path's entry records the pointer that was committed.
        entry.id = blob;
    }
    let updated = retry_on_git_lock(repo.path(), || {
        let mut index = repo.index()?;
        index.read(true)?;
        for removed in removed_rel_paths {
            if index.get_path(removed, 0).is_some() {
                index.remove_path(removed)?;
            }
        }
        for (path, blob) in &staged {
            if Some(*path) == rel_path
                && let Some(entry) = &main_entry
            {
                index.add(entry)?;
                continue;
            }
            index.add_path(path)?;
            // `add_path` stores the raw bytes of the working file together
            // with its stat data; an LFS path's entry records the pointer
            // that was committed. When the caller supplied the bytes for
            // `rel_path`, the working file may have moved on since: pairing
            // the committed blob id with that file's stat would make git trust
            // the cache and call it clean, hiding the file's real content from
            // every later checkout or reset. Zeroed stat data forces git to
            // re-hash the file and report it modified. The extra files are
            // always committed from the working tree, so they match.
            let working_differs = content.is_some()
                && Some(*path) == rel_path
                && super::lfs::working_blob_id(repo, workdir, path).ok() != Some(*blob);
            if let Some(mut entry) = index.get_path(path, 0)
                && (entry.id != *blob || working_differs)
            {
                entry.id = *blob;
                if working_differs {
                    entry.ctime = git2::IndexTime::new(0, 0);
                    entry.mtime = git2::IndexTime::new(0, 0);
                    entry.file_size = 0;
                    entry.ino = 0;
                }
                index.add(&entry)?;
            }
        }
        index.write()
    });
    if let Err(e) = updated {
        tracing::warn!(
            target: "signex::library",
            path = %rel_path.map(|p| p.display().to_string()).unwrap_or_default(),
            error = %e,
            "committed, but the git index could not be updated; `git status` may show the file as changed"
        );
    }
    Ok(Some(oid))
}

#[cfg(test)]
thread_local! {
    /// Runs once, on this thread, between resolving the parent commit and
    /// making the commit: where a test moves HEAD under the commit.
    static BEFORE_COMMIT: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        const { std::cell::RefCell::new(None) };
}

/// Arrange for `hook` to run once just before the next commit this thread
/// makes (see [`BEFORE_COMMIT`]).
#[cfg(test)]
pub(super) fn set_before_commit_hook(hook: impl FnOnce() + 'static) {
    BEFORE_COMMIT.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
}

#[cfg(test)]
fn run_before_commit_hook() {
    if let Some(hook) = BEFORE_COMMIT.with(|slot| slot.borrow_mut().take()) {
        hook();
    }
}

/// The index entry `add_path` makes for `rel_path` from the working tree
/// (its blob, the clean-filtered disk bytes, hashed and stored; its stat
/// taken), without writing the index.
fn staged_entry(repo: &git2::Repository, rel_path: &Path) -> Result<git2::IndexEntry, git2::Error> {
    let mut index = repo.index()?;
    index.add_path(rel_path)?;
    let entry = index.get_path(rel_path, 0);
    // `repo.index()` is shared: drop the in-memory addition again.
    index.read(true)?;
    entry.ok_or_else(|| git2::Error::from_str("staged entry missing"))
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

/// Refuse an insert whose `row_id` or `internal_pn` is already stored in
/// any table of `lf`; the part number is the library-wide key.
pub(super) fn check_insert_unique(
    lf: &LibraryFile,
    row_id: &str,
    internal_pn: &str,
) -> Result<(), LibraryError> {
    for (name, table) in &lf.tables {
        for stored in &table.rows {
            if stored.cells.get(LEGACY_ROW_ID_COL).map(String::as_str) == Some(row_id) {
                return Err(LibraryError::Conflict(format!(
                    "row {row_id} already exists in table {name:?}"
                )));
            }
        }
    }
    check_pn_unique(lf, internal_pn, None)
}

/// [`check_pn_unique`] for an edit, run only when the PN changes from the
/// stored row's (trimmed), so a library that already holds duplicate PNs
/// still lets their other fields be edited.
pub(super) fn check_pn_change(
    lf: &LibraryFile,
    table: &str,
    row_id: &str,
    internal_pn: &str,
) -> Result<(), LibraryError> {
    let stored_pn = lf.tables.get(table).and_then(|t| {
        t.rows
            .iter()
            .find(|r| r.cells.get(LEGACY_ROW_ID_COL).map(String::as_str) == Some(row_id))
            .and_then(|r| r.cells.get("internal_pn"))
    });
    if stored_pn.is_some_and(|s| s.trim() == internal_pn.trim()) {
        return Ok(());
    }
    check_pn_unique(lf, internal_pn, Some(row_id))
}

/// Refuse a part number another row already holds. A blank PN is a draft
/// the user has not filled in yet, so it never conflicts. `except_row_id`
/// names the row being edited, which may keep its own PN. Both sides are
/// compared trimmed, so `"PN-A "` clashes with `"PN-A"`; the comparison
/// stays case-sensitive because part numbers can be case-significant.
pub(super) fn check_pn_unique(
    lf: &LibraryFile,
    internal_pn: &str,
    except_row_id: Option<&str>,
) -> Result<(), LibraryError> {
    let wanted = internal_pn.trim();
    if wanted.is_empty() {
        return Ok(());
    }
    for (name, table) in &lf.tables {
        for stored in &table.rows {
            let own = except_row_id.is_some()
                && stored.cells.get(LEGACY_ROW_ID_COL).map(String::as_str) == except_row_id;
            if !own && stored.cells.get("internal_pn").map(|s| s.trim()) == Some(wanted) {
                return Err(LibraryError::Conflict(format!(
                    "internal_pn {internal_pn:?} already exists in table {name:?}"
                )));
            }
        }
    }
    Ok(())
}

pub(super) const LEGACY_ROW_ID_COL: &str = "row_id";

/// Header expected for the v0.9 fixed-schema `[tables.<name>]` blocks
/// — same column ordering as the old per-file TSV tables so the conversion
/// through [`row_to_record`] / [`record_to_row`] stays bit-exact. Stage 12 lifts the fixed-schema constraint.
pub(super) fn legacy_columns() -> Vec<String> {
    TABLE_HEADER.iter().map(|s| (*s).to_string()).collect()
}

/// Verify a `[tables.<name>]` block's column order matches the
/// legacy schema. Mismatches are loud — a hand-edited `.snxlib`
/// where someone reordered or renamed columns would silently
/// miscolumn data otherwise.
pub(super) fn validate_legacy_header(table: &str, columns: &[String]) -> Result<(), LibraryError> {
    let names: Vec<&str> = columns.iter().map(String::as_str).collect();
    crate::tables::check_columns(&names)
        .map_err(|e| LibraryError::InvalidData(format!("table {table:?} schema mismatch: {e}")))?;
    Ok(())
}

/// Convert a component row to the cells a `[tables.<name>]` block stores,
/// validating it first. The stock-library generator calls it so the bundled
/// rows cannot drift from what the adapter writes.
pub fn component_to_library_row(row: &ComponentRow) -> Result<LibraryRow, LibraryError> {
    crate::validate::validate_row_for_write(row)?;
    let cells = row_to_record(row)?;
    let mut lib_row = LibraryRow::default();
    for (col, val) in TABLE_HEADER.iter().zip(cells) {
        lib_row.cells.insert((*col).to_string(), val);
    }
    if let Some(cell) = alternate_symbols_cell(row)? {
        lib_row
            .cells
            .insert(ALTERNATE_SYMBOLS_COLUMN.to_string(), cell);
    }
    Ok(lib_row)
}

/// Declare the optional alternate-symbols column on a table that is about
/// to store a row carrying it; a cell outside the declared columns is
/// dropped on save.
pub(super) fn declare_alternate_symbols_column(columns: &mut Vec<String>, row: &LibraryRow) {
    let has_cell = row.cells.contains_key(ALTERNATE_SYMBOLS_COLUMN);
    if has_cell && !columns.iter().any(|c| c == ALTERNATE_SYMBOLS_COLUMN) {
        columns.push(ALTERNATE_SYMBOLS_COLUMN.to_string());
    }
}

/// Base revision of a stored row: the first 8 bytes of a SHA-256 over its
/// cells. A local library has no server counter, so the row's own content
/// stands in for one; it rides on `ComponentRow::server_revision` like a
/// server revision does.
pub(super) fn row_base(row: &LibraryRow) -> u64 {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for (col, val) in &row.cells {
        hasher.update(col.as_bytes());
        hasher.update([0]);
        hasher.update(val.as_bytes());
        hasher.update([0]);
    }
    let digest = hasher.finalize();
    u64::from_be_bytes(digest[..8].try_into().expect("sha256 digest is 32 bytes"))
}

/// Refuse a write whose base is not the stored row's. `None` means the
/// caller never read the row (a row being created), so nothing to compare.
pub(super) fn check_row_base(stored: &LibraryRow, base: Option<u64>) -> Result<(), LibraryError> {
    let current_revision = row_base(stored);
    match base {
        Some(b) if b != current_revision => Err(LibraryError::RowChanged { current_revision }),
        _ => Ok(()),
    }
}

pub(super) fn library_row_to_component(row: &LibraryRow) -> Result<ComponentRow, LibraryError> {
    let mut record = csv::StringRecord::new();
    for col in TABLE_HEADER.iter() {
        let val = row.cells.get(*col).map(String::as_str).unwrap_or("");
        record.push_field(val);
    }
    if let Some(val) = row.cells.get(ALTERNATE_SYMBOLS_COLUMN) {
        record.push_field(val);
    }
    record_to_row(&record)
}

/// Attach one fetched price to its row, or say why the row no longer takes
/// it. The row must still exist and still list the same SKU for the
/// distributor the price came from.
pub(super) fn apply_pricing_update(
    lf: &mut LibraryFile,
    update: &crate::pricing::PricingUpdate,
) -> Result<(), String> {
    let entry = lf
        .tables
        .get_mut(&update.table)
        .ok_or_else(|| format!("table {} no longer exists", update.table))?;
    let row_id_text = update.row_id.to_string();
    let slot = entry
        .rows
        .iter()
        .find(|r| r.cells.get(LEGACY_ROW_ID_COL).map(String::as_str) == Some(row_id_text.as_str()))
        .ok_or_else(|| "row was deleted".to_string())?;
    let row = library_row_to_component(slot).map_err(|e| format!("row unreadable: {e}"))?;
    if !crate::pricing::record_is_current(&row, &update.distributor, &update.pricing.sku) {
        return Err(format!(
            "part {} is no longer listed for {}",
            update.pricing.sku, update.distributor
        ));
    }
    let per_sku = entry
        .pricing
        .entry(row_id_text)
        .or_default()
        .entry(update.distributor.clone())
        .or_default();
    // A retyped SKU (`abc-nd` for `ABC-ND`) names the same part: replace the
    // old record instead of keeping both, or the stale one would shadow it.
    per_sku.retain(|k, _| !crate::pricing::same_part_number(k, &update.pricing.sku));
    per_sku.insert(update.pricing.sku.clone(), update.pricing.clone());
    // A SKU-less listing keeps only its newest priced part.
    if let Some(cached) = entry.pricing.get_mut(&update.row_id.to_string()) {
        crate::pricing::prune_row_pricing(&row, cached);
    }
    Ok(())
}

/// Drop every cached price whose row is gone or whose distributor part
/// number the row no longer lists. A row that cannot be read keeps its
/// prices: they are not ours to judge. Returns whether anything changed.
pub(super) fn prune_stale_pricing(lf: &mut LibraryFile) -> bool {
    let mut changed = false;
    for table in lf.tables.values_mut() {
        if table.pricing.is_empty() {
            continue;
        }
        let rows: std::collections::HashMap<String, ComponentRow> = table
            .rows
            .iter()
            .filter_map(|r| {
                let id = r.cells.get(LEGACY_ROW_ID_COL)?.clone();
                library_row_to_component(r).ok().map(|c| (id, c))
            })
            .collect();
        let live_ids: std::collections::HashSet<&String> = table
            .rows
            .iter()
            .filter_map(|r| r.cells.get(LEGACY_ROW_ID_COL))
            .collect();
        let before = table.pricing.len();
        table.pricing.retain(|id, _| live_ids.contains(id));
        changed |= table.pricing.len() != before;
        for (id, cached) in table.pricing.iter_mut() {
            if let Some(row) = rows.get(id) {
                changed |= crate::pricing::prune_row_pricing(row, cached);
            }
        }
        let before = table.pricing.len();
        table.pricing.retain(|_, cached| !cached.is_empty());
        changed |= table.pricing.len() != before;
    }
    changed
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

/// Make `.gitattributes` carry the LFS rules, adding only the missing ones
/// to a file that exists. True when the file changed.
pub(super) fn ensure_lfs_attributes(root_dir: &Path) -> Result<bool, LibraryError> {
    let path = root_dir.join(GITATTRIBUTES_FILE);
    if !path.exists() {
        write_lfs_attributes(root_dir)?;
        return Ok(true);
    }
    let mut text = fs::read_to_string(&path)?;
    let mut changed = false;
    for ext in LFS_EXTENSIONS {
        let rule = format!("*.{ext} filter=lfs diff=lfs merge=lfs -text");
        if !text.lines().any(|line| line.trim() == rule) {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&rule);
            text.push('\n');
            changed = true;
        }
    }
    if changed {
        signex_types::atomic_io::atomic_write(&path, text.as_bytes())?;
    }
    Ok(changed)
}

/// Record the library's own commit: `parent`'s tree (empty without one) plus
/// the `.snxlib`, the managed `.gitignore` block (written here, keeping any
/// user lines, #1729) and, for LFS, `.gitattributes`, built in memory so nothing
/// else staged in the repo can be swept in. `init` passes no parent, as it
/// always has; only the LFS top-up of a library's own first commit passes one.
/// The repo's index gains just those entries, after the commit succeeded.
pub(super) fn commit_initial(
    repo: &git2::Repository,
    file_path: &Path,
    use_lfs: bool,
    parent: Option<&git2::Commit>,
) -> Result<(), LibraryError> {
    let backend =
        |what: &'static str| move |e: git2::Error| LibraryError::Backend(format!("{what}: {e}"));
    let (sig_name, sig_email) = identity_for_repo(repo)?;
    let sig = git2::Signature::now(&sig_name, &sig_email).map_err(backend("git signature"))?;
    let root_dir = parent_dir(file_path)?;
    super::ignore_rules::write_ignore_rules(&root_dir)?;
    let mut names = vec![
        file_name_str(file_path)?,
        super::ignore_rules::GITIGNORE_FILE.to_string(),
    ];
    if use_lfs {
        names.push(GITATTRIBUTES_FILE.to_string());
    }
    let base = parent
        .map(git2::Commit::tree)
        .transpose()
        .map_err(backend("git parent tree"))?;
    let mut builder = repo
        .treebuilder(base.as_ref())
        .map_err(backend("git tree builder"))?;
    for name in &names {
        let blob = repo
            .blob(&fs::read(root_dir.join(name))?)
            .map_err(backend("git blob"))?;
        builder
            .insert(name, blob, 0o100644)
            .map_err(backend("git tree insert"))?;
    }
    let tree_oid = builder.write().map_err(backend("git write tree"))?;
    let tree = repo.find_tree(tree_oid).map_err(backend("git find tree"))?;
    let parents: Vec<&git2::Commit> = parent.into_iter().collect();
    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        "chore: initialise library",
        &tree,
        &parents,
    )
    .map_err(backend("git initial commit"))?;
    let mut index = repo.index().map_err(backend("git index"))?;
    for name in &names {
        index
            .add_path(Path::new(name))
            .map_err(backend("git add"))?;
    }
    index.write().map_err(backend("git index write"))?;
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
            layout: Default::default(),
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

    /// Alternate footprints survive the `.snxlib` row path (#1234), and a
    /// malformed cell is an error, not a dropped alternate.
    #[test]
    fn library_row_roundtrips_alternate_footprints() {
        use crate::footprint_binding::FootprintBinding;
        use crate::primitive::PrimitiveRef;
        let mut row = crate::component::tests::fixture_row();
        row.alternate_footprints = vec![FootprintBinding::new(PrimitiveRef::new(
            Uuid::now_v7(),
            Uuid::now_v7(),
        ))];
        let lib_row = component_to_library_row(&row).unwrap();
        assert!(lib_row.cells["footprint_ref"].starts_with('['));
        assert_eq!(library_row_to_component(&lib_row).unwrap(), row);

        let mut bad = lib_row;
        bad.cells.insert("footprint_ref".into(), "[]".into());
        assert!(library_row_to_component(&bad).is_err());
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
