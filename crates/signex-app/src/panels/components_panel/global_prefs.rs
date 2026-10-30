//! Signex-wide global library preferences.
//!
//! Stage 9 of `v0.9-snxlib-as-file-plan.md`. The Components Panel
//! surfaces three mount sources: Project (auto-mounted from
//! `Project.libraries`), Installed (session-scoped, in-memory), and
//! Global. This module owns the on-disk persistence for the Global
//! source — a single TOML file at
//! `<config_dir>/signex/global_libraries.toml`.
//!
//! Schema (TOML):
//! ```toml
//! [[libraries]]
//! path = "C:\\Users\\caner\\Documents\\Libraries\\Common.snxlib"
//! remote = "git@github.com:caner/common-lib.git"   # optional
//! auto_pull = true                                  # optional
//! ```
//!
//! All fields except `path` are optional. The whole file is optional —
//! a missing or unparseable file warns through `tracing` and the
//! Components Panel renders the Global section empty (no panic).
//! Updates are stricter: adding or removing a library starts from an
//! empty list only when the file is missing, and refuses to write over a
//! file that cannot be read or parsed, so its other entries are not lost
//! (#1352).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One row in `global_libraries.toml`. Mirrors the plan §3 schema —
/// `remote` and `auto_pull` are forward-compat hooks for the
/// upcoming "fetch once a day" Global library refresh; v0.9 uses
/// only `path`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalLibraryEntry {
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_pull: Option<bool>,
}

/// Top-level schema — `[[libraries]]` array.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct GlobalPrefsFile {
    #[serde(default)]
    libraries: Vec<GlobalLibraryEntry>,
}

/// Resolved on-disk path of `global_libraries.toml`. `None` when the
/// platform can't resolve a config dir (very rare — a stripped-down
/// headless environment). See [`crate::config_root::config_root`].
pub fn prefs_path() -> Option<PathBuf> {
    let root = crate::config_root::config_root()?;
    Some(root.join("global_libraries.toml"))
}

/// Load the global library list from disk. Returns an empty Vec when
/// the file is missing or unparseable — both non-fatal cases. Parse
/// errors warn through `tracing` so they're visible without taking the
/// whole panel down.
pub fn load() -> Vec<GlobalLibraryEntry> {
    let Some(path) = prefs_path() else {
        return Vec::new();
    };
    load_at(&path)
}

/// Load from a specific path — extracted so tests can hit the actual
/// parse path without going through `prefs_path()`/`config_root()`.
pub fn load_at(path: &Path) -> Vec<GlobalLibraryEntry> {
    if !path.exists() {
        return Vec::new();
    }
    match std::fs::read_to_string(path) {
        Ok(text) => match toml::from_str::<GlobalPrefsFile>(&text) {
            Ok(file) => file.libraries,
            Err(e) => {
                tracing::warn!(
                    target: "signex::library",
                    path = %path.display(),
                    error = %e,
                    "global_libraries.toml parse failed; treating as empty"
                );
                Vec::new()
            }
        },
        Err(e) => {
            tracing::warn!(
                target: "signex::library",
                path = %path.display(),
                error = %e,
                "global_libraries.toml read failed; treating as empty"
            );
            Vec::new()
        }
    }
}

/// Write the whole list. Private: a caller must hold [`with_list_lock`]
/// across the load that produced `entries` and this write, or another
/// process's concurrent update is lost (#1757).
///
/// Crash-safe: [`signex_types::atomic_io::atomic_write`] writes to a temp
/// sibling, fsyncs it and renames over the destination, so a crash mid-save
/// leaves the previous library list intact rather than a truncated file. It
/// also creates the parent directory, so no separate `create_dir_all` here.
fn save_at(path: &Path, entries: &[GlobalLibraryEntry]) -> Result<(), String> {
    let file = GlobalPrefsFile {
        libraries: entries.to_vec(),
    };
    let text = toml::to_string_pretty(&file)
        .map_err(|e| format!("serialise global_libraries.toml: {e}"))?;
    signex_types::atomic_io::atomic_write(path, text.as_bytes())
        .map_err(|e| format!("write {}: {}", path.display(), e))?;
    Ok(())
}

