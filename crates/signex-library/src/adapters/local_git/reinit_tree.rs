//! What a library Re-init commits (#1741): HEAD's tree plus the library's own
//! files, built in memory. The repo's index is never read for the commit, so
//! work a user staged there cannot ride along.

use std::path::Path;

use super::*;

/// Folders only the library writes into. Their content in a Re-init commit
/// is exactly what is on disk, so a deleted primitive leaves the tree.
const OWNED_DIRS: [&str; 4] = [
    SYMBOLS_DIR,
    FOOTPRINTS_DIR,
    SIMS_DIR,
    crate::datasheet_asset::DATASHEET_ASSET_DIR,
];

pub(super) struct Snapshot {
    pub(super) tree: git2::Oid,
    /// Every library file in the tree, with the blob committed for it.
    files: Vec<(String, git2::Oid)>,
}

/// The repo's HEAD commit, or `None` when it has no commit yet. A repo
/// with history that does not track this `.snxlib` is refused: a library
/// commit must never land in, or sweep up, another repo's work.
pub(super) fn refuse_foreign_history<'r>(
    repo: &'r git2::Repository,
    file_path: &Path,
    root_dir: &Path,
) -> Result<Option<git2::Commit<'r>>, LibraryError> {
    let head = repo.head().ok().and_then(|head| head.peel_to_commit().ok());
    if let Some(head) = &head {
        let name = file_name_str(file_path)?;
        let tracked = head.tree().is_ok_and(|tree| tree.get_name(&name).is_some());
        if !tracked {
            return Err(LibraryError::Conflict(format!(
                "{} is a git repository that does not track this library",
                root_dir.display()
            )));
        }
    }
    Ok(head)
}

/// Library files under `root_dir`, as `/`-separated paths: the `.snxlib`,
/// the managed `.gitignore` (written here, keeping user lines), a
/// `.gitattributes` if there is one, and every regular file in the owned
/// folders except write-lock sidecars and managed clutter.
fn library_files(file_path: &Path, root_dir: &Path) -> Result<Vec<String>, LibraryError> {
    super::ignore_rules::write_ignore_rules(root_dir)?;
    let mut out = vec![
        file_name_str(file_path)?,
        super::ignore_rules::GITIGNORE_FILE.to_string(),
    ];
    if root_dir.join(GITATTRIBUTES_FILE).is_file() {
        out.push(GITATTRIBUTES_FILE.to_string());
    }
    for dir in OWNED_DIRS {
        collect_files(root_dir, dir, &mut out)?;
    }
    Ok(out)
}

fn collect_files(root_dir: &Path, rel: &str, out: &mut Vec<String>) -> Result<(), LibraryError> {
    let Ok(entries) = fs::read_dir(root_dir.join(rel)) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let child = format!("{rel}/{name}");
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect_files(root_dir, &child, out)?;
        } else if kind.is_file()
            && !is_write_lock_file(Path::new(&child))
            && !super::ignore_rules::is_managed_clutter(&child)
        {
            out.push(child);
        }
    }
    Ok(())
}

fn entry(path: &str, id: git2::Oid) -> git2::IndexEntry {
    git2::IndexEntry {
        ctime: git2::IndexTime::new(0, 0),
        mtime: git2::IndexTime::new(0, 0),
        dev: 0,
        ino: 0,
        mode: 0o100644,
        uid: 0,
        gid: 0,
        file_size: 0,
        id,
        flags: path.len().min(0xFFF) as u16,
        flags_extended: 0,
        path: path.as_bytes().to_vec(),
    }
}

/// The tree of `parent` (empty without one) with the owned folders replaced
/// by what is on disk and the library's top-level files added or updated.
pub(super) fn library_tree(
    repo: &git2::Repository,
    file_path: &Path,
    root_dir: &Path,
    parent: Option<&git2::Commit>,
) -> Result<Snapshot, LibraryError> {
    let backend =
        |what: &'static str| move |e: git2::Error| LibraryError::Backend(format!("{what}: {e}"));
    let files = library_files(file_path, root_dir)?;
    // In memory only: not the repo's index, so nothing staged there is seen.
    let mut index = git2::Index::new().map_err(backend("git index"))?;
    if let Some(parent) = parent {
        let base = parent.tree().map_err(backend("git parent tree"))?;
        index.read_tree(&base).map_err(backend("git read tree"))?;
    }
    for dir in OWNED_DIRS {
        // Fails when the folder holds nothing in the tree: nothing to drop.
        let _ = index.remove_dir(Path::new(dir), 0);
    }
    let mut blobs = Vec::with_capacity(files.len());
    for name in files {
        let id =
            lfs::blob_for_path(repo, root_dir, Path::new(&name)).map_err(backend("git blob"))?;
        index
            .add(&entry(&name, id))
            .map_err(backend("git tree insert"))?;
        blobs.push((name, id));
    }
    let tree = index
        .write_tree_to(repo)
        .map_err(backend("git write tree"))?;
    Ok(Snapshot { tree, files: blobs })
}

/// After the commit, make the repo's own index agree with it for the
/// library's paths only: the files it now holds, and no entry of a deleted
/// primitive. Entries of anything else the user staged stay as they were.
pub(super) fn record_in_index(
    repo: &git2::Repository,
    snapshot: &Snapshot,
) -> Result<(), LibraryError> {
    let backend =
        |what: &'static str| move |e: git2::Error| LibraryError::Backend(format!("{what}: {e}"));
    let mut index = repo.index().map_err(backend("git index"))?;
    let kept: std::collections::HashSet<&str> = snapshot
        .files
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    let stale: Vec<String> = index
        .iter()
        .filter_map(|e| String::from_utf8(e.path).ok())
        .filter(|p| OWNED_DIRS.iter().any(|d| p.starts_with(&format!("{d}/"))))
        .filter(|p| !kept.contains(p.as_str()))
        .collect();
    for path in stale {
        index
            .remove_path(Path::new(&path))
            .map_err(backend("git index remove"))?;
    }
    let mut lfs_paths = false;
    for (name, id) in &snapshot.files {
        index
            .add_path(Path::new(name))
            .map_err(backend("git add"))?;
        // An LFS path is recorded as its pointer, as the commit holds it.
        if let Some(mut staged) = index.get_path(Path::new(name), 0) {
            if staged.id != *id {
                staged.id = *id;
                index.add(&staged).map_err(backend("git add"))?;
                lfs_paths = true;
            }
        }
    }
    if lfs_paths {
        lfs::install_pre_push_hook(repo).map_err(backend("git lfs hook"))?;
    }
    index.write().map_err(backend("git index write"))?;
    Ok(())
}
