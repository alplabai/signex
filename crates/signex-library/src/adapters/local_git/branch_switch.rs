//! The one branch switch (mockup rows 35 and 39). It is all or nothing: a
//! refusal writes no file, and a failure after the first write puts the
//! working tree, the index and HEAD back. Rollback tries every file; what it
//! could not restore comes back as [`SwitchRefusal::NotRestored`].
//!
//! libgit2 runs no LFS filter, so a file whose blob on either side is an LFS
//! pointer is not left to its checkout. Its content comes from
//! `.git/lfs/objects`, checked before anything is written.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::commit_lock::lock_repo_for_commit;
use super::lfs::{self, LfsPointer};
use super::project_repo::SwitchRefusal;
use super::tree_path::is_safe_to_write;
use crate::repo_branches::{BranchTarget, local_name, target_commit};

pub(crate) fn failed(what: &str, e: git2::Error) -> SwitchRefusal {
    SwitchRefusal::Failed(format!("{what}: {e}"))
}

fn io_failed(what: &str, e: std::io::Error) -> SwitchRefusal {
    SwitchRefusal::Failed(format!("{what}: {e}"))
}

fn shown(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// What an LFS-handled path holds after the switch.
enum Incoming {
    Absent,
    /// The verified content in `.git/lfs/objects`.
    Object(PathBuf),
    /// A plain blob replacing a pointer.
    Blob(git2::Oid),
}

struct LfsChange {
    rel: PathBuf,
    /// The path as the tree spells it, for the index entry.
    repo_path: Vec<u8>,
    incoming: Incoming,
    /// The target tree's blob, which the index records.
    blob: git2::Oid,
    mode: u32,
}

#[derive(Default)]
struct Plan {
    plain: Vec<PathBuf>,
    lfs: Vec<LfsChange>,
}

/// Whether git sees the working file at `rel` as what `tree` records. The diff
/// hashes it through the repository's clean filters, so a clean file checked
/// out with CRLF under `core.autocrlf` matches its LF blob. Only content is at
/// stake here, so a mode change alone is not a difference. Only one delta that
/// git calls unmodified counts: a path the diff missed is not a clean file.
fn working_file_matches_tree(
    repo: &git2::Repository,
    tree: &git2::Tree<'_>,
    rel: &Path,
) -> Result<bool, git2::Error> {
    let mut options = git2::DiffOptions::new();
    options
        .pathspec(repo_path_of(rel))
        .disable_pathspec_match(true)
        .include_unmodified(true)
        .ignore_filemode(true);
    let diff = repo.diff_tree_to_workdir(Some(tree), Some(&mut options))?;
    let mut deltas = diff.deltas();
    let first = deltas.next().map(|delta| delta.status());
    Ok(first == Some(git2::Delta::Unmodified) && deltas.next().is_none())
}

/// `rel` spelled as the repository does. On Unix a backslash is part of a
/// file name and a name need not be UTF-8, so the bytes are kept.
#[cfg(unix)]
fn repo_path_of(rel: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    rel.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
fn repo_path_of(rel: &Path) -> Vec<u8> {
    shown(rel).into_bytes()
}

/// Whether the file at `rel` still holds what the current commit `from`
/// records. A missing file has nothing to lose. LFS content is compared byte
/// exact; a plain file as git sees it.
fn working_file_is_committed(
    repo: &git2::Repository,
    workdir: &Path,
    from: &git2::Tree<'_>,
    rel: &Path,
    committed: git2::Oid,
    pointer: Option<&LfsPointer>,
) -> Result<bool, SwitchRefusal> {
    let bytes = match fs::read(workdir.join(rel)) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(true),
        Err(e) => return Err(io_failed("read working file", e)),
    };
    if committed.is_zero() {
        return Ok(false);
    }
    match pointer {
        Some(pointer) => {
            let is_committed = lfs::holds_content_of(&bytes, pointer);
            if is_committed {
                lfs::keep_content(repo, pointer, &bytes)
                    .map_err(|e| io_failed("store LFS object", e))?;
            }
            Ok(is_committed)
        }
        None => working_file_matches_tree(repo, from, rel).map_err(|e| failed("git diff", e)),
    }
}

fn lfs_change(
    repo: &git2::Repository,
    workdir: &Path,
    from: &git2::Tree<'_>,
    rel: &Path,
    delta: &git2::DiffDelta<'_>,
    before: Option<LfsPointer>,
    after: Option<LfsPointer>,
) -> Result<LfsChange, SwitchRefusal> {
    let (old_id, new_id) = (delta.old_file().id(), delta.new_file().id());
    if !working_file_is_committed(repo, workdir, from, rel, old_id, before.as_ref())? {
        return Err(SwitchRefusal::WouldOverwrite);
    }
    let incoming = match after {
        Some(pointer) => {
            let object = lfs::object_path(repo, &pointer.oid);
            if !lfs::object_holds(&object, &pointer.oid) {
                return Err(SwitchRefusal::LfsContentMissing(shown(rel)));
            }
            Incoming::Object(object)
        }
        None if new_id.is_zero() => Incoming::Absent,
        None => Incoming::Blob(new_id),
    };
    let repo_path = delta
        .new_file()
        .path_bytes()
        .or(delta.old_file().path_bytes())
        .unwrap_or_default()
        .to_vec();
    Ok(LfsChange {
        rel: rel.to_path_buf(),
        repo_path,
        incoming,
        blob: new_id,
        mode: u32::from(delta.new_file().mode()),
    })
}

/// Every check that can refuse the switch, before any file is written.
fn plan_switch(
    repo: &git2::Repository,
    workdir: &Path,
    from: &git2::Tree<'_>,
    to: &git2::Tree<'_>,
    held: &[PathBuf],
) -> Result<Plan, SwitchRefusal> {
    let diff = repo
        .diff_tree_to_tree(Some(from), Some(to), None)
        .map_err(|e| failed("git diff", e))?;
    let mut plan = Plan::default();
    for delta in diff.deltas() {
        let Some(rel) = delta.new_file().path().or(delta.old_file().path()) else {
            continue;
        };
        if held.iter().any(|h| rel.starts_with(h)) {
            return Err(SwitchRefusal::HeldFileDiffers(shown(rel)));
        }
        let pointer = |id| lfs::pointer_in_blob(repo, id).map_err(|e| failed("git blob", e));
        let before = pointer(delta.old_file().id())?;
        let after = pointer(delta.new_file().id())?;
        if before.is_none() && after.is_none() {
            plan.plain.push(rel.to_path_buf());
        } else if !is_safe_to_write(workdir, rel) {
            return Err(SwitchRefusal::UnsafePath(shown(rel)));
        } else {
            plan.lfs
                .push(lfs_change(repo, workdir, from, rel, &delta, before, after)?);
        }
    }
    Ok(plan)
}

/// The LFS-handled working files, moved aside until the switch has landed.
struct Stash {
    dir: PathBuf,
    moved: Vec<(PathBuf, PathBuf)>,
}

impl Stash {
    fn begin(
        repo: &git2::Repository,
        workdir: &Path,
        changes: &[LfsChange],
    ) -> Result<Self, SwitchRefusal> {
        let dir = repo
            .path()
            .join("signex")
            .join(format!("switch-{}", std::process::id()));
        let mut stash = Self {
            dir,
            moved: Vec::new(),
        };
        if changes.is_empty() {
            return Ok(stash);
        }
        fs::create_dir_all(&stash.dir).map_err(|e| io_failed("create switch folder", e))?;
        for (n, change) in changes.iter().enumerate() {
            let abs = workdir.join(&change.rel);
            if !abs.exists() {
                continue;
            }
            let kept = stash.dir.join(n.to_string());
            if let Err(e) = fs::rename(&abs, &kept) {
                let lost = stash.put_back(workdir, &[]);
                return Err(if lost.is_empty() {
                    io_failed("move working file", e)
                } else {
                    SwitchRefusal::NotRestored(lost)
                });
            }
            stash.moved.push((abs, kept));
        }
        Ok(stash)
    }

    /// Remove what the switch placed and return every moved file. A file
    /// that cannot go back stays in the folder, which is then kept. Returns
    /// the paths left wrong. A path behind a link was never placed, and
    /// removing it would reach outside the tree.
    fn put_back(&mut self, workdir: &Path, changes: &[LfsChange]) -> Vec<String> {
        let mut lost = Vec::new();
        for change in changes {
            let abs = workdir.join(&change.rel);
            let was_moved = self.moved.iter().any(|(path, _)| *path == abs);
            if was_moved
                || matches!(change.incoming, Incoming::Absent)
                || !is_safe_to_write(workdir, &change.rel)
            {
                continue;
            }
            if let Err(e) = fs::remove_file(&abs)
                && e.kind() != std::io::ErrorKind::NotFound
            {
                tracing::error!(target: "signex::library", path = %abs.display(), error = %e,
                    "branch switch: a placed file could not be removed");
                lost.push(shown(&change.rel));
            }
        }
        let mut is_whole = true;
        for (abs, kept) in self.moved.drain(..).rev() {
            let rel = abs.strip_prefix(workdir).unwrap_or(&abs);
            let returned = if is_safe_to_write(workdir, rel) {
                fs::rename(&kept, &abs)
            } else {
                Err(std::io::Error::other("a folder on its path became a link"))
            };
            if let Err(e) = returned {
                is_whole = false;
                tracing::error!(target: "signex::library", path = %abs.display(),
                    kept = %kept.display(), error = %e,
                    "branch switch: a working file could not be put back");
                lost.push(shown(rel));
            }
        }
        if is_whole {
            self.remove_folder();
        }
        lost
    }

    fn remove_folder(&self) {
        if let Err(e) = fs::remove_dir_all(&self.dir)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(target: "signex::library", path = %self.dir.display(), error = %e,
                "branch switch: its temporary folder was left behind");
        }
    }
}

