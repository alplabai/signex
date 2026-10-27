//! Disk JSON cache for `DistributorPart` records.
//!
//! - Layout: `<root>/<provider>/<mpn>.json` (one JSON file per part).
//! - Default TTL: **24 hours**. The whole entry (datasheet URL included,
//!   it is part of the same JSON) expires at the TTL.
//! - Tests use a temp dir, never `~/.signex/`.
//!
//! `DistributorCache::with_root` is the test-friendly constructor;
//! `DistributorCache::default_root()` resolves `~/.signex/cache/distributor`
//! for runtime use.

use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;

use crate::distributor::DistributorPart;

/// 24-hour TTL per `v0.9-library-plan.md` §14a.4.
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
    ///
    /// The MPN is partially trusted (vendor-supplied), so it is encoded
    /// injectively: every byte outside `[A-Za-z0-9-._]` becomes `%XX`. That
    /// keeps `A/B` and `A_B` distinct, makes the name valid on Windows
    /// (no `:*?"<>|`), and cannot contain a path separator, so no traversal
    /// check on the MPN is needed. Callers should go through `put`/`get`,
    /// which also verify the result stays under `self.root`.
    pub fn entry_path(&self, provider: &str, mpn: &str) -> PathBuf {
        let mut safe_mpn = String::with_capacity(mpn.len());
        for b in mpn.bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_') {
                safe_mpn.push(b as char);
            } else {
                safe_mpn.push_str(&format!("%{b:02X}"));
            }
        }
        self.root.join(provider).join(format!("{safe_mpn}.json"))
    }

    /// Enforce that the entry lies strictly inside `self.root`. Rejection
    /// bubbles up as `CacheError::Io(InvalidInput)`.
    fn validate_entry_path(&self, provider: &str, mpn: &str) -> Result<PathBuf, CacheError> {
        if provider.contains("..") {
            return Err(CacheError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "cache provider contains parent-directory traversal token (..)",
            )));
        }
        let path = self.entry_path(provider, mpn);
        // `path.starts_with(&self.root)` is lexical, which is sufficient for
        // our case because we never canonicalise — the tree is brand-new and
        // `..` was already rejected above. A symlink-based escape would need
        // canonicalisation, but the cache root is created and owned by
        // Signex; we don't follow user-controlled symlinks into it.
        if !path.starts_with(&self.root) {
            return Err(CacheError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "cache key resolves outside cache root",
            )));
        }
        Ok(path)
    }

    /// Write a part to the cache. Refreshes `captured_at` is the caller's
    /// responsibility — we persist whatever is on the part.
    pub fn put(&self, provider: &str, part: &DistributorPart) -> Result<(), CacheError> {
        let path = self.validate_entry_path(provider, &part.mpn)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_vec_pretty(part)?;
        signex_types::atomic_io::atomic_write(&path, &json)?;
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
        let path = self.validate_entry_path(provider, mpn)?;
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path)?;
        let part: DistributorPart = serde_json::from_slice(&bytes)?;
        // A file under this key holding a different part is a miss, not a hit.
        if part.mpn != mpn {
            return Ok(None);
        }

        // TTL check based on `captured_at`. If TTL is zero, every entry is
        // considered expired — useful for forcing a refresh.
        let age = Utc::now().signed_duration_since(part.captured_at);
        let age = age.to_std().unwrap_or(Duration::ZERO);
        if age >= ttl {
            return Ok(None);
        }
        Ok(Some(part))
    }

    /// [`get`](Self::get) for adapter read paths: an unreadable entry (corrupt
    /// JSON, permission error, rejected path) is logged at `warn` and treated
    /// as a miss, so the caller falls through to a live fetch. Mirrors the
    /// write side, which warns on a failed `put` rather than dropping it.
    pub fn get_or_warn(&self, provider: &str, mpn: &str, ttl: Duration) -> Option<DistributorPart> {
        match self.get(provider, mpn, ttl) {
            Ok(hit) => hit,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    provider = %provider,
                    mpn = %mpn,
                    "distributor cache read failed; treating as a miss",
                );
                None
            }
        }
    }

    /// Delete a cached entry, if present. Idempotent.
    pub fn invalidate(&self, provider: &str, mpn: &str) -> Result<(), CacheError> {
        let path = self.validate_entry_path(provider, mpn)?;
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
    use crate::param::ParamMap;

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

    /// `A/B` and `A_B` are different parts and must not share a cache file;
    /// a mislabelled file under a key is a miss.
    #[test]
    fn distinct_mpns_do_not_collide_and_mismatch_is_a_miss() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DistributorCache::with_root(dir.path()).unwrap();
        let ttl = Duration::from_secs(60);
        cache.put("p", &part("A/B")).unwrap();
        assert!(cache.get("p", "A_B", ttl).unwrap().is_none());
        assert!(cache.get("p", "A/B", ttl).unwrap().is_some());
        std::fs::copy(cache.entry_path("p", "A/B"), cache.entry_path("p", "C")).unwrap();
        assert!(cache.get("p", "C", ttl).unwrap().is_none());
    }

    /// Overwriting an existing entry replaces it and leaves no `*.tmp`
    /// sibling behind. (Crash-safety itself is not observable in a unit test.)
    #[test]
    fn put_replaces_entry_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DistributorCache::with_root(dir.path()).unwrap();
        cache.put("lcsc", &part("R1")).unwrap();
        let mut newer = part("R1");
        newer.description = "newer".into();
        cache.put("lcsc", &newer).unwrap();
        let path = cache.entry_path("lcsc", "R1");
        let back: DistributorPart = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(back.description, "newer");
        let stray = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .any(|e| e.path().extension().is_some_and(|x| x == "tmp"));
        assert!(!stray);
    }

    #[test]
    fn entry_path_sanitises_slashes() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DistributorCache::with_root(dir.path()).unwrap();
        let p = cache.entry_path("digikey", "311-10.0K/CRCT");
        assert_eq!(p.file_name().unwrap(), "311-10.0K%2FCRCT.json");
    }

    /// Windows-reserved characters round-trip through `put`/`get`.
    #[test]
    fn windows_reserved_chars_are_cacheable() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DistributorCache::with_root(dir.path()).unwrap();
        let mpn = "A:B*C?D<E>F|G..H";
        cache.put("p", &part(mpn)).unwrap();
        assert!(cache.get("p", mpn, DEFAULT_TTL).unwrap().is_some());
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

    /// M2: an MPN containing `..` must be rejected on every cache op so a
    /// vendor-supplied response can't escape the cache root and overwrite
    /// arbitrary files.
    #[test]
    fn entry_path_contains_parent_dir_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DistributorCache::with_root(dir.path()).unwrap();

        // A hostile MPN is encoded into a single file name under the root.
        let evil = "../../etc/passwd";
        cache.put("lcsc", &part(evil)).unwrap();
        let path = cache.entry_path("lcsc", evil);
        assert_eq!(path.parent().unwrap(), dir.path().join("lcsc"));
        assert!(cache.get("lcsc", evil, DEFAULT_TTL).unwrap().is_some());

        // A traversing provider is rejected on every op.
        assert!(cache.get("..", "x", DEFAULT_TTL).is_err());
        assert!(cache.invalidate("..", "anything").is_err());
    }

    /// Collects the `warn!` events emitted on the current thread while `f`
    /// runs, each rendered as `message key=value ...`. A tiny hand-written
    /// subscriber, so the crate needs no `tracing-subscriber` dev-dependency
    /// (#830 review).
    fn warnings(f: impl FnOnce()) -> Vec<String> {
        use std::fmt::Write as _;
        use std::sync::{Arc, Mutex};
        use tracing::field::{Field, Visit};
        use tracing::span::{Attributes, Id, Record};
        use tracing::{Event, Level, Metadata, Subscriber};

        struct Render(String);
        impl Visit for Render {
            fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    let _ = write!(self.0, "{value:?}");
                } else {
                    let _ = write!(self.0, " {}={value:?}", field.name());
                }
            }
        }

        struct Collect(Arc<Mutex<Vec<String>>>);
        impl Subscriber for Collect {
            fn enabled(&self, _: &Metadata<'_>) -> bool {
                true
            }
            fn new_span(&self, _: &Attributes<'_>) -> Id {
                Id::from_u64(1)
            }
            fn record(&self, _: &Id, _: &Record<'_>) {}
            fn record_follows_from(&self, _: &Id, _: &Id) {}
            fn event(&self, event: &Event<'_>) {
                if *event.metadata().level() == Level::WARN {
                    let mut render = Render(String::new());
                    event.record(&mut render);
                    self.0.lock().unwrap().push(render.0);
                }
            }
            fn enter(&self, _: &Id) {}
            fn exit(&self, _: &Id) {}
        }

        let seen = Arc::new(Mutex::new(Vec::new()));
        tracing::subscriber::with_default(Collect(seen.clone()), f);
        seen.lock().unwrap().clone()
    }

    /// #806: a corrupt entry is a miss, and the read failure is logged
    /// instead of being dropped.
    #[test]
    fn get_or_warn_logs_corrupt_entry_and_misses() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DistributorCache::with_root(dir.path()).unwrap();
        let path = cache.entry_path("lcsc", "R1");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"{").unwrap();

        let mut hit = Some(part("sentinel"));
        let warned = warnings(|| hit = cache.get_or_warn("lcsc", "R1", DEFAULT_TTL));
        assert!(hit.is_none());
        assert_eq!(warned.len(), 1);
        assert!(
            warned[0].contains("distributor cache read failed"),
            "message text missing: {}",
            warned[0]
        );
        assert!(warned[0].contains("R1"), "MPN missing: {}", warned[0]);
    }

    /// An absent entry is an ordinary miss: no warning.
    #[test]
    fn get_or_warn_is_silent_on_plain_miss_and_returns_hits() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DistributorCache::with_root(dir.path()).unwrap();
        let mut miss = Some(part("sentinel"));
        let warned = warnings(|| miss = cache.get_or_warn("lcsc", "R1", DEFAULT_TTL));
        assert!(miss.is_none());
        assert!(warned.is_empty());

        cache.put("lcsc", &part("R1")).unwrap();
        let hit = cache.get_or_warn("lcsc", "R1", DEFAULT_TTL).unwrap();
        assert_eq!(hit.mpn, "R1");
    }
}
