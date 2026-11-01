//! Atomic file write helper used across the workspace (HI-6).
//!
//! Crash-safety contract: a power loss or process crash mid-write
//! leaves the file system in one of two states only — the original
//! file untouched, or the new bytes fully present at the destination.
//! There is no half-written-file outcome.
//!
//! Implementation: write to a `<path>.<pid>-<counter>.tmp` sibling,
//! `fsync` it, then `rename(.tmp, path)`, then `fsync` the parent
//! directory. The pid + per-process counter suffix keeps concurrent
//! writers to the same target from colliding on one temp name (#416);
//! the temp file still lives next to `path`, so the rename stays a
//! same-filesystem atomic-replace.
//! `std::fs::rename` is atomic-replace on every platform we ship —
//! POSIX `rename(2)` by spec, Windows `MoveFileExW` with
//! `MOVEFILE_REPLACE_EXISTING`. A crash between the write and the
//! rename leaves the original at the destination AND a stranded `.tmp`
//! sibling. The name is unique per writer now (#416), so a *crashed*
//! writer's temp is not auto-reclaimed by a later save — harmless
//! clutter, not lost data; every in-process failure path cleans its own
//! temp up.
//!
//! An existing regular file keeps its permissions across the replace (a 0600
//! file stays 0600). That copy is best effort: some filesystems (SMB/CIFS,
//! FUSE, vfat) refuse `chmod`, and a save that worked before must not start
//! failing, so a refusal is logged with `tracing::warn!` and the write goes on.
//!
//! An existing read-only target is refused with [`ReadOnlyTarget`] before
//! anything is written: the temp-then-rename dance only needs the folder to be
//! writable, so without this check it would silently replace a file the user
//! locked on purpose (#1825). The contract is exactly [`std::fs::Permissions::readonly`]
//! on the file at `path`: on Unix no write bit set for anyone (`chmod 444`),
//! on Windows the read-only attribute. A file that merely is not writable by
//! the current user (owned by someone else, ACLs), or a symlink whose target
//! is read-only, is NOT covered. Callers recognise the refusal with
//! [`is_read_only_file_refusal`].
//!
//! The two `fsync`s are what make the power-loss guarantee real:
//! without fsyncing the temp file before the rename, many filesystems
//! (ext4 `data=writeback`, xfs, apfs) can persist the rename's
//! metadata before the file's data, so a power loss leaves a
//! zero-length or partially-written file at the destination. Fsyncing
//! the temp file forces the bytes down first; fsyncing the parent
//! directory forces the rename itself to be durable.
//!
//! NB: `remove_file → rename` two-step would open a window where a
//! crash between the two calls leaves NO file at the destination and
//! a stranded `.tmp`, breaking the invariant. Don't reintroduce that.

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Per-process counter mixed into the temp file name so concurrent
/// writers (threads in this process, or another process) never pick
/// the same `<path>.<pid>-<n>.tmp` sibling and race each other's
/// rename. See #416.
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Build a temp sibling path for `path`, unique per call: `<pid>-<counter>`
/// is appended before `.tmp` so concurrent writers to the same target
/// never collide on the same temp name, while the rename below stays a
/// same-directory (same-filesystem) atomic-replace.
pub(crate) fn tmp_path_for(path: &Path) -> io::Result<PathBuf> {
    let mut tmp = path.to_path_buf();
    let name = tmp
        .file_name()
        .ok_or_else(|| io::Error::other("destination path has no file name"))?;
    let pid = std::process::id();
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut s = name.to_os_string();
    s.push(format!(".{pid}-{n}.tmp"));
    tmp.set_file_name(s);
    Ok(tmp)
}

/// Give `tmp` the permissions of the regular file already at `path`, if any.
fn copy_existing_permissions(path: &Path, tmp: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() => std::fs::set_permissions(tmp, meta.permissions()),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// The write was refused because the file already at the destination is
/// read-only. Carried as the inner error of an [`io::Error`] of kind
/// [`io::ErrorKind::PermissionDenied`]; detect it with [`is_read_only_file_refusal`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOnlyTarget {
    /// The file name (no directory) that was left untouched.
    pub file_name: String,
}

impl std::fmt::Display for ReadOnlyTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} is read-only, so it was not replaced (clear the read-only flag to save)",
            self.file_name
        )
    }
}

impl std::error::Error for ReadOnlyTarget {}

/// Whether `error` is the [`ReadOnlyTarget`] refusal of [`atomic_write`].
pub fn is_read_only_file_refusal(error: &io::Error) -> bool {
    error
        .get_ref()
        .is_some_and(|inner| inner.is::<ReadOnlyTarget>())
}

/// Refuse when a regular file already at `path` is read-only.
fn refuse_read_only_target(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() && meta.permissions().readonly() => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            ReadOnlyTarget {
                file_name: path.file_name().map_or_else(
                    || path.display().to_string(),
                    |n| n.to_string_lossy().into(),
                ),
            },
        )),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Atomically write `bytes` to `path`. Creates parent directories
