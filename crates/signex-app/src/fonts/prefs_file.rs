//! Non-clobbering read-modify-write for `prefs.json` (#594).
//!
//! Every preference writer in this module tree is a read-modify-write:
//! load the whole file, set one key, write the whole file back. The
//! chain that used to do the "load" half discarded both the read error
//! and the parse error with `.ok()` and fell back to an empty JSON
//! object, which collapsed three different states into one: "the file
//! does not exist"
//! (a fresh install, where `{}` is right), "the file could not be read"
//! (a permission or hardware fault) and "the file did not parse" (a
//! truncated write, a hand-edit with a missing brace). In the last two
//! the user *has* preferences on disk, and starting from `{}` meant the
//! very next toggle of any single knob rewrote the file as that one key
//! alone — theme, dock layout, ERC severity overrides, the pin matrix,
//! filter presets and component classes all gone, with no error shown.
//!
//! [`update_prefs_json`] refuses instead: on anything but genuine
//! absence it reports and returns, leaving the file byte-identical so
//! the user still has something to repair.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use super::write_pref_atomic;

/// Why an existing `prefs.json` could not be loaded for an in-place
/// update. Each variant is a state the old chain silently turned into
/// an empty file; `Display` names what the user has to fix.
enum PrefsLoadError {
    Io(std::io::Error),
    Parse(serde_json::Error),
    NotAnObject,
}

impl std::fmt::Display for PrefsLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "could not be read: {e}"),
            Self::Parse(e) => write!(f, "is not valid JSON: {e}"),
            Self::NotAnObject => write!(
                f,
                "has a non-object root (expected a JSON object at the top level)"
            ),
        }
    }
}

/// Load `prefs.json` as the object it is supposed to be, or say why not.
///
/// Only two inputs yield an empty map, and both genuinely carry no user
/// data to protect: the file is absent (fresh install — this is the path
/// that must keep working), or it is empty / all ASCII whitespace, which
/// is what a writer killed between `File::create` and `write_all` leaves
/// behind.
///
/// Returning the `Map` rather than the enclosing `Value` makes "the
/// prefs root is an object" a fact the type carries, which is what lets
/// the `library_browser_searches` writer drop its
/// `.as_object_mut().expect(…)` panic.
fn load_for_update(
    path: &Path,
) -> Result<serde_json::Map<String, serde_json::Value>, PrefsLoadError> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(serde_json::Map::new()),
        Err(e) => return Err(PrefsLoadError::Io(e)),
    };
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(serde_json::Map::new());
    }
    match serde_json::from_slice::<serde_json::Value>(&bytes) {
        Ok(serde_json::Value::Object(map)) => Ok(map),
        Ok(_) => Err(PrefsLoadError::NotAnObject),
        Err(e) => Err(PrefsLoadError::Parse(e)),
    }
}

/// Paths currently in the refused state, so the report below fires on
/// the transition into failure rather than on every attempt.
/// `write_component_filter` runs once per keystroke in the Components
/// panel filter box, so an unconditional `error!` would fill the
/// Messages panel with one copy of the same line per character typed.
fn reported_failures() -> &'static Mutex<HashSet<PathBuf>> {
    static REPORTED: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    REPORTED.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Recover a poisoned latch instead of panicking on it: the set is a
/// plain collection of paths that no unwinding writer can leave
/// half-updated, and losing the UI thread over a de-duplication cache
/// would be a worse failure than the one being reported.
fn latch() -> std::sync::MutexGuard<'static, HashSet<PathBuf>> {
    reported_failures()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Report a refused write once, at `Error` level — the default filter
/// is `LevelFilter::Info` (`crate::diagnostics`), so `debug!` / `warn!`
/// would never reach the user's Messages panel, and this is one step
/// away from losing every preference they have.
fn report_refusal(path: &Path, context: &str, error: &PrefsLoadError) {
    if !latch().insert(path.to_path_buf()) {
        return;
    }
    tracing::error!(
        target = "signex::prefs",
        path = %path.display(),
        context = context,
        error = %error,
        "preferences were not saved: the existing preferences file could not be loaded, \
         and it was left exactly as it is rather than overwritten, so no other setting \
         was lost — repair or delete the file to start saving preferences again"
    );
}

/// Forget a path that loaded cleanly, so a file that is repaired and
/// then broken again reports the second break too.
fn forget_refusal(path: &Path) {
    latch().remove(path);
}

