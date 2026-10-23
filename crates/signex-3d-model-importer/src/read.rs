//! Bounded reads of untrusted model files (#708).

use std::io::{self, Read};
use std::path::Path;

/// Largest model, buffer or texture file read into memory.
pub const MAX_READ_BYTES: u64 = 256 * 1024 * 1024;

/// Read `path` fully, refusing anything but a regular file of at most
/// `max` bytes. The checks run before the file is opened, so a FIFO or device
/// never blocks and an oversized file is never allocated; the `take` guards a
/// file that grows between the check and the read.
pub fn read_capped(path: &Path, max: u64) -> io::Result<Vec<u8>> {
    let meta = std::fs::metadata(path)?;
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
    std::fs::File::open(path)?
        .take(max + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(too_large(max));
    }
    Ok(bytes)
}

/// [`read_capped`] at [`MAX_READ_BYTES`], decoded as UTF-8 text.
pub fn read_text_capped(path: &Path) -> io::Result<String> {
    String::from_utf8(read_capped(path, MAX_READ_BYTES)?)
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
}