fn check_out_plain(
    repo: &git2::Repository,
    to: &git2::Tree<'_>,
    plain: &[PathBuf],
) -> Result<(), SwitchRefusal> {
    // An empty path list would mean every path.
    if plain.is_empty() {
        return Ok(());
    }
    let mut checkout = git2::build::CheckoutBuilder::new();
    // An ignored file is local work no commit holds: it surfaces as a conflict.
    checkout
        .safe()
        .overwrite_ignored(false)
        .disable_pathspec_match(true);
    for rel in plain {
        checkout.path(rel.as_path());
    }
    repo.checkout_tree(to.as_object(), Some(&mut checkout))
        .map_err(|e| match e.code() {
            git2::ErrorCode::Conflict => SwitchRefusal::WouldOverwrite,
            _ => failed("git checkout", e),
        })
}

fn place_lfs_file(
    repo: &git2::Repository,
    workdir: &Path,
    change: &LfsChange,
) -> Result<(), SwitchRefusal> {
    let bytes = match &change.incoming {
        Incoming::Absent => return Ok(()),
        Incoming::Object(object) => {
            fs::read(object).map_err(|e| io_failed("read LFS object", e))?
        }
        Incoming::Blob(id) => repo
            .find_blob(*id)
            .map_err(|e| failed("git blob", e))?
            .content()
            .to_vec(),
    };
    // Again here: the plain checkout just ran and may have created a link.
    if !is_safe_to_write(workdir, &change.rel) {
        return Err(SwitchRefusal::UnsafePath(shown(&change.rel)));
    }
    let abs = workdir.join(&change.rel);
    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent).map_err(|e| io_failed("create folder", e))?;
    }
    signex_types::atomic_io::atomic_write(&abs, &bytes)
        .map_err(|e| io_failed("write working file", e))
}

