//! Bounded reads of untrusted model files (#708).

use std::io::{self, Read};
use std::path::Path;

/// Largest model file, and the total of all external buffers and textures one
/// glTF may pull in.
pub const MAX_READ_BYTES: u64 = 256 * 1024 * 1024;

/// Largest `.gltf` JSON text. Real files are a few MiB at most; the heavy data
/// lives in buffers, which have their own budget.
pub const MAX_GLTF_JSON_BYTES: u64 = 32 * 1024 * 1024;

/// A running byte allowance shared by every external read of one model, so
/// many references to one big file cannot multiply the per-file cap.
pub struct ReadBudget {
    remaining: u64,
}

impl ReadBudget {
    pub fn new(total: u64) -> Self {
        Self { remaining: total }
    }

    /// [`read_capped`] limited to what is left; the bytes read are spent.
    pub fn read(&mut self, path: &Path) -> io::Result<Vec<u8>> {
        let bytes = read_capped(path, self.remaining)?;
        self.remaining -= bytes.len() as u64;
        Ok(bytes)
    }
}

/// `O_NONBLOCK`, so opening a FIFO with no writer returns at once.
#[cfg(unix)]
fn open_nonblocking(path: &Path) -> io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const O_NONBLOCK: i32 = 0o4000;
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    const O_NONBLOCK: i32 = 0x4;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(O_NONBLOCK)
        .open(path)
}

#[cfg(not(unix))]
fn open_nonblocking(path: &Path) -> io::Result<std::fs::File> {
    std::fs::File::open(path)
}

/// Read `path` fully, refusing anything but a regular file of at most `max`
/// bytes. The file is opened first and its type and size are checked on the
/// open handle, so the path cannot be swapped between check and read, and a
/// FIFO or device never blocks; the `take` guards a file that grows after the
/// check.
pub fn read_capped(path: &Path, max: u64) -> io::Result<Vec<u8>> {
    let file = open_nonblocking(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    if meta.len() > max {
        return Err(too_large(max));
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    file.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(too_large(max));
    }
    Ok(bytes)
}

/// [`read_capped`] decoded as UTF-8 text.
pub fn read_text_capped(path: &Path, max: u64) -> io::Result<String> {
    String::from_utf8(read_capped(path, max)?)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn too_large(max: u64) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("file exceeds the {max}-byte read cap"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_and_non_regular_files_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.bin");
        std::fs::write(&f, [0u8; 10]).unwrap();
        assert_eq!(read_capped(&f, 10).unwrap().len(), 10);
        assert!(read_capped(&f, 9).is_err());
        assert!(read_capped(dir.path(), 100).is_err(), "directory refused");
    }

    #[test]
    fn budget_is_spent_across_reads() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.bin");
        std::fs::write(&f, [0u8; 10]).unwrap();
        let mut budget = ReadBudget::new(15);
        assert_eq!(budget.read(&f).unwrap().len(), 10);
        assert!(budget.read(&f).is_err(), "second read exceeds the total");
    }
}
