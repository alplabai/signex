use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use sha2::{Digest, Sha256};

use crate::error::{ImportWarning, ModelImportError};

const GLB_MAGIC: &[u8; 4] = b"glTF";
const GLB_VERSION: u32 = 2;
const CHUNK_JSON: u32 = 0x4E4F_534A;
#[cfg(test)]
const CHUNK_BIN: u32 = 0x004E_4942;
const GLB_HEADER_LEN: u64 = 12;
const CHUNK_HEADER_LEN: u64 = 8;

/// Most GLBs (each with its warnings sidecar) kept in a cache directory.
pub const MAX_CACHE_ENTRIES: usize = 256;

/// Compute the SHA-256-based cache path for a source file.
///
/// Cache key = sha256(canonical_path_str | mtime_unix_nanos | source_len |
/// converter_version | dependency_stamp). The path is canonicalised so
/// `a/../m.wrl` and `m.wrl` share one entry, and `dependency_stamp`
/// fingerprints files a `.gltf` pulls in (empty for self-contained formats).
/// The mtime keeps full sub-second precision and the byte length is mixed in
/// so two quick edits inside one filesystem timestamp tick still get distinct
/// keys whenever the size changes (#147).
pub fn cache_path(
    cache_dir: &Path,
    source_path: &Path,
    source_mtime: SystemTime,
    source_len: u64,
    converter_version: &str,
    dependency_stamp: &str,
) -> Result<PathBuf, ModelImportError> {
    let source_path = &source_path
        .canonicalize()
        .unwrap_or_else(|_| source_path.to_path_buf());
    let mtime_nanos = source_mtime
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| ModelImportError::CacheFailed {
            reason: e.to_string(),
        })?
        .as_nanos();

    let key = format!(
        "{}|{}|{}|{}|{}",
        source_path.to_string_lossy(),
        mtime_nanos,
        source_len,
        converter_version,
        dependency_stamp
    );

    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let hash = format!("{:x}", hasher.finalize());

    Ok(cache_dir.join(format!("{hash}.glb")))
}

/// Delete the least recently used cache entries (GLB plus sidecar) until at
/// most `keep` remain. `protect` (the entry just written) is never removed.
/// Best-effort: an entry that cannot be listed or deleted is skipped.
pub fn evict_oldest(cache_dir: &Path, keep: usize, protect: &Path) {
    let Ok(dir) = std::fs::read_dir(cache_dir) else {
        return;
    };
    let mut entries: Vec<(SystemTime, PathBuf)> = dir
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "glb"))
        .filter_map(|p| Some((p.metadata().ok()?.modified().ok()?, p)))
        .collect();
    if entries.len() <= keep {
        return;
    }
    entries.sort();
    let excess = entries.len() - keep;
    for (_, glb) in entries
        .into_iter()
        .filter(|(_, p)| p != protect)
        .take(excess)
    {
        let _ = std::fs::remove_file(warnings_path(&glb));
        let _ = std::fs::remove_file(&glb);
    }
}

/// Mark `glb_path` as just used so [`evict_oldest`] keeps it. Best-effort.
pub fn touch(glb_path: &Path) {
    if let Ok(file) = std::fs::File::options().write(true).open(glb_path) {
        let _ = file.set_modified(SystemTime::now());
    }
}

/// The sidecar beside a cached GLB that holds the warnings its conversion
/// produced, so a cache hit can report them too (#871).
pub fn warnings_path(glb_path: &Path) -> PathBuf {
    glb_path.with_extension("warnings.json")
}

