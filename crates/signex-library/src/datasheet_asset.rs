//! Content-addressed storage for pinned datasheet PDFs.
//!
//! A `DatasheetRef::HashPinned` row stores only the SHA-256 and a display
//! filename. The bytes live beside the `.snxlib` file at
//! `assets/datasheets/<hash>.pdf`. A git-backed library commits that file
//! in the same commit as the row that pins it (see
//! [`crate::adapter::LibraryAdapter::update_row_with_assets`]), so the
//! hash resolves after a clone. The PDFs are plain git blobs: they are not
//! in the LFS extension list, and a library that wants LFS for them opts in
//! through its own `.gitattributes`.
//!
//! The bytes are read and hashed when a file is picked ([`PdfAsset`]) but
//! written only on an explicit row save ([`store_datasheet`]). Re-pinning a
//! different PDF leaves the previous `<hash>.pdf` in place: the store is
//! content-addressed, and another row may still pin it.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha2::{Digest, Sha256};

/// Directory (relative to the library root) holding pinned datasheets.
pub const DATASHEET_ASSET_DIR: &str = "assets/datasheets";

/// How far into a file the `%PDF-` marker may sit. Readers tolerate junk
/// before the header; the PDF specification allows the first 1024 bytes.
const PDF_MAGIC_WINDOW: usize = 1024;

/// The largest PDF a pick accepts. Bigger files are refused before they are
/// read or hashed: the bytes are held in memory until the row is saved.
pub const MAX_DATASHEET_BYTES: u64 = 64 * 1024 * 1024;

/// A picked file is not a PDF.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("this file is not a PDF (no %PDF header)")]
pub struct NotAPdf;

/// Whether `bytes` carry a `%PDF-` header near the start.
pub fn is_pdf(bytes: &[u8]) -> bool {
    let window = &bytes[..bytes.len().min(PDF_MAGIC_WINDOW)];
    window.windows(5).any(|w| w == b"%PDF-")
}

/// A validated PDF and its content address, held on the editor until the
/// row is saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfAsset {
    /// Lower-case hex SHA-256 of `bytes`.
    pub hash: String,
    pub bytes: Arc<[u8]>,
}

impl PdfAsset {
    /// Validate and hash `bytes`. CPU work proportional to the file size:
    /// call from a worker, not from `update`.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, NotAPdf> {
        if !is_pdf(&bytes) {
            return Err(NotAPdf);
        }
        Ok(Self {
            hash: datasheet_hash(&bytes),
            bytes: bytes.into(),
        })
    }
}

/// Lower-case hex SHA-256 of `bytes` — the content address.
pub fn datasheet_hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_hash(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Path of the pinned PDF relative to the library root
/// (`assets/datasheets/<hash>.pdf`, forward slashes, as git names it).
/// `None` when `hash` is not a 64-char lower-case hex digest.
pub fn datasheet_asset_rel_path(hash: &str) -> Option<String> {
    is_hash(hash).then(|| format!("{DATASHEET_ASSET_DIR}/{hash}.pdf"))
}

/// Where the PDF pinned under `hash` lives for the library whose
/// `.snxlib` file is `library_file`. `None` when `hash` is not a 64-char
/// lower-case hex digest (so a hand-edited row cannot escape the asset
/// directory) or the file has no parent directory.
pub fn datasheet_asset_path(library_file: &Path, hash: &str) -> Option<PathBuf> {
    let rel = datasheet_asset_rel_path(hash)?;
    Some(library_file.parent()?.join(rel))
}

/// Store `bytes` in the library's content-addressed datasheet directory
/// (atomic write) and return the hash to pin. Re-pinning identical bytes
/// rewrites the same content; bytes that are not a PDF are refused.
/// Blocking IO: call from a worker.
pub fn store_datasheet(library_file: &Path, bytes: &[u8]) -> io::Result<String> {
    if !is_pdf(bytes) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, NotAPdf));
    }
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
        assert!(datasheet_asset_rel_path("../x").is_none());
    }

    #[test]
    fn rel_path_names_the_hash_under_the_asset_dir() {
        let hash = datasheet_hash(b"%PDF-1.7 x");
        assert_eq!(
            datasheet_asset_rel_path(&hash).unwrap(),
            format!("assets/datasheets/{hash}.pdf")
        );
    }

    #[test]
    fn a_non_pdf_is_refused_when_picked_and_when_stored() {
        assert_eq!(PdfAsset::from_bytes(b"MZ not a pdf".to_vec()), Err(NotAPdf));
        let dir = tempfile::tempdir().unwrap();
        let err = store_datasheet(&dir.path().join("l.snxlib"), b"hello").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(!dir.path().join(DATASHEET_ASSET_DIR).exists());
    }

    #[test]
    fn the_pdf_header_may_follow_a_little_junk_but_not_a_lot() {
        let asset = PdfAsset::from_bytes(b"\n\n%PDF-1.4 body".to_vec()).unwrap();
        assert_eq!(asset.hash, datasheet_hash(&asset.bytes));
        let mut late = vec![b'x'; 2000];
        late.extend_from_slice(b"%PDF-1.4");
        assert!(!is_pdf(&late));
    }
}
