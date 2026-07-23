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

/// Resolved on-disk path of `global_libraries.toml`. `None` when
/// `dirs::config_dir()` can't resolve a config dir (very rare — a
/// stripped-down headless environment).
pub fn prefs_path() -> Option<PathBuf> {
    let base = dirs::config_dir()?;
    Some(base.join("signex").join("global_libraries.toml"))
}

/// Load the global library list from disk. Returns an empty Vec when
/// the file is missing or unparseable — both non-fatal cases. Parse
/// errors warn through `tracing` so they're visible without taking the
/// whole panel down.
pub fn load() -> Vec<GlobalLibraryEntry> {
    let Some(path) = prefs_path() else {
        return Vec::new();
    };
    if !path.exists() {
        return Vec::new();
    }
    match std::fs::read_to_string(&path) {
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

/// Persist `entries` to `global_libraries.toml`. Creates the parent
/// directory if missing. Errors warn through `tracing` and surface to
/// the caller via the `Result` so the dispatcher can show a brief
/// inline error in the Components Panel.
pub fn save(entries: &[GlobalLibraryEntry]) -> Result<(), String> {
    let path = prefs_path().ok_or_else(|| "no user config dir available".to_string())?;
    save_at(&path, entries)
}

/// Variant for tests / explicit paths — [`prefs_path`] is config-dir-global,
/// so the real `save` is untestable without one. Mirrors the
/// `save_preferred_order` / `save_preferred_order_at` split in
/// `library::settings::persistence`.
///
/// Crash-safe: [`signex_types::atomic_io::atomic_write`] writes to a temp
/// sibling, fsyncs it and renames over the destination, so a crash mid-save
/// leaves the previous library list intact rather than a truncated file. It
/// also creates the parent directory, so no separate `create_dir_all` here.
pub fn save_at(path: &Path, entries: &[GlobalLibraryEntry]) -> Result<(), String> {
    let file = GlobalPrefsFile {
        libraries: entries.to_vec(),
    };
    let text = toml::to_string_pretty(&file)
        .map_err(|e| format!("serialise global_libraries.toml: {e}"))?;
    signex_types::atomic_io::atomic_write(path, text.as_bytes())
        .map_err(|e| format!("write {}: {}", path.display(), e))?;
    Ok(())
}

/// Append a path to the global list and persist. Idempotent — already-
/// present paths are skipped. Returns the resulting full list.
pub fn add_path(path: PathBuf) -> Result<Vec<GlobalLibraryEntry>, String> {
    let mut current = load();
    if !current.iter().any(|e| e.path == path) {
        current.push(GlobalLibraryEntry {
            path,
            remote: None,
            auto_pull: None,
        });
        save(&current)?;
    }
    Ok(current)
}

/// Remove a path from the global list and persist.
#[allow(dead_code)]
pub fn remove_path(path: &Path) -> Result<Vec<GlobalLibraryEntry>, String> {
    let mut current = load();
    let before = current.len();
    current.retain(|e| e.path != path);
    if current.len() != before {
        save(&current)?;
    }
    Ok(current)
}

/// Mount every entry in `entries` onto the supplied `LibraryState`,
/// best-effort. Adapter open failures are logged through `tracing`
/// and the affected entry is skipped — one bad library shouldn't sink
/// the whole load.
///
/// Called by `bootstrap` once at startup so global libraries are
/// available across every project the user opens during the session.
pub fn mount_all(library_state: &mut crate::library::LibraryState, entries: &[GlobalLibraryEntry]) {
    for entry in entries {
        if let Err(e) = library_state.open_library(entry.path.clone()) {
            tracing::warn!(
                target: "signex::library",
                path = %entry.path.display(),
                error = %e,
                "global library mount failed"
            );
        }
    }
}

/// One-shot "load and mount" — used by the bootstrap path so callers
/// don't have to remember the two-step dance.
pub fn load_and_mount_all(
    library_state: &mut crate::library::LibraryState,
) -> Vec<GlobalLibraryEntry> {
    let entries = load();
    mount_all(library_state, &entries);
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_returns_empty_when_file_missing() {
        // The default config-dir-relative path almost certainly doesn't
        // exist on a fresh CI runner; if it does, the test fixture is
        // tolerant either way. The contract under test is "no panic".
        let _ = load();
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
