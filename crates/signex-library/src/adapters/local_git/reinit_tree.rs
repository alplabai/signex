//! What a library Re-init commits (#1741): HEAD's tree plus the library's own
//! files, built in memory. The repo's index is never read for the commit, so
//! work a user staged there cannot ride along.

use std::path::Path;

use super::*;

/// Folders only the library writes into. Their content in a Re-init commit
/// is exactly what is on disk, so a deleted primitive leaves the tree.
fn owned_dirs() -> impl Iterator<Item = &'static str> {
    LIBRARY_SUBFOLDERS
        .into_iter()
        .chain([crate::datasheet_asset::DATASHEET_ASSET_DIR])
}

pub(super) struct Snapshot {
    pub(super) tree: git2::Oid,
    /// Every library file in the tree, with the blob committed for it.
    files: Vec<(String, git2::Oid)>,
    /// Tracked library paths that are gone from disk and left the tree.
    removed: Vec<String>,
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
    let name = file_name_str(file_path)?;
    let tracks = |commit: &git2::Commit| {
        commit
            .tree()
            .is_ok_and(|tree| tree.get_name(&name).is_some())
    };
    let foreign = match &head {
        Some(head) => !tracks(head),
        // An unborn HEAD (a deleted `HEAD` file is re-made at the default
        // branch) can hide a repo whose real branch has history.
        None => {
            let tips: Vec<git2::Commit> = repo
                .branches(Some(git2::BranchType::Local))
                .map(|branches| {
                    branches
                        .flatten()
                        .filter_map(|(branch, _)| branch.get().peel_to_commit().ok())
                        .collect()
                })
                .unwrap_or_default();
            !tips.is_empty() && !tips.iter().any(&tracks)
        }
    };
    if foreign {
        return Err(LibraryError::Conflict(format!(
            "{} is a git repository that does not track this library",
            root_dir.display()
        )));
    }
    Ok(head)
}

/// Whether `rel` under `root_dir` is a file or a link, with no symbolic link
/// on the way to it. A link as the last part is committed as a link and never
/// read; a link in a folder above it is not followed out of the library.
fn is_committable(root_dir: &Path, rel: &str) -> bool {
    let mut path = root_dir.to_path_buf();
    let parts: Vec<&str> = rel.split('/').collect();
    for (i, part) in parts.iter().enumerate() {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(meta) if i + 1 == parts.len() => {
                return meta.is_file() || meta.file_type().is_symlink();
            }
            Ok(meta) if meta.file_type().is_symlink() => {
                tracing::warn!(
                    target: "signex::library",
                    "Re-init left out {}: a folder above it is a symbolic link",
                    root_dir.join(rel).display()
                );
                return false;
            }
            Ok(_) => {}
            Err(_) => return false,
        }
    }
    false
}

/// A link's blob is its target path, as git stores it; the target is never
/// opened.
fn link_blob(repo: &git2::Repository, path: &Path) -> Result<git2::Oid, git2::Error> {
    let target = fs::read_link(path).map_err(|e| git2::Error::from_str(&e.to_string()))?;
    #[cfg(unix)]
    let bytes = std::os::unix::ffi::OsStrExt::as_bytes(target.as_os_str()).to_vec();
    #[cfg(not(unix))]
    let bytes = target.to_string_lossy().replace('\\', "/").into_bytes();
    repo.blob(&bytes)
}

/// Whether the regular file at `path` is executable on disk (unix only).
fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

const LINK_MODE: u32 = 0o120000;
const FILE_MODE: u32 = 0o100644;
const EXEC_MODE: u32 = 0o100755;

