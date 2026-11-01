//! What an open document was last known to hold on disk (#1826).
//!
//! A save compares the file with this fingerprint right before it writes, so a
//! change made by another program (a `git pull`, a sync client, an editor) is
//! never overwritten silently. See `docs/architecture/external-change.md`.

use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Fingerprint of the exact bytes of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    pub fn of(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }
}

/// What one read of a document's file learned: its fingerprint, and the file
/// it really is once symlinks are followed (#1827).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskRead {
    pub hash: ContentHash,
    pub real_path: Option<PathBuf>,
}

impl DiskRead {
    /// Taken on the worker that read `text` from `path`.
    pub fn of(path: &Path, text: &str) -> Self {
        Self {
            hash: ContentHash::of(text.as_bytes()),
            real_path: std::fs::canonicalize(path).ok(),
        }
    }

    /// For tests that build a read without a file.
    #[cfg(test)]
    pub fn of_bytes(bytes: &[u8]) -> Self {
        Self {
            hash: ContentHash::of(bytes),
            real_path: None,
        }
    }
}

/// How the file on disk differs from what the document was loaded from or
/// last saved as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskDrift {
    /// The file holds other bytes.
    Changed,
    /// The file is gone.
    Missing,
    /// The folder that held the file is gone too (moved or deleted). A save
    /// cannot re-create it (#1828), so the card offers no Re-create (#1864).
    FolderMissing,
}

impl DiskDrift {
    /// The one wording of the difference, for Messages, errors and the card:
    /// "<file> {describe}".
    pub fn describe(self) -> &'static str {
        match self {
            Self::Changed => "changed on disk since it was opened or last saved",
            Self::Missing => "is missing on disk (deleted or moved)",
            Self::FolderMissing => {
                "cannot be saved: its folder is missing on disk (moved or deleted)"
            }
        }
    }
}

/// Whether `path` still holds one of the `known` fingerprints. A blocking
/// read: call it from a worker, never from `update`.
pub fn drift_from(path: &Path, known: &[ContentHash]) -> io::Result<Option<DiskDrift>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok((!known.contains(&ContentHash::of(&bytes))).then_some(DiskDrift::Changed)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Some(
            if path
                .parent()
                .is_some_and(|dir| !dir.as_os_str().is_empty() && !dir.is_dir())
            {
                DiskDrift::FolderMissing
            } else {
                DiskDrift::Missing
            },
        )),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_bytes_are_no_drift_and_other_bytes_are() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.snxsch");
        std::fs::write(&path, b"one").unwrap();
        let one = ContentHash::of(b"one");
        assert_eq!(drift_from(&path, &[one]).unwrap(), None);
        std::fs::write(&path, b"two").unwrap();
        assert_eq!(drift_from(&path, &[one]).unwrap(), Some(DiskDrift::Changed));
        assert_eq!(
            drift_from(&path, &[one, ContentHash::of(b"two")]).unwrap(),
            None
        );
    }

    #[test]
    fn a_vanished_folder_is_not_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let known = [ContentHash::of(b"x")];
        assert_eq!(
            drift_from(&dir.path().join("gone").join("a.snxsch"), &known).unwrap(),
            Some(DiskDrift::FolderMissing)
        );
    }

    #[test]
    fn a_vanished_file_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let known = [ContentHash::of(b"x")];
        assert_eq!(
            drift_from(&dir.path().join("gone"), &known).unwrap(),
            Some(DiskDrift::Missing)
        );
    }
}
