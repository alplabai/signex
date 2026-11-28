//! A new import's drawn preview (#3171): the converter writes the library
//! into a staging folder beside the target (same parent, so the same file
//! system), the dialog draws its parts, and Import moves the folder into
//! place with no second converter run. Blocking; call from
//! `spawn_blocking`. See docs/architecture/library-dialogs.md, "Import
//! Library dialog".
//!
//! The staging folder follows the output rules of [`super::output`]: the
//! run creates it (an existing one is refused, never adopted) and it is
//! removed without following a link ([`StagedLibrary::discard`]). An empty
//! target folder the user chose is kept, with its permissions; the staged
//! entries are moved into it. Staging folders a crash left are swept when
//! the dialog opens ([`sweep_stale`]); one whose owner still holds its lock
//! ([`super::preview_lock`]) never is.

use std::ffi::{OsStr, OsString};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::empty_dir::{FolderId, empty_folder, folder_id_at, is_link};
use super::error::ImportError;
use super::output::ClaimedOutput;
use super::place::{
    HeldDir, copy_across, copy_tree, remove_source, rename_entry, rename_no_replace, sync_dir,
};
use super::preview_lock::{LOCK_SUFFIX, Owner, owner_of};
use super::run::{LibraryRunRequest, RunOutcome};

/// What a staging folder's name holds after the library's name.
pub const STAGING_MARK: &str = ".signex-preview-";

/// What a split library's kept staging folder is renamed to hold instead,
/// so no sweep takes it for a stale preview.
pub const SPLIT_MARK: &str = ".signex-split-";

/// The file that marks a split library's staging folder when it could not
/// be renamed.
pub const SPLIT_MARKER: &str = ".signex-split";

/// The staging folder for `target`: `.<name>.signex-preview-<id>` in the
/// target's parent. `None` when the target has no parent or no name.
pub fn staging_dir_for(target: &Path) -> Option<PathBuf> {
    let parent = target.parent()?;
    let name = target.file_name()?.to_str()?;
    let id = uuid::Uuid::now_v7().simple();
    Some(parent.join(format!(".{name}{STAGING_MARK}{id}")))
}

/// What the target folder was when the preview was staged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetBefore {
    Absent,
    /// An empty folder, with its identity then.
    Empty(FolderId),
}

impl TargetBefore {
    /// Read the target: absent or an empty folder. A link, a file or a
    /// folder that holds anything is refused.
    pub fn read(target: &Path) -> Result<Self, ImportError> {
        let unusable = |e: std::io::Error| ImportError::OutputUnusable {
            path: target.to_path_buf(),
            detail: e.to_string(),
        };
        let meta = match std::fs::symlink_metadata(target) {
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Self::Absent),
            Err(e) => return Err(unusable(e)),
            Ok(meta) => meta,
        };
        if is_link(&meta) {
            return Err(ImportError::OutputIsLink {
                path: target.to_path_buf(),
            });
        }
        let taken = || ImportError::OutputNotEmpty {
            path: target.to_path_buf(),
        };
        if !meta.is_dir() {
            return Err(taken());
        }
        let id = folder_id_at(target).map_err(|e| {
            if e.kind() == ErrorKind::NotADirectory {
                ImportError::OutputIsLink {
                    path: target.to_path_buf(),
                }
            } else {
                unusable(e)
            }
        })?;
        let mut entries = std::fs::read_dir(target).map_err(unusable)?;
        match entries.next() {
            None => Ok(Self::Empty(id)),
            Some(_) => Err(taken()),
        }
    }

    /// The target is still as it was read: still absent, or still the same
    /// empty folder.
    fn unchanged(&self, target: &Path) -> bool {
        match (Self::read(target), self) {
            (Ok(Self::Absent), Self::Absent) => true,
            (Ok(Self::Empty(now)), Self::Empty(then)) => now == *then,
            _ => false,
        }
    }
}

/// A finished staged preview. Whoever holds it (the dialog) owns the
/// staging folder until Import adopts it or [`Self::discard`] removes it.
#[derive(Debug, Clone, PartialEq)]
pub struct StagedLibrary {
    /// The staging folder as the run took it.
    pub claimed: ClaimedOutput,
    /// Where Import moves it.
    pub target: PathBuf,
    pub target_before: TargetBefore,
    /// The request the preview ran (its `output` is the target), so a
    /// changed option can be told from the one previewed.
    pub request: LibraryRunRequest,
    /// The run, its paths inside the staging folder.
    pub run: RunOutcome,
    /// The `.snxlib` inside the staging folder.
    pub library_file: PathBuf,
}

/// Why a staged preview was not moved.
#[derive(Debug, Clone, PartialEq)]
pub struct MoveFailure {
    pub error: ImportError,
    /// The staging folder is still whole, so Import can be tried again.
    pub preview_kept: bool,
}

