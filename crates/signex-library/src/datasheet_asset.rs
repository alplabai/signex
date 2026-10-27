//! Content-addressed storage for pinned datasheet PDFs.
//!
//! A `DatasheetRef::HashPinned` row stores only the SHA-256 and a display
//! filename. The bytes live beside the `.snxlib` file at
//! `assets/datasheets/<hash>.pdf`, so the hash resolves to a file that
//! travels with the library (and its git repo).

use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Directory (relative to the library root) holding pinned datasheets.
pub const DATASHEET_ASSET_DIR: &str = "assets/datasheets";

/// Lower-case hex SHA-256 of `bytes` — the content address.
pub fn datasheet_hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Where the PDF pinned under `hash` lives for the library whose
/// `.snxlib` file is `library_file`. `None` when `hash` is not a 64-char
/// lower-case hex digest (so a hand-edited row cannot escape the asset
/// directory) or the file has no parent directory.
pub fn datasheet_asset_path(library_file: &Path, hash: &str) -> Option<PathBuf> {
    let valid = hash.len() == 64 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if !valid {
        return None;
    }
    let root = library_file.parent()?;
    Some(root.join(DATASHEET_ASSET_DIR).join(format!("{hash}.pdf")))
}

/// Store `bytes` in the library's content-addressed datasheet directory
/// (atomic write) and return the hash to pin. Re-pinning identical bytes
/// rewrites the same content. Blocking IO: call from a worker.
pub fn store_datasheet(library_file: &Path, bytes: &[u8]) -> io::Result<String> {
    let hash = datasheet_hash(bytes);
    let path = datasheet_asset_path(library_file, &hash).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "library file has no parent directory",
        )
    })?;
    signex_types::atomic_io::atomic_write(&path, bytes)?;
    Ok(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_bytes_under_their_hash() {
        let dir = tempfile::tempdir().unwrap();
        let lib = dir.path().join("lib.snxlib");
        let bytes = b"%PDF-1.7 fake datasheet";
        let hash = store_datasheet(&lib, bytes).unwrap();
        let path = datasheet_asset_path(&lib, &hash).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), bytes);
        assert_eq!(hash, datasheet_hash(bytes));
    }

    #[test]
    fn rejects_non_hex_hash() {
        let lib = Path::new("/x/lib.snxlib");
        assert!(datasheet_asset_path(lib, "../../etc/passwd").is_none());
        assert!(datasheet_asset_path(lib, "ABC").is_none());
    }
}