fn index_time(time: SystemTime) -> git2::IndexTime {
    let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    git2::IndexTime::new(since.as_secs() as i32, since.subsec_nanos())
}

/// The stat fields only some platforms have: change time, device, inode,
/// user and group.
#[cfg(unix)]
fn platform_stat(meta: &fs::Metadata) -> (git2::IndexTime, u32, u32, u32, u32) {
    use std::os::unix::fs::MetadataExt;
    let ctime = git2::IndexTime::new(meta.ctime() as i32, meta.ctime_nsec() as u32);
    (
        ctime,
        meta.dev() as u32,
        meta.ino() as u32,
        meta.uid(),
        meta.gid(),
    )
}

#[cfg(not(unix))]
fn platform_stat(meta: &fs::Metadata) -> (git2::IndexTime, u32, u32, u32, u32) {
    let created = meta.created().unwrap_or(UNIX_EPOCH);
    (index_time(created), 0, 0, 0, 0)
}

/// The index entry of a placed LFS file: the pointer's id with the real
/// file's stat. Built by hand because `add_path` would store the whole file
/// as a git blob, which is what LFS exists to avoid.
fn lfs_index_entry(workdir: &Path, change: &LfsChange) -> std::io::Result<git2::IndexEntry> {
    let meta = fs::metadata(workdir.join(&change.rel))?;
    let (ctime, dev, ino, uid, gid) = platform_stat(&meta);
    Ok(git2::IndexEntry {
        ctime,
        mtime: index_time(meta.modified()?),
        dev,
        ino,
        mode: change.mode,
        uid,
        gid,
        file_size: meta.len() as u32,
        id: change.blob,
        flags: change.repo_path.len().min(0xFFF) as u16,
        flags_extended: 0,
        path: change.repo_path.clone(),
    })
}

