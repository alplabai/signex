//! Put a whole new library directory in place in one step: write it into a
//! hidden staging sibling, then rename that to the target.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use uuid::Uuid;

use crate::adapter::LibraryError;

/// Renames tried before giving up: a virus scanner on Windows can hold a
/// fresh file open for a moment and make the folder rename fail.
const RENAME_ATTEMPTS: u32 = 5;
const RENAME_PAUSE: Duration = Duration::from_millis(100);

/// Write `files` into a fresh sibling of `dir`, then rename it to `dir`.
/// `dir` must not exist or be an empty folder; on failure it is left as it
/// was (an empty folder that existed is there again).
pub(super) fn write_whole(
    dir: &Path,
    files: &BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), LibraryError> {
    let existed = match std::fs::read_dir(dir) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                return Err(LibraryError::Conflict(format!(
                    "{} is not empty; a new library is written only into a missing or empty folder",
                    dir.display()
                )));
            }
            true
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(e.into()),
    };
    let parent = dir
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let prefix = staging_prefix(dir);
    let staging = parent.join(format!("{prefix}{}", Uuid::now_v7().simple()));
    std::fs::create_dir(&staging)?;
    remove_stale_staging(parent, &prefix, &staging);
    let result = fill_and_move(&staging, dir, existed, files);
    if result.is_ok() {
        sync_dir(parent);
    } else if staging.exists()
        && let Err(e) = std::fs::remove_dir_all(&staging)
    {
        tracing::warn!(
            target: "signex::library",
            "could not remove the staging folder {}: {e}",
            staging.display()
        );
    }
    result
}

/// `.<name>.writing-`: the name every staging folder for `dir` starts with.
pub(super) fn staging_prefix(dir: &Path) -> String {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "library".into());
    format!(".{name}.writing-")
}

/// Remove staging folders a crashed earlier write of the same target left
/// in `parent`: real folders (not links) with this writer's own prefix,
/// owned by the user who owns `ours`, the staging folder just made.
fn remove_stale_staging(parent: &Path, prefix: &str, ours: &Path) {
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    let Ok(own) = std::fs::symlink_metadata(ours) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let named = entry
            .file_name()
            .to_str()
            .is_some_and(|n| n.starts_with(prefix));
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !named || path == ours || !meta.file_type().is_dir() || !same_owner(&meta, &own) {
            continue;
        }
        if let Err(e) = std::fs::remove_dir_all(&path) {
            tracing::warn!(
                target: "signex::library",
                "could not remove the stale staging folder {}: {e}",
                path.display()
            );
        }
    }
}

#[cfg(unix)]
fn same_owner(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    a.uid() == b.uid()
}

/// No owner in std on other systems: the prefix and the folder check only.
#[cfg(not(unix))]
fn same_owner(_: &std::fs::Metadata, _: &std::fs::Metadata) -> bool {
    true
}

fn fill_and_move(
    staging: &Path,
    dir: &Path,
    existed: bool,
    files: &BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), LibraryError> {
    for (rel, bytes) in files {
        signex_types::atomic_io::atomic_write(&staging.join(rel), bytes)?;
    }
    if existed {
        // Renaming over a folder is not portable: the empty one goes first,
        // and comes back if the rename fails.
        std::fs::remove_dir(dir)?;
    }
    let renamed = rename_with_retries(staging, dir);
    if renamed.is_err()
        && existed
        && let Err(e) = std::fs::create_dir(dir)
    {
        tracing::warn!(
            target: "signex::library",
            "could not put back the empty folder {}: {e}",
            dir.display()
        );
    }
    Ok(renamed?)
}

fn rename_with_retries(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut attempt = 1;
    loop {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) if attempt < RENAME_ATTEMPTS && !to.exists() => {
                tracing::debug!(target: "signex::library", "rename {} failed ({e}); retrying", from.display());
                std::thread::sleep(RENAME_PAUSE);
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

/// Make the rename durable (Unix; Windows cannot open a folder handle and
/// its rename is durable without this). Best effort, as in `atomic_io`.
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Err(e) = std::fs::File::open(dir).and_then(|handle| handle.sync_all()) {
        tracing::debug!(target: "signex::library", "fsync of {} failed: {e}", dir.display());
    }
    // `dir` is read only by the unix branch.
    #[cfg(not(unix))]
    // ast-grep-ignore: no-binding-silencer
    let _ = dir;
}
