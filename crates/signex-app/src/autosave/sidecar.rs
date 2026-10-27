//! The recovery sidecar of one document: where it lives and how it is
//! written and removed.
//!
//! **Location (decision for #104).** Sidecars live in one recovery
//! directory under the per-user config root, not next to the document.
//! Next to the document they would show up in the project tree, in
//! `git status` and in a synced folder, would fail for a read-only
//! folder, and would sit one rename away from the real file. The recovery
//! directory keeps every project clean. The cost is that the name must
//! encode the document's full path: `<file name>-<hash of the path>.recovery`.
//!
//! The sidecar holds exactly the bytes a normal save would write, so a
//! user can also rescue it by hand (copy it, rename it to `.snxsch`). It
//! is always a different file from the document, and it is written with
//! [`signex_types::atomic_io::atomic_write`]: a crash mid-autosave leaves
//! the previous sidecar or the new one, never half of one.

use std::path::{Path, PathBuf};

/// Extension that marks a file as a recovery sidecar.
pub const SIDECAR_EXTENSION: &str = "recovery";

/// FNV-1a over the path text. Stable across Rust versions and platforms
/// (unlike `DefaultHasher`), which is what a name that must be found again
/// on the next launch needs.
fn path_hash(path: &Path) -> u64 {
    bytes_hash(path.to_string_lossy().as_bytes())
}

/// FNV-1a over `bytes`; also the stamp of a draft without a revision counter.
pub fn bytes_hash(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The sidecar path for the document at `document` inside `recovery_dir`.
pub fn sidecar_path(recovery_dir: &Path, document: &Path) -> PathBuf {
    let name = document
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "document".to_string());
    recovery_dir.join(format!(
        "{name}-{:016x}.{SIDECAR_EXTENSION}",
        path_hash(document)
    ))
}

/// Atomically write `bytes` as the sidecar of `document`.
pub fn write(recovery_dir: &Path, document: &Path, bytes: &[u8]) -> Result<(), String> {
    let sidecar = sidecar_path(recovery_dir, document);
    signex_types::atomic_io::atomic_write(&sidecar, bytes)
        .map_err(|e| format!("{e}: could not write recovery copy {}", sidecar.display()))
}

/// Remove the sidecar of `document`. A sidecar that is already gone is
/// success: the goal is "no sidecar", and two cleanups may race.
pub fn remove(recovery_dir: &Path, document: &Path) -> Result<(), String> {
    let sidecar = sidecar_path(recovery_dir, document);
    match std::fs::remove_file(&sidecar) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!(
            "{e}: could not remove recovery copy {}",
            sidecar.display()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_is_never_the_document_and_differs_per_directory() {
        let dir = Path::new("rec");
        let a = Path::new("proj_a/main.snxsch");
        let b = Path::new("proj_b/main.snxsch");
        assert_ne!(sidecar_path(dir, a), a);
        assert_ne!(sidecar_path(dir, a), sidecar_path(dir, b));
        assert_eq!(sidecar_path(dir, a), sidecar_path(dir, a));
        assert_eq!(sidecar_path(dir, a).extension().unwrap(), SIDECAR_EXTENSION);
    }

    #[test]
    fn write_then_remove_round_trips_and_remove_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let doc = tmp.path().join("main.snxsch");
        let rec = tmp.path().join("recovery");
        write(&rec, &doc, b"draft").unwrap();
        assert_eq!(std::fs::read(sidecar_path(&rec, &doc)).unwrap(), b"draft");
        assert!(!doc.exists(), "the document itself is never created");
        remove(&rec, &doc).unwrap();
        remove(&rec, &doc).unwrap();
        assert!(!sidecar_path(&rec, &doc).exists());
    }
}
