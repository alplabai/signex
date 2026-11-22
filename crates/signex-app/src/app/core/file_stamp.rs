//! What a file looked like when it was read: plain state, UI-free (ADR-0006).
//! The host stats the file (`FileStamp::of` in `library::closed_sheets`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

/// A file's modified time and length: cheap to compare, enough to see a write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStamp {
    modified: Option<SystemTime>,
    len: u64,
}

impl FileStamp {
    pub(crate) fn new(modified: Option<SystemTime>, len: u64) -> Self {
        Self { modified, len }
    }
}

/// The stamp of each sheet a read took from disk (`None`: it could not be
/// stated).
pub(crate) type DiskStamps = HashMap<PathBuf, Option<FileStamp>>;