/// The index records the pointer, with the stat of the real file beside it,
/// as a commit of an LFS file leaves it.
fn record_lfs_in_index(
    repo: &git2::Repository,
    workdir: &Path,
    changes: &[LfsChange],
) -> Result<(), git2::Error> {
    if changes.is_empty() {
        return Ok(());
    }
    let mut index = repo.index()?;
    for change in changes {
        if matches!(change.incoming, Incoming::Absent) {
            index.remove_path(&change.rel)?;
            continue;
        }
        let entry = lfs_index_entry(workdir, change)
            .map_err(|e| git2::Error::from_str(&format!("read placed file: {e}")))?;
        index.add(&entry)?;
    }
    index.write()
}

/// A remote target with no local branch gets one tracking it. Returned so a
/// failure after this can remove it.
fn create_tracking_branch<'r>(
    repo: &'r git2::Repository,
    target: &BranchTarget,
    name: &str,
    commit: &git2::Commit<'r>,
) -> Result<Option<git2::Branch<'r>>, SwitchRefusal> {
    let BranchTarget::Remote(full) = target else {
        return Ok(None);
    };
    if repo.find_branch(name, git2::BranchType::Local).is_ok() {
        return Ok(None);
    }
    let mut branch = repo
        .branch(name, commit, false)
        .map_err(|e| failed("git branch", e))?;
    if let Err(e) = branch.set_upstream(Some(full)) {
        delete_branch(&mut branch);
        return Err(failed("git upstream", e));
    }
    Ok(Some(branch))
}

fn delete_branch(branch: &mut git2::Branch<'_>) {
    if let Err(e) = branch.delete() {
        tracing::warn!(target: "signex::library", error = %e,
            "branch switch: the branch it created could not be removed");
    }
}

fn apply_switch<'r>(
    repo: &'r git2::Repository,
    workdir: &Path,
    target: &BranchTarget,
    commit: &git2::Commit<'r>,
    to: &git2::Tree<'_>,
    plan: &Plan,
) -> Result<String, SwitchRefusal> {
    check_out_plain(repo, to, &plan.plain)?;
    for change in &plan.lfs {
        place_lfs_file(repo, workdir, change)?;
    }
    record_lfs_in_index(repo, workdir, &plan.lfs).map_err(|e| failed("git index", e))?;
    let name = local_name(target).to_owned();
    let mut created = create_tracking_branch(repo, target, &name, commit)?;
    if let Err(e) = repo.set_head(&format!("refs/heads/{name}")) {
        if let Some(branch) = created.as_mut() {
            delete_branch(branch);
        }
        return Err(failed("git set head", e));
    }
    Ok(name)
}

/// Whether the working file at `rel` holds what `tree` records, as git sees
/// it, or is absent when the tree has no such path.
fn matches_tree(
    repo: &git2::Repository,
    workdir: &Path,
    tree: &git2::Tree<'_>,
    rel: &Path,
) -> bool {
    let Ok(entry) = tree.get_path(rel) else {
        return fs::symlink_metadata(workdir.join(rel)).is_err();
    };
    entry.kind() == Some(git2::ObjectType::Blob)
        && working_file_matches_tree(repo, tree, rel).is_ok_and(|matches| matches)
}

