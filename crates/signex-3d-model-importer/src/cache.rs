use std::path::{Path, PathBuf};
use std::time::SystemTime;

use sha2::{Digest, Sha256};

use crate::error::ModelImportError;

/// Compute the SHA-256-based cache path for a source file.
///
/// Cache key = sha256(absolute_path_str + "|" + mtime_unix_sec_str + "|" + converter_version)
pub fn cache_path(
    cache_dir: &Path,
    source_path: &Path,
    source_mtime: SystemTime,
    converter_version: &str,
) -> Result<PathBuf, ModelImportError> {
    let mtime_secs = source_mtime
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| ModelImportError::CacheFailed {
            reason: e.to_string(),
        })?
        .as_secs();

    let key = format!(
        "{}|{}|{}",
        source_path.to_string_lossy(),
        mtime_secs,
        converter_version
    );

    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let hash = format!("{:x}", hasher.finalize());

    Ok(cache_dir.join(format!("{hash}.glb")))
}

/// Returns `true` if a valid cached GLB already exists for this source file.
pub fn is_cache_valid(glb_path: &Path) -> bool {
    glb_path.exists() && glb_path.metadata().map(|m| m.len() > 0).unwrap_or(false)
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
        let p1 = cache_path(&dir, &src, mtime(1_000_000), "0.1.0").unwrap();
        let p2 = cache_path(&dir, &src, mtime(1_000_000), "0.1.0").unwrap();
        assert_eq!(p1, p2);
    }

    #[test]
    fn cache_path_differs_on_mtime_change() {
        let dir = PathBuf::from("/tmp");
        let src = PathBuf::from("/models/part.wrl");
        let p1 = cache_path(&dir, &src, mtime(1_000_000), "0.1.0").unwrap();
        let p2 = cache_path(&dir, &src, mtime(1_000_001), "0.1.0").unwrap();
        assert_ne!(p1, p2);
    }

    #[test]
    fn cache_path_differs_on_version_change() {
        let dir = PathBuf::from("/tmp");
        let src = PathBuf::from("/models/part.wrl");
        let p1 = cache_path(&dir, &src, mtime(1_000_000), "0.1.0").unwrap();
        let p2 = cache_path(&dir, &src, mtime(1_000_000), "0.2.0").unwrap();
        assert_ne!(p1, p2);
    }

    #[test]
    fn cache_path_has_glb_extension() {
        let dir = PathBuf::from("/tmp");
        let src = PathBuf::from("/models/part.wrl");
        let p = cache_path(&dir, &src, mtime(1_000_000), "0.1.0").unwrap();
        assert_eq!(p.extension().and_then(|e| e.to_str()), Some("glb"));
    }
}