impl StagedLibrary {
    /// The staging folder.
    pub fn dir(&self) -> &Path {
        self.claimed.folder()
    }

    /// Remove the staging folder, and the parent folders the preview made
    /// while they are empty, never following a link. True when it is gone.
    pub fn discard(&self) -> bool {
        self.claimed.discard()
    }

    /// Move the staged library to the target: refused, with the preview
    /// kept, unless the target is still as it was when the preview was
    /// staged. An absent target is the staging folder renamed (a synced
    /// copy then a removal across file systems); an empty one is kept as
    /// it is, with its permissions, and the staged entries are moved into
    /// it. Nothing at the target is ever replaced ([`super::place`]).
    pub fn move_into_place(&self) -> Result<(), MoveFailure> {
        self.move_into_place_with(&mut move_entry)
    }

    /// [`Self::move_into_place`], moving each entry between the held
    /// staging and target folders with `move_one`, both ways (tests make it
    /// fail).
    pub(super) fn move_into_place_with(&self, move_one: MoveOne<'_>) -> Result<(), MoveFailure> {
        // The folder check again, just before anything moves.
        if !self.target_before.unchanged(&self.target) {
            return Err(kept(self.changed()));
        }
        let moved = match self.target_before {
            TargetBefore::Absent => self.rename_whole(),
            TargetBefore::Empty(id) => self.move_contents(id, move_one),
        };
        // No preview is left at the staging path to guard (moved, split
        // or gone), so its lock file goes now, not when the last clone of
        // the outcome drops.
        if !matches!(&moved, Err(failure) if failure.preview_kept) {
            self.claimed.release_preview_lock();
        }
        moved
    }

    fn changed(&self) -> ImportError {
        ImportError::TargetChanged {
            path: self.target.clone(),
        }
    }

    /// An absent target: the staging folder becomes it, and the parent is
    /// synced so the rename survives a crash.
    fn rename_whole(&self) -> Result<(), MoveFailure> {
        match rename_no_replace(self.dir(), &self.target) {
            Ok(()) => {
                self.sync_parent();
                Ok(())
            }
            Err(e) if crosses_devices(&e) => self.copy_into_place(),
            Err(e)
                if matches!(
                    e.kind(),
                    ErrorKind::AlreadyExists | ErrorKind::DirectoryNotEmpty
                ) =>
            {
                Err(kept(self.changed()))
            }
            Err(e) => Err(self.not_moved(ImportError::MoveFailed {
                path: self.target.clone(),
                detail: e.to_string(),
            })),
        }
    }

    /// A move that failed before anything reached the target: the preview
    /// is kept for another try only while its folder is still there (a
    /// sweep or someone else may have removed it, #3234).
    fn not_moved(&self, error: ImportError) -> MoveFailure {
        let whole = std::fs::symlink_metadata(self.dir())
            .is_ok_and(|meta| meta.is_dir() && !is_link(&meta));
        MoveFailure {
            error,
            preview_kept: whole,
        }
    }

    fn sync_parent(&self) {
        if let Some(parent) = self.target.parent()
            && let Err(e) = sync_dir(parent)
        {
            log::warn!("library import: {} not synced: {e}", parent.display());
        }
    }

    /// An empty target the user chose: held (without following a link on
    /// Unix, without sharing delete on Windows) and checked to be the folder
    /// the preview found, then each staged entry is moved into it and the
    /// empty staging folder removed. A move that fails half way moves what
    /// it moved back and keeps the preview; when that cannot be done either,
    /// the library is split and both folders are kept ([`Self::split`]).
    fn move_contents(&self, id: FolderId, move_one: MoveOne<'_>) -> Result<(), MoveFailure> {
        let target = match HeldDir::open(&self.target) {
            Ok(held) if held.id().ok() == Some(id) => held,
            _ => return Err(kept(self.changed())),
        };
        let unreadable = |e: std::io::Error| {
            self.not_moved(ImportError::MoveFailed {
                path: self.dir().to_path_buf(),
                detail: e.to_string(),
            })
        };
        let staging = HeldDir::open(self.dir()).map_err(unreadable)?;
        let names: Vec<OsString> = std::fs::read_dir(self.dir())
            .map_err(unreadable)?
            .map(|entry| entry.map(|e| e.file_name()))
            .collect::<Result<_, _>>()
            .map_err(unreadable)?;
        let mut moved: Vec<&OsString> = Vec::new();
        for name in &names {
            let Err(e) = move_one(&staging, &target, name) else {
                moved.push(name);
                continue;
            };
            let back = move_back(&staging, &target, &moved, move_one);
            // The holds go first: a held folder cannot be renamed (Windows).
            drop((staging, target));
            if !back {
                return Err(self.split(e.to_string()));
            }
            return Err(kept(if e.kind() == ErrorKind::AlreadyExists {
                self.changed()
            } else {
                ImportError::MoveFailed {
                    path: self.target.join(name),
                    detail: e.to_string(),
                }
            }));
        }
        for held in [&target, &staging] {
            if let Err(e) = held.sync() {
                log::warn!("library import: {} not synced: {e}", held.path().display());
            }
        }
        drop((staging, target));
        if let Err(e) = std::fs::remove_dir(self.dir()) {
            log::warn!(
                "library import: the emptied preview {} was left behind: {e}",
                self.dir().display()
            );
        }
        Ok(())
    }