/// The warnings recorded for `glb_path`'s conversion. `None` when the
/// sidecar is missing or unreadable, which the caller treats as a cache miss
/// so the model is converted again rather than served without its warnings.
pub fn read_warnings(glb_path: &Path) -> Option<Vec<ImportWarning>> {
    let bytes = std::fs::read(warnings_path(glb_path)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Returns `true` only if `glb_path` holds a structurally complete GLB 2.0:
/// the 12-byte header, a declared length equal to the file's real length, and
/// a leading JSON chunk followed by any further chunks (BIN or unknown ones a
/// loader must ignore) that exactly tile it.
///
/// A truncated or foreign file is a miss, so it is regenerated rather than
/// served (#147).
pub fn is_cache_valid(glb_path: &Path) -> bool {
    let Ok(file) = File::open(glb_path) else {
        return false;
    };
    let Ok(len) = file.metadata().map(|m| m.len()) else {
        return false;
    };
    validate_glb(file, len).unwrap_or(false)
}

/// [`is_cache_valid`] for a GLB already held in memory.
pub fn is_valid_glb(bytes: &[u8]) -> bool {
    validate_glb(std::io::Cursor::new(bytes), bytes.len() as u64).unwrap_or(false)
}

fn validate_glb<R: Read + Seek>(mut file: R, file_len: u64) -> std::io::Result<bool> {
    if file_len < GLB_HEADER_LEN + CHUNK_HEADER_LEN {
        return Ok(false);
    }

    let mut header = [0u8; GLB_HEADER_LEN as usize];
    file.read_exact(&mut header)?;
    let declared_len = u32::from_le_bytes([header[8], header[9], header[10], header[11]]);
    let version = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
    if &header[0..4] != GLB_MAGIC || version != GLB_VERSION || u64::from(declared_len) != file_len {
        return Ok(false);
    }

    let mut offset = GLB_HEADER_LEN;
    let mut chunk_types = Vec::new();
    while offset < file_len {
        if offset + CHUNK_HEADER_LEN > file_len {
            return Ok(false);
        }
        file.seek(SeekFrom::Start(offset))?;
        let mut chunk = [0u8; CHUNK_HEADER_LEN as usize];
        file.read_exact(&mut chunk)?;
        let chunk_len = u64::from(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        let chunk_type = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
        offset += CHUNK_HEADER_LEN + chunk_len;
        if chunk_len % 4 != 0 || offset > file_len {
            return Ok(false);
        }
        chunk_types.push(chunk_type);
    }

    // GLB 2.0 requires the JSON chunk first and lets loaders ignore any
    // further chunk types, so trailing unknown chunks are fine as long as
    // they tile the declared length (checked above).
    Ok(chunk_types.first() == Some(&CHUNK_JSON))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    fn mtime(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[test]
    fn cache_path_is_deterministic() {
        let dir = PathBuf::from("/tmp");
        let src = PathBuf::from("/models/part.wrl");
        let p1 = cache_path(&dir, &src, mtime(1_000_000), 10, "0.1.0", "").unwrap();
        let p2 = cache_path(&dir, &src, mtime(1_000_000), 10, "0.1.0", "").unwrap();
        assert_eq!(p1, p2);
    }

    #[test]
    fn cache_path_differs_on_mtime_change() {
        let dir = PathBuf::from("/tmp");
        let src = PathBuf::from("/models/part.wrl");
        let p1 = cache_path(&dir, &src, mtime(1_000_000), 10, "0.1.0", "").unwrap();
        let p2 = cache_path(&dir, &src, mtime(1_000_001), 10, "0.1.0", "").unwrap();
        assert_ne!(p1, p2);
    }

    #[test]
    fn cache_path_differs_on_version_change() {
        let dir = PathBuf::from("/tmp");
        let src = PathBuf::from("/models/part.wrl");
        let p1 = cache_path(&dir, &src, mtime(1_000_000), 10, "0.1.0", "").unwrap();
        let p2 = cache_path(&dir, &src, mtime(1_000_000), 10, "0.2.0", "").unwrap();
        assert_ne!(p1, p2);
    }

    #[test]
    fn cache_path_has_glb_extension() {
        let dir = PathBuf::from("/tmp");
        let src = PathBuf::from("/models/part.wrl");
        let p = cache_path(&dir, &src, mtime(1_000_000), 10, "0.1.0", "").unwrap();
        assert_eq!(p.extension().and_then(|e| e.to_str()), Some("glb"));
    }

    #[test]
    fn cache_path_differs_on_sub_second_mtime_change() {
        let dir = PathBuf::from("/tmp");
        let src = PathBuf::from("/models/part.wrl");
        let t0 = mtime(1_000_000);
        let t1 = t0 + Duration::from_millis(250);
        let p1 = cache_path(&dir, &src, t0, 10, "0.1.0", "").unwrap();
        let p2 = cache_path(&dir, &src, t1, 10, "0.1.0", "").unwrap();
        assert_ne!(p1, p2);
    }

    #[test]
    fn cache_path_differs_on_size_change() {
        let dir = PathBuf::from("/tmp");
        let src = PathBuf::from("/models/part.wrl");
        let p1 = cache_path(&dir, &src, mtime(1_000_000), 10, "0.1.0", "").unwrap();
        let p2 = cache_path(&dir, &src, mtime(1_000_000), 11, "0.1.0", "").unwrap();
        assert_ne!(p1, p2);
    }

    fn glb_bytes(json: &[u8], bin: &[u8]) -> Vec<u8> {
        crate::glb::writer::write_glb(json, bin).unwrap()
    }

    fn write_tmp(bytes: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.glb");
        std::fs::write(&path, bytes).unwrap();
        (dir, path)
    }

    #[test]
    fn complete_glb_is_valid() {
        let (_d, p) = write_tmp(&glb_bytes(br#"{"asset":{"version":"2.0"}}"#, &[]));
        assert!(is_cache_valid(&p));
        let (_d, p) = write_tmp(&glb_bytes(br#"{"asset":{"version":"2.0"}}"#, &[1, 2, 3, 4]));
        assert!(is_cache_valid(&p));
    }

    #[test]
    fn missing_empty_garbage_and_truncated_files_are_invalid() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_cache_valid(&dir.path().join("absent.glb")));

        let (_d, p) = write_tmp(b"");
        assert!(!is_cache_valid(&p));
        let (_d, p) = write_tmp(b"not a glb at all, but non-empty");
        assert!(!is_cache_valid(&p));

        let full = glb_bytes(br#"{"asset":{"version":"2.0"}}"#, &[1, 2, 3, 4]);
        for cut in [1, 11, 12, 19, 20, full.len() - 1] {
            let (_d, p) = write_tmp(&full[..cut]);
            assert!(!is_cache_valid(&p), "truncation at {cut} accepted");
        }
    }

    #[test]
    fn wrong_version_or_trailing_bytes_are_invalid() {
        let mut v1 = glb_bytes(br#"{"asset":{"version":"2.0"}}"#, &[]);
        v1[4] = 1;
        let (_d, p) = write_tmp(&v1);
        assert!(!is_cache_valid(&p));

        let mut extra = glb_bytes(br#"{"asset":{"version":"2.0"}}"#, &[]);
        extra.extend_from_slice(&[0; 4]);
        let (_d, p) = write_tmp(&extra);
        assert!(
            !is_cache_valid(&p),
            "declared length must equal file length"
        );
    }

    #[test]
    fn trailing_unknown_chunk_is_valid_but_leading_non_json_is_not() {
        let mut glb = glb_bytes(br#"{"asset":{"version":"2.0"}}"#, &[1, 2, 3, 4]);
        // Append an unknown 4-byte chunk and fix the declared total length.
        glb.extend_from_slice(&4u32.to_le_bytes());
        glb.extend_from_slice(&0x1234_5678u32.to_le_bytes());
        glb.extend_from_slice(&[9, 9, 9, 9]);
        let total = glb.len() as u32;
        glb[8..12].copy_from_slice(&total.to_le_bytes());
        let (_d, p) = write_tmp(&glb);
        assert!(is_cache_valid(&p), "unknown trailing chunk must be ignored");

        // Chunks that do not tile the declared length stay invalid.
        let mut bad = glb.clone();
        bad.truncate(bad.len() - 2);
        let total = bad.len() as u32;
        bad[8..12].copy_from_slice(&total.to_le_bytes());
        let (_d, p) = write_tmp(&bad);
        assert!(!is_cache_valid(&p));

        // First chunk must be JSON.
        let mut nojson = glb;
        nojson[16..20].copy_from_slice(&CHUNK_BIN.to_le_bytes());
        let (_d, p) = write_tmp(&nojson);
        assert!(!is_cache_valid(&p));
    }

    #[test]
    fn cache_path_differs_on_dependency_change_and_ignores_path_spelling() {
        let dir = PathBuf::from("/tmp");
        let src = PathBuf::from("/models/part.gltf");
        let a = cache_path(&dir, &src, mtime(1), 10, "0.1.0", "m.bin:1:4;").unwrap();
        let b = cache_path(&dir, &src, mtime(1), 10, "0.1.0", "m.bin:2:4;").unwrap();
        assert_ne!(a, b);

        let real = tempfile::tempdir().unwrap();
        let file = real.path().join("p.wrl");
        std::fs::write(&file, b"x").unwrap();
        std::fs::create_dir(real.path().join("sub")).unwrap();
        let roundabout = real.path().join("sub").join("..").join("p.wrl");
        let p1 = cache_path(&dir, &file, mtime(1), 1, "v", "").unwrap();
        let p2 = cache_path(&dir, &roundabout, mtime(1), 1, "v", "").unwrap();
        assert_eq!(p1, p2);
    }

    #[test]
    fn evict_oldest_drops_least_recently_used_with_sidecars_and_spares_protected() {
        let dir = tempfile::tempdir().unwrap();
        let mut paths = Vec::new();
        for i in 0..5u64 {
            let glb = dir.path().join(format!("{i}.glb"));
            std::fs::write(&glb, b"g").unwrap();
            std::fs::write(warnings_path(&glb), b"[]").unwrap();
            let f = std::fs::File::options().write(true).open(&glb).unwrap();
            f.set_modified(mtime(1_000 + i)).unwrap();
            paths.push(glb);
        }
        // Entry 0 is the oldest but protected; 1 and 2 are the next oldest.
        evict_oldest(dir.path(), 3, &paths[0]);
        assert!(paths[0].exists());
        assert!(!paths[1].exists() && !warnings_path(&paths[1]).exists());
        assert!(!paths[2].exists() && !warnings_path(&paths[2]).exists());
        assert!(paths[3].exists() && paths[4].exists());

        // Touching an entry makes it the newest.
        touch(&paths[3]);
        evict_oldest(dir.path(), 2, &paths[4]);
        assert!(paths[3].exists() && paths[4].exists() && !paths[0].exists());
    }
}