/// The local branch a Re-init without a HEAD continues, and its tip.
/// Among the branches whose tip tracks this `.snxlib`: `main`, then
/// `master`, then the branch the HEAD reflog last checked out, then the
/// one with the newest tip commit (name order breaks a tie).
pub(super) fn tracking_branch<'r>(
    repo: &'r git2::Repository,
    file_path: &Path,
) -> Result<Option<(String, git2::Commit<'r>)>, LibraryError> {
    let name = file_name_str(file_path)?;
    let mut found: Vec<(String, git2::Commit)> = Vec::new();
    let branches = repo
        .branches(Some(git2::BranchType::Local))
        .map_err(|e| LibraryError::Backend(format!("git branches: {e}")))?;
    for (branch, _) in branches.flatten() {
        let (Ok(Some(short)), Ok(tip)) = (branch.name(), branch.get().peel_to_commit()) else {
            continue;
        };
        if tip.tree().is_ok_and(|t| t.get_name(&name).is_some()) {
            found.push((short.to_owned(), tip));
        }
    }
    let last_checkout = repo.reflog("HEAD").ok().and_then(|log| {
        log.iter().find_map(|e| {
            let msg = e.message().ok()??;
            let rest = msg.strip_prefix("checkout: moving from ")?;
            rest.rsplit_once(" to ").map(|(_, to)| to.to_owned())
        })
    });
    let rank = |short: &str| match short {
        "main" => 0,
        "master" => 1,
        s if last_checkout.as_deref() == Some(s) => 2,
        _ => 3,
    };
    found.sort_by(|(a, ca), (b, cb)| {
        rank(a)
            .cmp(&rank(b))
            .then(cb.time().seconds().cmp(&ca.time().seconds()))
            .then(a.cmp(b))
    });
    Ok(found
        .into_iter()
        .next()
        .map(|(short, tip)| (format!("refs/heads/{short}"), tip)))
}

/// Library files and links under `root_dir`, as `/`-separated paths: the `.snxlib`,
/// the managed `.gitignore` (written here, keeping user lines), a
/// `.gitattributes` if there is one, and every regular file in the owned
/// folders except write-lock sidecars and what the repo's ignore rules name.
/// The managed `.gitignore` block is written best-effort: a file that cannot
/// take it (read-only, a link, broken markers) is warned about and left as
/// it is, as a normal commit does, and Re-init goes on.
fn library_files(
    repo: &git2::Repository,
    file_path: &Path,
    root_dir: &Path,
) -> Result<Vec<String>, LibraryError> {
    if let Err(e) = super::ignore_rules::write_ignore_rules(root_dir) {
        tracing::warn!(
            target: "signex::library",
            "Re-init could not add Signex's ignore rules to {}: {e}",
            root_dir.join(super::ignore_rules::GITIGNORE_FILE).display()
        );
    }
    if let Err(e) = super::ignore_rules::apply_managed_rules(repo) {
        return Err(LibraryError::Backend(format!("git ignore rules: {e}")));
    }
    let snxlib = file_name_str(file_path)?;
    if !is_committable(root_dir, &snxlib) {
        return Err(LibraryError::Backend(format!(
            "{} is not a file or a link to commit",
            file_path.display()
        )));
    }
    let mut out = vec![snxlib];
    for name in [super::ignore_rules::GITIGNORE_FILE, GITATTRIBUTES_FILE] {
        if is_committable(root_dir, name) {
            out.push(name.to_string());
        }
    }
    for dir in owned_dirs() {
        collect_files(repo, root_dir, dir, &mut out)?;
    }
    Ok(out)
}

fn collect_files(
    repo: &git2::Repository,
    root_dir: &Path,
    rel: &str,
    out: &mut Vec<String>,
) -> Result<(), LibraryError> {
    // The folder itself (and `assets/` above `assets/datasheets`) must not be
    // a link either; `is_committable` is for files, so walk the parts here.
    let mut dir = root_dir.to_path_buf();
    for part in rel.split('/') {
        dir.push(part);
        match fs::symlink_metadata(&dir) {
            Ok(meta) if meta.is_dir() => {}
            _ => return Ok(()),
        }
    }
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!(
                target: "signex::library",
                "Re-init could not read {}, so it is left out of the commit: {e}",
                dir.display()
            );
            return Ok(());
        }
    };
    for entry in entries {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            tracing::warn!(
                target: "signex::library",
                "Re-init left out {}: its name is not UTF-8",
                entry.path().display()
            );
            continue;
        };
        let child = format!("{rel}/{name}");
        let kind = entry.file_type()?;
        if kind.is_dir()
            && (name == ".git" || fs::symlink_metadata(entry.path().join(".git")).is_ok())
        {
            tracing::warn!(
                target: "signex::library",
                "Re-init left out {}: it is or holds a git repository",
                entry.path().display()
            );
        } else if kind.is_dir() {
            collect_files(repo, root_dir, &child, out)?;
        } else if (kind.is_file() || kind.is_symlink())
            && !is_write_lock_file(Path::new(&child))
            && !super::ignore_rules::is_managed_clutter(&child)
            && !repo
                .status_should_ignore(Path::new(&child))
                .unwrap_or(false)
        {
            out.push(child);
        }
    }
    Ok(())
}

