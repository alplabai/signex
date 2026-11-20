//! The test adapter's document disk: every document write goes through the
//! real atomic sink, so the disk-changed check (#1826) stays live, but only
//! under the adapter's temporary root, and each landed write is logged.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::persist::{AtomicFileSink, DocumentSink, SameFile};

/// One document write that landed, in landing order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskWrite {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

/// The document sink the adapter installs before its first open.
pub struct TestDisk {
    root: PathBuf,
    writes: Mutex<Vec<DiskWrite>>,
}

impl TestDisk {
    pub(super) fn new(root: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            root,
            writes: Mutex::new(Vec::new()),
        })
    }

    /// Every document write that landed so far, oldest first.
    pub fn writes(&self) -> Vec<DiskWrite> {
        self.log().clone()
    }

    fn log(&self) -> MutexGuard<'_, Vec<DiskWrite>> {
        self.writes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl DocumentSink for TestDisk {
    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        // A write outside the root is refused, never made: a test must not
        // touch the developer's files.
        if !path.starts_with(&self.root) || path.components().any(|c| c.as_os_str() == "..") {
            return Err(io::Error::other(format!(
                "the test disk refuses {}: outside {}",
                path.display(),
                self.root.display()
            )));
        }
        AtomicFileSink.write(path, bytes)?;
        self.log().push(DiskWrite {
            path: path.to_path_buf(),
            bytes: bytes.to_vec(),
        });
        Ok(())
    }

    fn same_file(&self, a: &Path, b: &Path) -> SameFile {
        AtomicFileSink.same_file(a, b)
    }
}