/// Put the plain files back, trying every one, and return the paths left
/// wrong. A path the old tree lacks was added by this switch: the safe
/// checkout refuses one that existed before. One that is not safe to write
/// was never written, and removing it would reach outside the tree.
fn restore_plain(
    repo: &git2::Repository,
    workdir: &Path,
    from: &git2::Tree<'_>,
    plain: &[PathBuf],
) -> Vec<String> {
    if plain.is_empty() {
        return Vec::new();
    }
    let mut lost = Vec::new();
    for rel in plain {
        if from.get_path(rel).is_ok() || !is_safe_to_write(workdir, rel) {
            continue;
        }
        if let Err(e) = fs::remove_file(workdir.join(rel))
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::error!(target: "signex::library", path = %rel.display(), error = %e,
                "branch switch: an added file could not be removed");
            lost.push(shown(rel));
        }
    }
    let mut force = git2::build::CheckoutBuilder::new();
    force.force().disable_pathspec_match(true);
    for rel in plain {
        force.path(rel.as_path());
    }
    if let Err(e) = repo.checkout_tree(from.as_object(), Some(&mut force)) {
        tracing::error!(target: "signex::library", error = %e,
            "branch switch: the old files could not be checked out again");
        for rel in plain {
            let name = shown(rel);
            if is_safe_to_write(workdir, rel)
                && !lost.contains(&name)
                && !matches_tree(repo, workdir, from, rel)
            {
                lost.push(name);
            }
        }
    }
    lost
}

fn restore_index(repo: &git2::Repository, saved: Option<&[u8]>) -> Result<(), git2::Error> {
    if let Some(bytes) = saved {
        signex_types::atomic_io::atomic_write(&repo.path().join("index"), bytes)
            .map_err(|e| git2::Error::from_str(&format!("restore index: {e}")))?;
    }
    repo.index()?.read(true)
}

/// Undo a switch that failed part way. Every step runs whatever the one
/// before it did. Returns the paths that could not be restored.
fn roll_back(
    repo: &git2::Repository,
    workdir: &Path,
    from: &git2::Tree<'_>,
    plan: &Plan,
    stash: &mut Stash,
    saved_index: Option<&[u8]>,
    wrote_plain: bool,
) -> Vec<String> {
    let mut lost = stash.put_back(workdir, &plan.lfs);
    if let Err(e) = restore_index(repo, saved_index) {
        tracing::error!(target: "signex::library", error = %e,
            "branch switch: the index could not be restored");
        lost.push(".git/index".to_owned());
    }
    if wrote_plain {
        lost.extend(restore_plain(repo, workdir, from, &plan.plain));
    }
    lost
}

/// Switch the working tree and HEAD at `root` to `target`. `held` lists the
/// paths, relative to the root, the app holds in memory without watching the
/// disk: a switch that would change one is refused. Returns the local branch
/// now checked out.
pub(crate) fn switch(
    root: &Path,
    target: &BranchTarget,
    held: &[PathBuf],
) -> Result<String, SwitchRefusal> {
    let repo = git2::Repository::open(root).map_err(|e| failed("git open", e))?;
    let _lock = lock_repo_for_commit(repo.path()).map_err(SwitchRefusal::Failed)?;
    let commit = target_commit(&repo, target)?;
    let to = commit.tree().map_err(|e| failed("git tree", e))?;
    let from = repo
        .head()
        .and_then(|head| head.peel_to_tree())
        .map_err(|_| SwitchRefusal::NoCommits)?;
    let workdir = repo
        .workdir()
        .ok_or_else(|| SwitchRefusal::Failed("the repository has no working tree".into()))?
        .to_path_buf();
    let plan = plan_switch(&repo, &workdir, &from, &to, held)?;
    let saved_index = fs::read(repo.path().join("index")).ok();
    let mut stash = Stash::begin(&repo, &workdir, &plan.lfs)?;
    match apply_switch(&repo, &workdir, target, &commit, &to, &plan) {
        Ok(name) => {
            stash.remove_folder();
            Ok(name)
        }
        Err(refusal) => {
            // A conflict is found before the checkout writes anything.
            let wrote_plain = !matches!(refusal, SwitchRefusal::WouldOverwrite);
            let lost = roll_back(
                &repo,
                &workdir,
                &from,
                &plan,
                &mut stash,
                saved_index.as_deref(),
                wrote_plain,
            );
            Err(if lost.is_empty() {
                refusal
            } else {
                SwitchRefusal::NotRestored(lost)
            })
        }
    }
}

#[cfg(test)]
#[path = "branch_switch_tests.rs"]
mod tests;