    /// The library is split between the target and the staging folder and
    /// could not be put back together. The staging folder is kept, never
    /// discarded: it is renamed out of the staging pattern (or marked, when
    /// that fails) so no sweep removes it, and the error names both folders.
    fn split(&self, detail: String) -> MoveFailure {
        let staging = self.keep_split();
        log::error!(
            "library import: the library is split between {} and {}: {detail}",
            self.target.display(),
            staging.display()
        );
        MoveFailure {
            error: ImportError::LibrarySplit {
                target: self.target.clone(),
                staging,
                detail,
            },
            preview_kept: false,
        }
    }

    /// Where the split staging folder is kept: renamed to
    /// `.<name>.signex-split-<id>`, else marked with [`SPLIT_MARKER`].
    fn keep_split(&self) -> PathBuf {
        let renamed = self.dir().file_name().and_then(|n| n.to_str()).map(|n| {
            self.dir()
                .with_file_name(n.replacen(STAGING_MARK, SPLIT_MARK, 1))
        });
        if let Some(renamed) = renamed
            && rename_no_replace(self.dir(), &renamed).is_ok()
        {
            return renamed;
        }
        let marker = self.dir().join(SPLIT_MARKER);
        if let Err(e) = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&marker)
        {
            log::error!("library import: {} not marked: {e}", self.dir().display());
        }
        self.dir().to_path_buf()
    }

    /// The fallback across file systems: copy into a target this creates,
    /// each file and folder synced, then discard the staging folder. A copy
    /// that fails removes what it wrote and keeps the preview.
    fn copy_into_place(&self) -> Result<(), MoveFailure> {
        let failed = |e: std::io::Error| ImportError::MoveFailed {
            path: self.target.clone(),
            detail: e.to_string(),
        };
        if let Err(e) = std::fs::create_dir(&self.target) {
            return Err(kept(if e.kind() == ErrorKind::AlreadyExists {
                self.changed()
            } else {
                failed(e)
            }));
        }
        if let Err(e) = copy_tree(self.dir(), &self.target) {
            // The target was created just now, so all of it is the copy's.
            if let Err(why) = std::fs::remove_dir_all(&self.target) {
                log::error!(
                    "library import: the partial copy {} was not removed: {why}",
                    self.target.display()
                );
            }
            return Err(self.not_moved(failed(e)));
        }
        self.sync_parent();
        if !self.discard() {
            log::warn!(
                "library import: the copied preview {} was left behind",
                self.dir().display()
            );
        }
        Ok(())
    }

    /// The run as if it had written into the target: its folder, summary
    /// and file paths moved from the staging folder.
    pub fn run_in_target(&self) -> RunOutcome {
        let moved = |path: &Path| match path.strip_prefix(self.dir()) {
            Ok(rel) => self.target.join(rel),
            Err(_) => path.to_path_buf(),
        };
        let mut run = self.run.clone();
        run.library = Some(self.target.clone());
        run.summary.library = run.summary.library.as_deref().map(moved);
        run.summary.report = run.summary.report.as_deref().map(moved);
        run.summary.files = run
            .summary
            .files
            .iter()
            .map(|p| moved(p.as_path()))
            .collect();
        run
    }

    /// The `.snxlib` once moved into the target.
    pub fn library_file_in_target(&self) -> PathBuf {
        match self.library_file.strip_prefix(self.dir()) {
            Ok(rel) => self.target.join(rel),
            Err(_) => self.library_file.clone(),
        }
    }
}

/// One entry move from the first held folder into the second.
pub(super) type MoveOne<'a> = &'a mut dyn FnMut(&HeldDir, &HeldDir, &OsStr) -> std::io::Result<()>;

/// A refusal that keeps the preview.
fn kept(error: ImportError) -> MoveFailure {
    MoveFailure {
        error,
        preview_kept: true,
    }
}

/// A rename that failed because the two paths are on different file
/// systems.
fn crosses_devices(e: &std::io::Error) -> bool {
    e.kind() == ErrorKind::CrossesDevices
}

