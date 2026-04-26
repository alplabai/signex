//! Disk JSON cache for `DistributorPart` records.
//!
//! Spec (WS-C):
//! - Layout: `<root>/<provider>/<mpn>.json` (one JSON file per part).
//! - Default TTL: **24 hours** for metadata. Datasheet URLs cached
//!   indefinitely (the URL is stored as part of the same JSON; the
//!   indefinite-ness is a property of the URL not changing — we re-read
//!   stale entries explicitly when the caller wants a refresh).
//! - Tests use a temp dir, never `~/.signex/`.
//!
//! `DistributorCache::with_root` is the test-friendly constructor;
//! `DistributorCache::default_root()` resolves `~/.signex/cache/distributor`
//! for runtime use.

use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;

use crate::distributor::DistributorPart;

/// 24-hour TTL per LIBRARY_PLAN §14a.4 / WS-C.
pub const DEFAULT_TTL: Duration = Duration::from_secs(60 * 60 * 24);

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("home directory not resolvable")]
    NoHomeDir,
}

/// Filesystem-backed cache of `DistributorPart` JSON, keyed by `(provider, mpn)`.
#[derive(Debug, Clone)]
pub struct DistributorCache {
    root: PathBuf,
}

impl DistributorCache {
    /// Construct a cache at the given root directory. Creates the root if it
    /// does not exist. Test-friendly: pass a `tempfile::TempDir` path.
    pub fn with_root(root: impl AsRef<Path>) -> Result<Self, CacheError> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    /// Resolve `~/.signex/cache/distributor` (creates it on first use).
    /// Production callers use this; tests should call [`Self::with_root`].
    pub fn default_root() -> Result<Self, CacheError> {
        let home = dirs::home_dir().ok_or(CacheError::NoHomeDir)?;
        let root = home.join(".signex").join("cache").join("distributor");
        Self::with_root(root)
    }

    /// Compute the on-disk path for a `(provider, mpn)` entry.
    pub fn entry_path(&self, provider: &str, mpn: &str) -> PathBuf {
        // mpn may contain `/` in some vendor catalogues — sanitise to `_`.
        let safe_mpn = mpn.replace(['/', '\\'], "_");
        self.root.join(provider).join(format!("{safe_mpn}.json"))
    }

    /// Write a part to the cache. Refreshes `captured_at` is the caller's
    /// responsibility — we persist whatever is on the part.
    pub fn put(&self, provider: &str, part: &DistributorPart) -> Result<(), CacheError> {
        let path = self.entry_path(provider, &part.mpn);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_vec_pretty(part)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    /// Read a cached part if it exists and is fresher than `ttl`.
    /// Returns `Ok(None)` for cache miss or stale entry.
    pub fn get(
        &self,
        provider: &str,
        mpn: &str,
        ttl: Duration,
    ) -> Result<Option<DistributorPart>, CacheError> {
        let path = self.entry_path(provider, mpn);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path)?;
        let part: DistributorPart = serde_json::from_slice(&bytes)?;

        // TTL check based on `captured_at`. If TTL is zero, every entry is
        // considered expired — useful for forcing a refresh.
        let age = Utc::now().signed_duration_since(part.captured_at);
        let age = age.to_std().unwrap_or(Duration::ZERO);
        if age >= ttl {
            return Ok(None);
        }
        Ok(Some(part))
    }

    /// Delete a cached entry, if present. Idempotent.
    pub fn invalidate(&self, provider: &str, mpn: &str) -> Result<(), CacheError> {
        let path = self.entry_path(provider, mpn);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(CacheError::Io(e)),
        }
    }

    /// Root directory of this cache. Mostly for tests/diagnostics.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distributor::DistributorSource;
    use crate::embed::ParamMap;

    fn part(mpn: &str) -> DistributorPart {
        DistributorPart {
            mpn: mpn.into(),
            manufacturer: "M".into(),
            description: "D".into(),
            datasheet_url: None,
            footprint_hint: None,
            parameters: ParamMap::new(),
            pricing: None,
            stock: None,
            source: DistributorSource::Lcsc,
            captured_at: Utc::now(),
            extra: Default::default(),
        }
    }

    #[test]
    fn entry_path_sanitises_slashes() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DistributorCache::with_root(dir.path()).unwrap();
        let p = cache.entry_path("digikey", "311-10.0K/CRCT");
        assert_eq!(p.file_name().unwrap(), "311-10.0K_CRCT.json");
    }

    #[test]
    fn invalidate_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DistributorCache::with_root(dir.path()).unwrap();
        // Missing entry — should not error.
        cache.invalidate("lcsc", "MISSING").unwrap();
        cache.put("lcsc", &part("X")).unwrap();
        cache.invalidate("lcsc", "X").unwrap();
        assert!(cache.get("lcsc", "X", DEFAULT_TTL).unwrap().is_none());
    }
}