/// Update one key of `prefs.json` at `path` without clobbering the
/// others, or — when the existing file cannot be loaded — without
/// touching the file at all (#594).
///
/// `context` is the preference key being written; it rides through to
/// `write_pref_atomic` and into the refusal report so a failure names
/// which knob the user was turning. Creates the parent dir if missing.
pub(super) fn update_prefs_json(
    path: &Path,
    context: &str,
    mutator: impl FnOnce(&mut serde_json::Map<String, serde_json::Value>),
) {
    let mut prefs = match load_for_update(path) {
        Ok(prefs) => prefs,
        Err(error) => {
            report_refusal(path, context, &error);
            return;
        }
    };
    forget_refusal(path);
    mutator(&mut prefs);
    match serde_json::to_string_pretty(&serde_json::Value::Object(prefs)) {
        Ok(serialized) => write_pref_atomic(path, serialized.as_bytes(), context),
        Err(error) => tracing::error!(
            target = "signex::prefs",
            path = %path.display(),
            context = context,
            error = %error,
            "preferences were not saved: the new value could not be serialised to JSON; \
             the existing file was left unchanged so no other setting was lost"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tempdir plus the `prefs.json` path inside it. Deliberately not
    /// `prefs_path()`: that resolver caches in a process-wide
    /// `OnceLock`, so tests sharing it would race under `cargo test`'s
    /// default parallelism.
    fn temp_prefs() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::Builder::new()
            .prefix("signex-prefs-")
            .tempdir()
            .expect("a temp directory for the prefs file");
        let path = dir.path().join("prefs.json");
        (dir, path)
    }

    fn read_prefs_object(path: &Path) -> serde_json::Map<String, serde_json::Value> {
        let bytes = std::fs::read(path).expect("the prefs file exists");
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).expect("the written prefs file is valid JSON");
        match value {
            serde_json::Value::Object(map) => map,
            other => panic!("expected a JSON object at the prefs root, got {other}"),
        }
    }

    /// The #594 regression: a truncated file (killed mid-write, or
    /// hand-edited with a brace missing) must survive the next write.
    #[test]
    fn malformed_prefs_file_is_left_byte_identical() {
        // Arrange
        let (_dir, path) = temp_prefs();
        std::fs::write(
            &path,
            br#"{"theme": "signex", "erc_severity": {"unused_pin": "off"}"#,
        )
        .expect("seed the malformed prefs file");
        let before = std::fs::read(&path).expect("read the seeded bytes back");

        // Act
        update_prefs_json(&path, "dock", |prefs| {
            prefs.insert("dock".to_string(), serde_json::json!({ "left": [] }));
        });

        // Assert
        let after = std::fs::read(&path).expect("the prefs file still exists");
        assert_eq!(
            before, after,
            "a malformed prefs file must be left byte-identical, not replaced"
        );
    }

    /// A directory standing where the file should be is the portable
    /// stand-in for "readable() fails with something other than
    /// NotFound": `std::fs::read` on a directory fails on every
    /// platform, and — unlike a `chmod 000` file — it still fails when
    /// the suite runs as root.
    #[test]
    fn unreadable_prefs_file_is_left_alone() {
        // Arrange
        let (_dir, path) = temp_prefs();
        std::fs::create_dir(&path).expect("seed a directory where the prefs file belongs");

        // Act
        update_prefs_json(&path, "theme", |prefs| {
            prefs.insert("theme".to_string(), serde_json::json!("signex"));
        });

        // Assert
        assert!(
            path.is_dir(),
            "an unreadable prefs path must be left exactly as it was"
        );
    }

    /// The fresh-install path: absence is the one state that legitimately
    /// starts from an empty object.
    #[test]
    fn absent_prefs_file_is_created() {
        // Arrange
        let (_dir, path) = temp_prefs();
        assert!(!path.exists(), "the tempdir starts without a prefs file");

        // Act
        update_prefs_json(&path, "grid_visible", |prefs| {
            prefs.insert("grid_visible".to_string(), serde_json::Value::Bool(false));
        });

        // Assert
        let prefs = read_prefs_object(&path);
        assert_eq!(prefs.len(), 1, "only the written key should be present");
        assert_eq!(
            prefs.get("grid_visible"),
            Some(&serde_json::Value::Bool(false))
        );
    }

    /// A zero-byte file carries no user data to protect and is what an
    /// interrupted writer leaves behind, so it is treated as absent.
    #[test]
    fn empty_prefs_file_is_treated_as_absent() {
        // Arrange
        let (_dir, path) = temp_prefs();
        std::fs::write(&path, b"").expect("seed a zero-byte prefs file");

        // Act
        update_prefs_json(&path, "theme", |prefs| {
            prefs.insert("theme".to_string(), serde_json::json!("signex"));
        });

        // Assert
        let prefs = read_prefs_object(&path);
        assert_eq!(prefs.get("theme"), Some(&serde_json::json!("signex")));
    }

    /// A non-object root used to reach the `library_browser_searches`
    /// writer's `.as_object_mut().expect(…)` and panic the UI thread.
    #[test]
    fn non_object_root_is_left_alone_and_does_not_panic() {
        // Arrange
        let (_dir, path) = temp_prefs();
        std::fs::write(&path, b"[]").expect("seed a prefs file with an array root");

        // Act
        update_prefs_json(&path, "library_browser_searches", |prefs| {
            let entry = prefs
                .entry("library_browser_searches".to_string())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            if let Some(obj) = entry.as_object_mut() {
                obj.insert(
                    "/libs/parts.snxlib".to_string(),
                    serde_json::Value::String("resistor".to_string()),
                );
            }
        });

        // Assert
        let after = std::fs::read(&path).expect("the prefs file still exists");
        assert_eq!(
            after, b"[]",
            "a non-object prefs root must be left exactly as it was"
        );
    }

    /// The base property the whole module exists for: writing one key
    /// leaves every other key untouched.
    #[test]
    fn existing_keys_survive_an_unrelated_write() {
        // Arrange
        let (_dir, path) = temp_prefs();
        let seeded = serde_json::json!({
            "theme": "signex",
            "component_classes": [{ "key": "resistor", "label": "Resistor" }],
        });
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&seeded).expect("seed serialises"),
        )
        .expect("seed a valid prefs file");

        // Act
        update_prefs_json(&path, "dock", |prefs| {
            prefs.insert("dock".to_string(), serde_json::json!({ "left": [] }));
        });

        // Assert
        let prefs = read_prefs_object(&path);
        assert_eq!(prefs.len(), 3, "the two seeded keys plus the written one");
        assert_eq!(prefs.get("theme"), Some(&seeded["theme"]));
        assert_eq!(
            prefs.get("component_classes"),
            Some(&seeded["component_classes"])
        );
        assert_eq!(prefs.get("dock"), Some(&serde_json::json!({ "left": [] })));
    }
}