/// Best effort after a failed move: the entries already moved go back
/// into the staging folder. True when every one did.
fn move_back(
    staging: &HeldDir,
    target: &HeldDir,
    moved: &[&OsString],
    move_one: MoveOne<'_>,
) -> bool {
    let mut all = true;
    for name in moved.iter().rev() {
        if let Err(e) = move_one(target, staging, name) {
            log::error!(
                "library import: {} not moved back into the preview: {e}",
                target.path().join(name).display()
            );
            all = false;
        }
    }
    all
}

/// Move the entry `name` from `from` into `to`, never replacing anything
/// there: a rename relative to the held folders, or across file systems a
/// synced copy then a removal, after checking `to`'s path still names the
/// held folder (the copy goes by path, so a narrow race remains there).
pub(super) fn move_entry(from: &HeldDir, to: &HeldDir, name: &OsStr) -> std::io::Result<()> {
    match rename_entry(from, to, name) {
        Err(e) if crosses_devices(&e) => {}
        done => return done,
    }
    if folder_id_at(to.path()).ok() != to.id().ok() {
        return Err(std::io::Error::other(format!(
            "{} is no longer the folder the import holds",
            to.path().display()
        )));
    }
    copy_across(
        &from.path().join(name),
        &to.path().join(name),
        &mut remove_source,
    )
}

/// How long a staging folder must have gone unchanged before a sweep takes
/// it. A preview another running Signex holds is left alone whatever its
/// age, by its lock ([`super::preview_lock`]); the age covers a preview
/// without a lock file (an older build's, or one whose lock could not be
/// made), and a sweep never takes one of this session.
pub const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(3600);

/// What a sweep takes as stale: changed before this session `started` and
/// at least [`STALE_AFTER`] ago.
pub fn stale_cutoff(started: SystemTime) -> SystemTime {
    SystemTime::now()
        .checked_sub(STALE_AFTER)
        .map_or(started, |quiet| quiet.min(started))
}

/// Remove the staging folders a crash left (#3171): in each of `roots`,
/// the folders whose name holds [`STAGING_MARK`] and that were last changed
/// before `started` (callers pass [`stale_cutoff`] of this session's start),
/// so no preview of this session is touched. A folder whose lock a running
/// process holds is left alone, however old (#3234). A link, a file, a
/// split library's folder (renamed out of the pattern, or holding
/// [`SPLIT_MARKER`]) is left alone; a folder is emptied through its
/// identity, never following a link. A lock file whose folder is gone and
/// whose owner is gone goes too. Returns how many previews went.
pub fn sweep_stale(roots: &[PathBuf], started: SystemTime) -> usize {
    let mut removed = 0;
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name
                .to_str()
                .filter(|n| n.starts_with('.') && n.contains(STAGING_MARK))
            else {
                continue;
            };
            if let Some(folder) = name.strip_suffix(LOCK_SUFFIX) {
                remove_orphan_lock(&root.join(folder), &entry.path(), started);
            } else if is_stale(&entry.path(), started) && sweep_one(&entry.path()) {
                removed += 1;
            }
        }
    }
    removed
}

/// Remove the stale preview `path` unless its owner is still running.
fn sweep_one(path: &Path) -> bool {
    match owner_of(path) {
        Owner::Alive => {
            log::info!(
                "library import: the preview {} is in use, not swept",
                path.display()
            );
            false
        }
        Owner::Unknown => remove_stale(path),
        // The lock file goes when `lock` drops, after the folder.
        Owner::Gone(lock) => {
            let removed = remove_stale(path);
            drop(lock);
            removed
        }
    }
}

/// A lock file left without its folder (a crash between the two): removed
/// once it is older than `started` and nobody holds it.
fn remove_orphan_lock(folder: &Path, lock_file: &Path, started: SystemTime) {
    let old_plain_file = std::fs::symlink_metadata(lock_file).is_ok_and(|meta| {
        meta.is_file() && !is_link(&meta) && meta.modified().is_ok_and(|at| at < started)
    });
    let folder_gone =
        std::fs::symlink_metadata(folder).is_err_and(|e| e.kind() == ErrorKind::NotFound);
    if old_plain_file
        && folder_gone
        && let Owner::Gone(lock) = owner_of(folder)
    {
        drop(lock);
    }
}

fn is_stale(path: &Path, started: SystemTime) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if is_link(&meta) || !meta.is_dir() || path.join(SPLIT_MARKER).exists() {
        return false;
    }
    meta.modified().is_ok_and(|changed| changed < started)
}

fn remove_stale(path: &Path) -> bool {
    let emptied = folder_id_at(path).and_then(|id| empty_folder(path, &id));
    match emptied.and_then(|()| std::fs::remove_dir(path)) {
        Ok(()) => {
            log::info!(
                "library import: removed the stale preview {}",
                path.display()
            );
            true
        }
        Err(e) => {
            log::warn!(
                "library import: the stale preview {} was not removed: {e}",
                path.display()
            );
            false
        }
    }
}