/// Load for a read-modify-write: like [`load_at`], but a file that exists
/// and cannot be read or parsed is an error rather than an empty list, so a
/// caller about to write never replaces the user's list with a near-empty
/// one (#1352). A missing file is an empty list.
pub fn load_for_update_at(path: &Path) -> Result<Vec<GlobalLibraryEntry>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(path).map_err(|e| {
        format!(
            "{} could not be read ({e}); it was left unchanged",
            path.display()
        )
    })?;
    toml::from_str::<GlobalPrefsFile>(&text)
        .map(|file| file.libraries)
        .map_err(|e| {
            format!(
                "{} could not be parsed ({e}); it was left unchanged. Fix or remove the file, then try again",
                path.display()
            )
        })
}

/// Run `update` while holding an exclusive OS lock on a sidecar file
/// (`<prefs>.lock`) next to the list. The lock spans the caller's
/// load -> modify -> write, so two Signex processes (and two threads) cannot
/// both load the same list and have the last writer drop the other's entry
/// (#1757).
///
/// The lock is on a separate sidecar, not the list: `atomic_write` renames a
/// new file over the list, which would orphan a lock held on the old inode.
/// The sidecar is never replaced or deleted. `File::lock` is advisory
/// `flock` on Unix and `LockFileEx` on Windows; it is released when the
/// handle drops, including if the process crashes.
fn with_list_lock<T>(
    prefs: &Path,
    update: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let mut name = prefs.as_os_str().to_owned();
    name.push(".lock");
    let lock_path = PathBuf::from(name);
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|e| format!("open {}: {e}", lock_path.display()))?;
    lock.lock()
        .map_err(|e| format!("lock {}: {e}", lock_path.display()))?;
    update()
}

/// Append a path to the global list and persist. Idempotent — already-
/// present paths are skipped. Returns the resulting full list.
pub fn add_path(path: PathBuf) -> Result<Vec<GlobalLibraryEntry>, String> {
    let prefs = prefs_path().ok_or_else(|| "no user config dir available".to_string())?;
    add_path_at(&prefs, path)
}

/// [`add_path`] against an explicit prefs file.
pub fn add_path_at(prefs: &Path, path: PathBuf) -> Result<Vec<GlobalLibraryEntry>, String> {
    with_list_lock(prefs, || {
        let mut current = load_for_update_at(prefs)?;
        if !current.iter().any(|e| e.path == path) {
            current.push(GlobalLibraryEntry {
                path,
                remote: None,
                auto_pull: None,
            });
            save_at(prefs, &current)?;
        }
        Ok(current)
    })
}

/// Add `path` to an in-memory list unless it is already there (same
/// exact-path identity as [`add_path_at`]). For a handler whose list may
/// have changed since a worker persisted the entry.
pub fn merge_path(list: &mut Vec<GlobalLibraryEntry>, path: PathBuf) {
    if !list.iter().any(|e| e.path == path) {
        list.push(GlobalLibraryEntry {
            path,
            remote: None,
            auto_pull: None,
        });
    }
}

/// Remove a path from the global list and persist.
pub fn remove_path(path: &Path) -> Result<Vec<GlobalLibraryEntry>, String> {
    let prefs = prefs_path().ok_or_else(|| "no user config dir available".to_string())?;
    remove_path_at(&prefs, path)
}

