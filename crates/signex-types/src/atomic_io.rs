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
//! rename leaves the original at the destination AND a `.tmp`
//! sibling that future saves will overwrite.
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
fn tmp_path_for(path: &Path) -> io::Result<PathBuf> {
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

/// Atomically write `bytes` to `path`. Creates parent directories
/// (if any) as a side effect — matches `std::fs::write` ergonomics.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = tmp_path_for(path)?;

    // Write the temp file and fsync it before the rename, so its bytes
    // are durable at the point the rename makes them visible at `path`.
    {
        let mut file = File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;

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
}
