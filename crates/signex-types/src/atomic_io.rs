//! Atomic file write helper used across the workspace (HI-6).
//!
//! Crash-safety contract: a power loss or process crash mid-write
//! leaves the file system in one of two states only — the original
//! file untouched, or the new bytes fully present at the destination.
//! There is no half-written-file outcome.
//!
//! Implementation: write to `<path>.tmp`, then `rename(.tmp, path)`.
//! `std::fs::rename` is atomic-replace on every platform we ship —
//! POSIX `rename(2)` by spec, Windows `MoveFileExW` with
//! `MOVEFILE_REPLACE_EXISTING`. A crash between the write and the
//! rename leaves the original at the destination AND a `.tmp`
//! sibling that future saves will overwrite.
//!
//! NB: `remove_file → rename` two-step would open a window where a
//! crash between the two calls leaves NO file at the destination and
//! a stranded `.tmp`, breaking the invariant. Don't reintroduce that.

use std::io;
use std::path::Path;

/// Atomically write `bytes` to `path`. Creates parent directories
/// (if any) as a side effect — matches `std::fs::write` ergonomics.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let mut tmp = path.to_path_buf();
    let tmp_name = match tmp.file_name() {
        Some(name) => {
            let mut s = name.to_os_string();
            s.push(".tmp");
            s
        }
        None => return Err(io::Error::other("destination path has no file name")),
    };
    tmp.set_file_name(tmp_name);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_creates_destination() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hello.txt");
        atomic_write(&path, b"hi").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"hi");
        assert!(!dir.path().join("hello.txt.tmp").exists());
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
}