fn entry(path: &str, id: git2::Oid, mode: u32) -> git2::IndexEntry {
    git2::IndexEntry {
        ctime: git2::IndexTime::new(0, 0),
        mtime: git2::IndexTime::new(0, 0),
        dev: 0,
        ino: 0,
        mode,
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
    let files = library_files(repo, file_path, root_dir)?;
    // In memory only: not the repo's index, so nothing staged there is seen.
    let mut index = git2::Index::new().map_err(backend("git index"))?;
    let mut removed = Vec::new();
    if let Some(parent) = parent {
        let base = parent.tree().map_err(backend("git parent tree"))?;
        index.read_tree(&base).map_err(backend("git read tree"))?;
        // A tracked file leaves the tree only when it is gone from disk. One
        // that is still there but left out (a link, an ignored file, a name
        // that is not UTF-8) keeps what HEAD has.
        let tracked: Vec<String> = index
            .iter()
            .filter_map(|e| String::from_utf8(e.path).ok())
            .filter(|p| owned_dirs().any(|d| p.starts_with(&format!("{d}/"))))
            .collect();
        for path in tracked {
            if fs::symlink_metadata(root_dir.join(&path))
                .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
            {
                index
                    .remove_path(Path::new(&path))
                    .map_err(backend("git tree remove"))?;
                removed.push(path);
            }
        }
    }
    let mut blobs = Vec::with_capacity(files.len());
    for name in files {
        let on_disk = root_dir.join(&name);
        let is_link = fs::symlink_metadata(&on_disk).is_ok_and(|m| m.file_type().is_symlink());
        let (id, mode) = if is_link {
            (
                link_blob(repo, &on_disk).map_err(backend("git blob"))?,
                LINK_MODE,
            )
        } else {
            let id = lfs::blob_for_path(repo, root_dir, Path::new(&name))
                .map_err(backend("git blob"))?;
            // A file keeps the mode HEAD has for it; a new one (or one that
            // was a link) is executable when it is on disk.
            let mode = match index.get_path(Path::new(&name), 0) {
                Some(kept) if kept.mode == EXEC_MODE || kept.mode == FILE_MODE => kept.mode,
                _ if is_executable(&on_disk) => EXEC_MODE,
                _ => FILE_MODE,
            };
            (id, mode)
        };
        index
            .add(&entry(&name, id, mode))
            .map_err(backend("git tree insert"))?;
        blobs.push((name, id));
    }
    let tree = index
        .write_tree_to(repo)
        .map_err(backend("git write tree"))?;
    Ok(Snapshot {
        tree,
        files: blobs,
        removed,
    })
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
    for path in &snapshot.removed {
        index
            .remove_path(Path::new(path))
            .map_err(backend("git index remove"))?;
    }
    let mut lfs_paths = false;
    for (name, id) in &snapshot.files {
        index
            .add_path(Path::new(name))
            .map_err(backend("git add"))?;
        // An LFS path is recorded as its pointer, as the commit holds it.
        if lfs::is_lfs_path(repo, Path::new(name)).unwrap_or(false)
            && let Some(mut staged) = index.get_path(Path::new(name), 0)
        {
            staged.id = *id;
            index.add(&staged).map_err(backend("git add"))?;
            lfs_paths = true;
        }
    }
    if lfs_paths {
        lfs::install_pre_push_hook(repo).map_err(backend("git lfs hook"))?;
    }
    index.write().map_err(backend("git index write"))?;
    Ok(())
}