/// [`remove_path`] against an explicit prefs file.
pub fn remove_path_at(prefs: &Path, path: &Path) -> Result<Vec<GlobalLibraryEntry>, String> {
    with_list_lock(prefs, || {
        let mut current = load_for_update_at(prefs)?;
        let before = current.len();
        current.retain(|e| e.path != path);
        if current.len() != before {
            save_at(prefs, &current)?;
        }
        Ok(current)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #1352: an unparseable file must not be replaced by a near-empty list
    /// when the user adds or removes a library.
    #[test]
    fn add_and_remove_leave_an_unparseable_file_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let prefs = tmp.path().join("global_libraries.toml");
        let corrupt = "libraries = [ { path = \"/a.snxlib\" }, oops";
        std::fs::write(&prefs, corrupt).unwrap();

        let err = add_path_at(&prefs, PathBuf::from("/b.snxlib")).unwrap_err();
        assert!(err.contains("left unchanged"), "{err}");
        assert_eq!(std::fs::read_to_string(&prefs).unwrap(), corrupt);

        let err = remove_path_at(&prefs, Path::new("/a.snxlib")).unwrap_err();
        assert!(err.contains("left unchanged"), "{err}");
        assert_eq!(std::fs::read_to_string(&prefs).unwrap(), corrupt);
    }

    #[test]
    fn merge_path_keeps_existing_entries_and_skips_duplicates() {
        let mut list = vec![GlobalLibraryEntry {
            path: PathBuf::from("/a.snxlib"),
            remote: Some("r".into()),
            auto_pull: None,
        }];
        merge_path(&mut list, PathBuf::from("/a.snxlib"));
        merge_path(&mut list, PathBuf::from("/b.snxlib"));
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].remote.as_deref(), Some("r"));
        assert_eq!(list[1].path, PathBuf::from("/b.snxlib"));
    }

    /// #1757: concurrent adds, each with its own lock handle, must all land.
    /// Threads exercise the same OS lock a second process would use.
    #[test]
    fn concurrent_adds_and_removes_never_lose_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let prefs = tmp.path().join("global_libraries.toml");
        let workers: Vec<_> = (0..4)
            .map(|w| {
                let prefs = prefs.clone();
                std::thread::spawn(move || {
                    for i in 0..25 {
                        let keep = PathBuf::from(format!("/w{w}/keep{i}.snxlib"));
                        let temp = PathBuf::from(format!("/w{w}/temp{i}.snxlib"));
                        add_path_at(&prefs, temp.clone()).unwrap();
                        add_path_at(&prefs, keep).unwrap();
                        remove_path_at(&prefs, &temp).unwrap();
                    }
                })
            })
            .collect();
        for w in workers {
            w.join().unwrap();
        }
        let list = load_at(&prefs);
        assert_eq!(list.len(), 100, "entries lost: {}", 100 - list.len());
        assert!(
            list.iter()
                .all(|e| e.path.to_string_lossy().contains("keep"))
        );
    }

    #[test]
    fn add_to_a_missing_file_creates_it() {
        let tmp = tempfile::tempdir().unwrap();
        let prefs = tmp.path().join("global_libraries.toml");
        let list = add_path_at(&prefs, PathBuf::from("/b.snxlib")).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(load_at(&prefs), list);
    }

    #[test]
    fn load_returns_empty_when_file_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("does-not-exist.toml");
        assert_eq!(load_at(&path), Vec::new());
    }

    #[test]
    fn round_trip_serialises_path_only_entry() {
        let entries = vec![GlobalLibraryEntry {
            path: PathBuf::from("/tmp/foo.snxlib"),
            remote: None,
            auto_pull: None,
        }];
        let file = GlobalPrefsFile {
            libraries: entries.clone(),
        };
        let text = toml::to_string_pretty(&file).unwrap();
        let back: GlobalPrefsFile = toml::from_str(&text).unwrap();
        assert_eq!(back.libraries, entries);
    }

    #[test]
    fn round_trip_preserves_remote_and_auto_pull() {
        let entries = vec![GlobalLibraryEntry {
            path: PathBuf::from("/tmp/bar.snxlib"),
            remote: Some("git@github.com:caner/bar.git".to_string()),
            auto_pull: Some(true),
        }];
        let file = GlobalPrefsFile {
            libraries: entries.clone(),
        };
        let text = toml::to_string_pretty(&file).unwrap();
        let back: GlobalPrefsFile = toml::from_str(&text).unwrap();
        assert_eq!(back.libraries, entries);
    }

    /// `save_at` must go through `atomic_write`, not `fs::write`: a failed
    /// save leaves the previously persisted list fully intact.
    ///
    /// Discriminator: denying new-file creation in the destination's
    /// parent directory makes `atomic_write`'s `File::create(&tmp)` fail
    /// before it can touch the destination, regardless of the unique
    /// per-writer temp name it picks (#416). A plain `fs::write` would
    /// ignore that and clobber the old file — so this test fails on a
    /// revert.
    #[test]
    fn save_at_leaves_original_intact_when_write_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("global_libraries.toml");
        let original = vec![GlobalLibraryEntry {
            path: PathBuf::from("/tmp/keep-me.snxlib"),
            remote: None,
            auto_pull: None,
        }];
        save_at(&path, &original).unwrap();
        let before = std::fs::read_to_string(&path).unwrap();
        assert!(!crate::test_support::has_stray_tmp(path.parent().unwrap()));

        let _deny = crate::test_support::DenyNewFiles::on(path.parent().unwrap());
        let replacement = vec![GlobalLibraryEntry {
            path: PathBuf::from("/tmp/clobber.snxlib"),
            remote: None,
            auto_pull: None,
        }];
        assert!(save_at(&path, &replacement).is_err());

        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    }

    #[test]
    fn malformed_toml_returns_empty_via_load_path_logic() {
        // Mirror the path the loader takes when toml::from_str fails.
        let bad = "not a valid [[libraries]] toml";
        let result = toml::from_str::<GlobalPrefsFile>(bad);
        assert!(result.is_err());
    }
}