/// (if any) as a side effect — matches `std::fs::write` ergonomics.
///
/// Fails with a [`ReadOnlyTarget`] error, leaving the file untouched, when
/// `path` is an existing read-only file.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    refuse_read_only_target(path)?;
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = tmp_path_for(path)?;

    // Write the temp file and fsync it before the rename, so its bytes
    // are durable at the point the rename makes them visible at `path`.
    //
    // Clean up the temp sibling on any post-creation failure. The name is
    // now unique per writer (#416), so — unlike the old fixed `<path>.tmp`,
    // which the next write to the same target simply overwrote — a stranded
    // temp would never be reclaimed and repeated failures (a full or flaky
    // disk) would accumulate `<path>.<pid>-<n>.tmp` siblings unboundedly.
    {
        let mut file = File::create(&tmp)?;
        if let Err(e) = file.write_all(bytes).and_then(|()| file.sync_all()) {
            drop(file);
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
    }
    // A replaced file keeps its permissions (a 0600 file stays 0600).
    if let Err(e) = copy_existing_permissions(path, &tmp) {
        tracing::warn!(
            path = %path.display(),
            error = %e,
            "could not keep the replaced file's permissions; writing with default ones"
        );
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }

    // Fsync the destination directory so the rename entry itself is
    // durable (Unix only — Windows `MoveFileExW` is durable without
    // this, and opening a directory handle there fails). Best-effort:
    // the data is already safe on disk, so a directory-fsync failure
    // shouldn't fail a write that otherwise succeeded.
    #[cfg(unix)]
    {
        let dir = match path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p,
            _ => Path::new("."),
        };
        if let Ok(dir_file) = File::open(dir) {
            let _ = dir_file.sync_all();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// True if `dir` contains a leftover atomic-write temp sibling
    /// (`*.tmp`). `atomic_write` picks a unique per-writer name (#416),
    /// so tests must scan for any match instead of a fixed name.
    fn has_stray_tmp(dir: &Path) -> bool {
        std::fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| entry.path().extension().is_some_and(|ext| ext == "tmp"))
    }

    #[test]
    fn round_trip_creates_destination() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hello.txt");
        atomic_write(&path, b"hi").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"hi");
        assert!(!has_stray_tmp(dir.path()));
    }

    #[cfg(unix)]
    #[test]
    fn overwrite_keeps_the_original_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret.txt");
        std::fs::write(&path, b"old").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        atomic_write(&path, b"new").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn a_missing_destination_is_not_a_permission_copy_failure() {
        let dir = tempfile::tempdir().unwrap();
        let tmp = dir.path().join("t.tmp");
        std::fs::write(&tmp, b"x").unwrap();
        copy_existing_permissions(&dir.path().join("absent"), &tmp).unwrap();
    }

    #[test]
    fn an_unreadable_destination_does_not_fail_the_write() {
        // A directory is not a regular file: its permissions are never
        // copied, and the write still reports its own outcome only.
        let dir = tempfile::tempdir().unwrap();
        let tmp = dir.path().join("t.tmp");
        std::fs::write(&tmp, b"x").unwrap();
        copy_existing_permissions(dir.path(), &tmp).unwrap();
    }

    #[test]
    fn a_read_only_destination_is_refused_and_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("locked.txt");
        std::fs::write(&path, b"old").unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&path, perms).unwrap();

        let error = atomic_write(&path, b"new").unwrap_err();

        assert!(is_read_only_file_refusal(&error), "got {error}");
        assert!(error.to_string().contains("locked.txt"));
        assert_eq!(std::fs::read(&path).unwrap(), b"old");
        assert!(!has_stray_tmp(dir.path()));

        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        std::fs::set_permissions(&path, perms).unwrap();
        atomic_write(&path, b"new").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
    }

    #[test]
    fn overwrites_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, b"old").unwrap();
        atomic_write(&path, b"new").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
    }

    #[test]
    fn creates_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b/c.txt");
        atomic_write(&path, b"deep").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"deep");
    }

    #[test]
    fn writes_empty_and_large_payloads_and_leaves_no_tmp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("d.bin");

        atomic_write(&path, b"").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"");
        assert!(!has_stray_tmp(dir.path()));

        // Overwrite with a multi-page payload to exercise write_all +
        // sync_all across more than one filesystem block.
        let big = vec![0xABu8; 256 * 1024];
        atomic_write(&path, &big).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), big);
        assert!(!has_stray_tmp(dir.path()));
    }

    #[test]
    fn tmp_path_for_is_unique_per_call() {
        let path = Path::new("/some/dir/table.json");
        let a = tmp_path_for(path).unwrap();
        let b = tmp_path_for(path).unwrap();
        assert_ne!(a, b, "two writers must not pick the same tmp sibling");
        // Both still live next to the target so the rename stays atomic
        // on the same filesystem.
        assert_eq!(a.parent(), path.parent());
        assert_eq!(b.parent(), path.parent());
    }

    #[test]
    fn a_post_creation_failure_leaves_no_stray_tmp() {
        // Force the rename to fail *after* the temp file is created: the
        // target is a non-empty directory, which no platform lets a file
        // rename replace. The unique-per-writer temp (#416) must still be
        // cleaned up here — otherwise repeated failures would accumulate
        // `<path>.<pid>-<n>.tmp` siblings, since (unlike the old fixed name)
        // nothing ever overwrites them.
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("occupant"), b"x").unwrap();

        let result = atomic_write(&target, b"data");

        assert!(
            result.is_err(),
            "rename onto a non-empty directory must fail"
        );
        assert!(
            !has_stray_tmp(dir.path()),
            "the temp sibling must be cleaned up after a failed rename",
        );
    }
}
