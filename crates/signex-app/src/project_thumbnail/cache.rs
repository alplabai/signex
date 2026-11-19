//! The per-user thumbnail cache: one small versioned JSON file per project
//! under `<config root>/thumbnails/`, named by a stable hash of the project
//! path. Format and location: `docs/PROJECT_THUMBNAILS.md`.
//!
//! Everything here that touches the disk runs on a blocking worker, never in
//! `update`. A file of another version, or one that does not parse, reads as
//! no preview.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::{LoadedThumbnails, Thumbnail, ThumbnailWriteFailure, WrittenThumbnail};

/// The format version this build writes and reads. A file with any other
/// version is ignored (no preview) and is rewritten on the next save.
pub const CACHE_VERSION: u32 = 1;

/// The cache folder under the config root.
pub const CACHE_DIR: &str = "thumbnails";

/// FNV-1a, 64 bit: a hash that is the same on every platform and build
/// (unlike `DefaultHasher`), so a cache file keeps its name.
pub fn fnv1a_64(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    bytes.iter().fold(OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(PRIME)
    })
}

/// The cache file name of `project`: its path's hash, 16 hex digits.
pub fn file_name(project: &Path) -> String {
    format!(
        "{:016x}.json",
        fnv1a_64(project.to_string_lossy().as_bytes())
    )
}

/// Where the render of `project` is cached under the config `root`.
pub fn cache_path(root: &Path, project: &Path) -> PathBuf {
    root.join(CACHE_DIR).join(file_name(project))
}

/// The file body. `project` is kept so a hash collision reads as no preview
/// instead of another project's picture.
#[derive(Serialize)]
struct CacheFileOut<'a> {
    version: u32,
    project: &'a Path,
    thumbnail: &'a Thumbnail,
}

#[derive(Deserialize)]
struct CacheFileIn {
    project: PathBuf,
    thumbnail: Thumbnail,
}

/// The cache file bytes of `thumbnail` for `project`.
pub fn encode(project: &Path, thumbnail: &Thumbnail) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(&CacheFileOut {
        version: CACHE_VERSION,
        project,
        thumbnail,
    })
}

/// The render in a cache file of `project`; `None` for another version,
/// another project, or bytes that do not parse.
pub fn decode(project: &Path, bytes: &[u8]) -> Option<Thumbnail> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    if value.get("version")?.as_u64()? != u64::from(CACHE_VERSION) {
        return None;
    }
    let file: CacheFileIn = serde_json::from_value(value).ok()?;
    (file.project == project).then_some(file.thumbnail)
}

/// Write the render of `project` under `root`, crash-safe (temp file, fsync,
/// rename).
pub fn write(root: &Path, project: &Path, thumbnail: &Thumbnail) -> Result<(), String> {
    let bytes = encode(project, thumbnail).map_err(|e| e.to_string())?;
    signex_types::atomic_io::atomic_write(&cache_path(root, project), &bytes)
        .map_err(|e| e.to_string())
}

/// Write the render of the open project at `project` under the spelling the
/// recent and pinned lists use, into the per-user config root. Blocking. A
/// failure still carries that spelling, so the render kept in memory moves
/// to it either way.
pub fn write_listed(
    project: &Path,
    thumbnail: &Thumbnail,
) -> Result<WrittenThumbnail, ThumbnailWriteFailure> {
    let listed_as = crate::recent_projects::RecentProjects::spelling_of(project);
    let written = crate::config_root::config_root()
        .ok_or_else(|| "this system has no per-user configuration folder".to_string())
        .and_then(|root| write(&root, &listed_as, thumbnail));
    match written {
        Ok(()) => Ok(WrittenThumbnail {
            project: project.to_path_buf(),
            listed_as,
        }),
        Err(reason) => Err(ThumbnailWriteFailure {
            project: project.to_path_buf(),
            listed_as,
            reason,
        }),
    }
}

/// Read the cached renders of `projects` under `root`. A missing file, or
/// one of another version or that does not parse, is no preview; a file that
/// exists but cannot be read is listed in `unreadable`. Blocking.
pub fn read_all(root: &Path, projects: &[PathBuf]) -> LoadedThumbnails {
    let mut loaded = LoadedThumbnails::default();
    for project in projects {
        match std::fs::read(cache_path(root, project)) {
            Ok(bytes) => {
                if let Some(thumbnail) = decode(project, &bytes) {
                    loaded.found.push((project.clone(), Arc::new(thumbnail)));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => loaded.unreadable.push((project.clone(), error.to_string())),
        }
    }
    loaded
}
