//! The `prefs.json` file's health, its non-clobbering read-modify-write,
//! and the one recovery action that gets a broken file out of the way
//! (#594, #602).
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
//!
//! Refusing is only half an answer, though. Every *reader* in this module
//! tree swallows the same failure with `.ok()` and hands back its default,
//! so a broken file makes the whole UI look factory-reset while nothing
//! the user changes sticks — and the only report is a `tracing::error!`
//! in a dock panel they may never open. [`check_prefs_file`] is the
//! health probe the UI asks at boot and on every Preferences open, and
//! [`move_prefs_file_aside`] is the one in-app repair: rename the broken
//! file to a free `.bak` slot and drop a fresh `{}` in its place, so the
//! next [`update_prefs_json`] has a loadable file to build on and the
//! original is still on disk beside it (#602). The empty object rather
//! than an empty slot is deliberate — [`super::migrate_legacy_prefs`]
//! copies a legacy file forward on exactly `!canonical.exists()`, and
//! would otherwise undo the reset on the next launch.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use super::write_pref_atomic;

/// Why an existing `prefs.json` could not be loaded for an in-place
/// update. Each variant is a state the old chain silently turned into
/// an empty file; `Display` names what the user has to fix.
///
/// The `Display` strings are deliberately sentence *fragments* that
/// follow a path — "…/prefs.json is not valid JSON: …" — because both
/// consumers put the path in front of them: the refusal report below,
/// and the Preferences banner (#602).
#[derive(Debug)]
pub enum PrefsLoadError {
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

/// Hand-rolled rather than derived: `thiserror` is not a dependency of
/// this crate, and `keymap::ProfileLoadError` (`keymap/profile.rs`) is
/// exactly this bare impl for the same reason.
///
/// No `source()`, deliberately — do not "restore" one. `Display` above
/// already renders the underlying `io::Error` / `serde_json::Error`
/// inline, because both consumers put that whole string in front of a
/// user (the Preferences banner and the write-refusal report) and a
/// diagnostic that drops the errno is not worth showing. Carrying the
/// same cause in `source()` as well is the mutually-exclusive half of
/// that convention: every chain walker — `anyhow`'s `{:#}`, any standard
/// chain render — would then print it twice, as
/// `could not be read: Permission denied (os error 13): Permission denied
/// (os error 13)`.
impl std::error::Error for PrefsLoadError {}

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
        target: "signex::prefs",
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
            target: "signex::prefs",
            path = %path.display(),
            context = context,
            error = %error,
            "preferences were not saved: the new value could not be serialised to JSON; \
             the existing file was left unchanged so no other setting was lost"
        ),
    }
}

/// The resolved `prefs.json` path, for UI that has to *name* the file.
///
/// [`super::prefs_path`] is private to `fonts` and this is its child
/// module, so this is the only way the Preferences banner can name the
/// real file rather than re-deriving a path that could disagree with the
/// one actually written (the no-config-dir fallback is a per-process
/// temp directory, which nothing outside this module can reconstruct).
pub fn prefs_file_path() -> PathBuf {
    super::prefs_path()
}

/// Is the `prefs.json` at `path` in a state a write can build on?
///
/// Pure and read-only — no logging, no latch, no file creation. It is
/// [`load_for_update`] with the loaded map thrown away, deliberately:
/// one source of truth means the banner can never disagree with what the
/// next [`update_prefs_json`] will actually do. `Ok(())` therefore covers
/// genuine absence and an all-whitespace file as well as a valid object,
/// because those are exactly the inputs a write starts fresh from.
///
/// It must NOT touch the refusal latch. The boot report and the
/// write-refusal report say different things — one "could not be
/// loaded", one "were not saved" — and both are true; keeping them
/// independent keeps a probe from suppressing a refusal the user has not
/// been shown yet.
pub fn check_prefs_file_at(path: &Path) -> Result<(), PrefsLoadError> {
    load_for_update(path).map(drop)
}

/// [`check_prefs_file_at`] against the resolved user prefs path,
/// mirroring the `x()` / `x_at()` pairing every other writer here uses.
pub fn check_prefs_file() -> Result<(), PrefsLoadError> {
    check_prefs_file_at(&prefs_file_path())
}

/// How many `.bak` slots to try before giving up. A user who resets a
/// hundred times has a problem no rename will fix, and an unbounded
/// search would spin the UI thread on a directory that refuses renames.
const MAX_ASIDE_SLOTS: u32 = 100;

/// First free sibling name to move a broken prefs file to.
///
/// Appended to the WHOLE file name, so `prefs.json` moves to
/// `prefs.json.bak` rather than the `prefs.bak` that `set_extension`
/// would produce — and built through [`std::ffi::OsString`] so a
/// non-UTF-8 config directory survives instead of being mangled by a
/// lossy round-trip. Same reasoning as `keymap::profile::backup_path_for`.
///
/// Unlike that one, an existing `.bak` does not stop the search: it
/// ladders to `prefs.json.bak.2`, `.3`, and so on. The keymap backup can
/// stop, because there the `.bak` IS the original and the save that
/// follows overwrites in place. Here the whole point is to get the
/// broken file off `path`, and stopping would trap a user who has to
/// reset a second time.
///
/// A slot is free only when `symlink_metadata` answers `NotFound`. An
/// error that is anything else is not "occupied", it is "cannot tell",
/// and it aborts the search carrying that errno rather than laddering
/// past it. Both halves of that matter:
///
/// - Treating "cannot tell" as free is what `Path::exists()` does — it is
///   `metadata().is_ok()` and answers `false` for a name it merely cannot
///   stat, which would hand [`std::fs::rename`] a destination it then
///   silently replaces. POSIX `rename` overwrites, so the file that
///   vanished would be the user's earlier backup.
/// - Treating it as occupied is safe but dishonest. On a config directory
///   at mode 0000 every one of the [`MAX_ASIDE_SLOTS`] candidates fails
///   the same way, and the caller would report "all 100 .bak slots are
///   already taken" for what is really `Permission denied (os error 13)`
///   — a false cause, on the one screen whose job is to tell the user
///   what is actually wrong with their file.
///
/// `symlink_metadata` and not `metadata`, so a dangling symlink counts as
/// occupied: `rename` operates on the link itself, and `metadata` would
/// follow it to a missing target and call the slot free.
fn aside_path_for(path: &Path) -> Result<PathBuf, std::io::Error> {
    let Some(file_name) = path.file_name() else {
        return Err(std::io::Error::other(format!(
            "no name to move {} aside to: the path has no file name",
            path.display()
        )));
    };
    let mut base = file_name.to_os_string();
    base.push(".bak");
    for slot in 1..=MAX_ASIDE_SLOTS {
        let mut name = base.clone();
        // Slot 1 is the unsuffixed `prefs.json.bak`; the ladder only
        // starts numbering at `.2`, so the loop covers exactly
        // MAX_ASIDE_SLOTS names and the exhaustion message stays true.
        if slot > 1 {
            name.push(format!(".{slot}"));
        }
        let candidate = path.with_file_name(name);
        match std::fs::symlink_metadata(&candidate) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(candidate),
            Err(error) => return Err(error),
            Ok(_) => {}
        }
    }
    Err(std::io::Error::other(format!(
        "no free name to move {} aside to: all {MAX_ASIDE_SLOTS} .bak slots \
         beside it are already taken",
        path.display()
    )))
}

/// Move the `prefs.json` at `path` aside so the next write starts a
/// fresh one, keeping the old file on disk (#602).
///
/// A rename and not a copy: [`update_prefs_json`] refuses on anything but
/// genuine absence, so the broken file has to stop being at `path` for
/// writes to resume at all. Nothing is deleted — the bytes the user may
/// still want to repair by hand land at the returned path.
///
/// `path` is then re-seeded with `{}` rather than left absent, so
/// `super::migrate_legacy_prefs` cannot copy a stale legacy file back over
/// the fresh start on the next launch. `{}` is indistinguishable from
/// absence to every reader here.
///
/// `Ok(None)` means there was no file to move; that is not a failure, and
/// writes were never blocked in the first place. `Err` means the file is
/// still exactly where it was, so the caller must keep reporting it
/// broken rather than telling the user they have a fresh start.
///
/// # Precondition
///
/// Only call this for a path [`check_prefs_file_at`] has just reported as
/// unusable. The two deliberately answer different questions and can
/// disagree: `check_prefs_file_at` reads through a symlink, so a
/// `prefs.json` pointing at a detached volume reads as `NotFound` and is
/// called healthy, while `rename` here operates on the link itself and
/// would move it aside and seed `{}` over it — orphaning the real file
/// behind the `.bak` link once the volume returns. The single caller
/// (`PrefMsg::ResetPrefsFile`) re-checks immediately before calling, which
/// is what keeps that unreachable; keep that guard if you add a caller.
pub fn move_prefs_file_aside_at(path: &Path) -> Result<Option<PathBuf>, std::io::Error> {
    // Carries the real errno when a candidate cannot be stat'ed, rather
    // than flattening every failure into one "the slots are taken"
    // sentence — see [`aside_path_for`].
    let aside = aside_path_for(path)?;
    // Let the rename itself answer whether there was a file, instead of
    // asking `Path::exists()` first. `exists()` is `metadata().is_ok()`
    // and swallows every errno, so it says `false` for a file that is
    // merely unreadable — while `load_for_update` treats only `NotFound`
    // as healthy. The two disagreeing is what let a user with a
    // permission fault be told "there was no preferences file to move
    // aside", watch the banner clear, and keep every write refused for
    // the rest of the session. One syscall also closes the TOCTOU window
    // between the check and the rename.
    match std::fs::rename(path, &aside) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    }
    // The rename landed, so seed a fresh empty object rather than leaving
    // `path` absent. `migrate_legacy_prefs` fires its legacy-copy branch
    // on exactly `!canonical.exists()`, and `prefs_path()` runs it inside
    // a `OnceLock::get_or_init` on the first prefs touch of every launch —
    // so on macOS and Windows, where the legacy path differs from the
    // canonical one, an empty slot means the next launch copies the old
    // file back over the fresh start. If the legacy file is the broken
    // one, the user loops (banner, reset, restart, banner) and burns a
    // `.bak` slot per cycle. `{}` is exactly what `load_for_update`
    // already reads as an empty map, so every reader, `update_prefs_json`
    // and `check_prefs_file` behave as they do for an absent file.
    if let Err(error) = signex_types::atomic_io::atomic_write(path, b"{}") {
        // NOT an `Err` return. The rename has already happened, so the
        // caller's failure wording ("it was left untouched") would be a
        // lie and the banner would stay up over a file that really did
        // move. Report at `Error` level and carry on — `write_pref_atomic`
        // reports at `debug!`, which the default `LevelFilter::Info`
        // swallows before it can reach the Messages panel.
        tracing::error!(
            target: "signex::prefs",
            path = %path.display(),
            error = %error,
            "the unreadable preferences file was moved aside, but a fresh empty one could \
             not be written in its place; a stale legacy preferences file may be restored \
             over it on the next launch"
        );
    }
    // Whatever now sits at `path` is a NEW file. If it breaks later that
    // is a failure the user has not been told about, and the
    // de-duplication latch would otherwise swallow the report as a repeat
    // of the one just fixed.
    forget_refusal(path);
    Ok(Some(aside))
}

/// [`move_prefs_file_aside_at`] against the resolved user prefs path.
pub fn move_prefs_file_aside() -> Result<Option<PathBuf>, std::io::Error> {
    move_prefs_file_aside_at(&prefs_file_path())
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

    // ── The health probe (#602) ──

    /// Absence is the fresh-install path the writer starts from, so the
    /// probe must not raise a banner over it.
    #[test]
    fn checking_an_absent_file_reports_it_as_healthy() {
        // Arrange
        let (_dir, path) = temp_prefs();

        // Act
        let result = check_prefs_file_at(&path);

        // Assert
        assert!(
            result.is_ok(),
            "an absent prefs file is a fresh install, not a fault: {:?}",
            result.err().map(|error| error.to_string())
        );
    }

    /// Same rule as [`empty_prefs_file_is_treated_as_absent`]: a writer
    /// killed between `File::create` and `write_all` leaves a zero-byte
    /// file that carries no user data, and the next write recreates it.
    #[test]
    fn checking_an_empty_file_reports_it_as_healthy() {
        // Arrange
        let (_dir, path) = temp_prefs();
        std::fs::write(&path, b"   \n\t ").expect("seed an all-whitespace prefs file");

        // Act
        let result = check_prefs_file_at(&path);

        // Assert
        assert!(
            result.is_ok(),
            "an all-whitespace prefs file is what the writer treats as absent: {:?}",
            result.err().map(|error| error.to_string())
        );
    }

    #[test]
    fn checking_a_valid_object_reports_it_as_healthy() {
        // Arrange
        let (_dir, path) = temp_prefs();
        std::fs::write(&path, br#"{"theme": "signex"}"#).expect("seed a valid prefs file");

        // Act
        let result = check_prefs_file_at(&path);

        // Assert
        assert!(
            result.is_ok(),
            "a valid object root is the healthy case: {:?}",
            result.err().map(|error| error.to_string())
        );
    }

    #[test]
    fn checking_a_malformed_file_reports_a_parse_error() {
        // Arrange
        let (_dir, path) = temp_prefs();
        std::fs::write(&path, br#"{"theme": "signex""#).expect("seed a truncated prefs file");

        // Act
        let result = check_prefs_file_at(&path);

        // Assert
        let error = result.expect_err("a truncated prefs file must not report as healthy");
        assert!(
            matches!(error, PrefsLoadError::Parse(_)),
            "expected a parse error, got {error}"
        );
        assert!(
            error.to_string().starts_with("is not valid JSON:"),
            "the message is a fragment that follows a path, got {error}"
        );
    }

    #[test]
    fn checking_a_non_object_root_reports_it() {
        // Arrange
        let (_dir, path) = temp_prefs();
        std::fs::write(&path, b"[]").expect("seed a prefs file with an array root");

        // Act
        let result = check_prefs_file_at(&path);

        // Assert
        let error = result.expect_err("an array root must not report as healthy");
        assert!(
            matches!(error, PrefsLoadError::NotAnObject),
            "expected the non-object variant, got {error}"
        );
    }

    /// The probe runs on every Preferences open, so it must be inert: it
    /// may not create the file it is asked about, and it may not rewrite
    /// one that is already there.
    #[test]
    fn checking_a_file_never_creates_or_modifies_it() {
        // Arrange
        let (_dir, absent) = temp_prefs();
        let (_seeded_dir, seeded) = temp_prefs();
        std::fs::write(&seeded, br#"{"theme": "signex""#).expect("seed a malformed prefs file");
        let before = std::fs::read(&seeded).expect("read the seeded bytes back");

        // Act
        let absent_result = check_prefs_file_at(&absent);
        let seeded_result = check_prefs_file_at(&seeded);

        // Assert
        assert!(absent_result.is_ok(), "an absent file probes as healthy");
        assert!(
            !absent.exists(),
            "probing an absent prefs file must not create it"
        );
        assert!(seeded_result.is_err(), "a malformed file probes as broken");
        let after = std::fs::read(&seeded).expect("the seeded prefs file still exists");
        assert_eq!(
            before, after,
            "probing must leave the prefs file byte-identical"
        );
    }

    // ── The recovery (#602) ──

    /// The invariant is not "the path is empty" — that was the shape, and
    /// leaving it empty is what let the legacy migration undo the reset.
    /// It is that the broken bytes are off the prefs path and whatever
    /// replaces them loads clean, which is what makes the next write land.
    #[test]
    fn moving_aside_leaves_the_prefs_path_loadable_again() {
        // Arrange
        let (_dir, path) = temp_prefs();
        std::fs::write(&path, br#"{"theme": "signex""#).expect("seed a malformed prefs file");

        // Act
        let aside = move_prefs_file_aside_at(&path).expect("the rename succeeds");

        // Assert
        let aside = aside.expect("an existing file reports where it went");
        assert_ne!(
            std::fs::read(&path).ok().as_deref(),
            Some(br#"{"theme": "signex""#.as_slice()),
            "the broken bytes must stop being at the prefs path, or writes stay refused"
        );
        assert!(
            check_prefs_file_at(&path).is_ok(),
            "what is left at the prefs path must load clean: {:?}",
            check_prefs_file_at(&path).err().map(|e| e.to_string())
        );
        assert_eq!(
            std::fs::read(&aside).expect("the moved-aside file exists"),
            br#"{"theme": "signex""#,
            "the original bytes must survive the move for a hand repair"
        );
    }

    /// Leaving the prefs path absent handed the reset straight back to
    /// `migrate_legacy_prefs`, whose F1 branch fires on exactly
    /// `!canonical.exists()` and runs on the first prefs touch of every
    /// launch. On macOS and Windows the legacy path differs from the
    /// canonical one, so the broken file the user just moved aside would
    /// be copied back over their fresh start — banner, reset, restart,
    /// banner, one `.bak` slot burnt per cycle.
    #[test]
    fn moving_aside_is_not_undone_by_the_legacy_prefs_migration() {
        // Arrange
        let (_dir, canonical) = temp_prefs();
        let (_legacy_dir, legacy) = temp_prefs();
        std::fs::write(&canonical, br#"{"theme": "canonical-and-broken""#)
            .expect("seed a malformed canonical prefs file");
        std::fs::write(&legacy, br#"{"theme": "legacy"}"#).expect("seed a legacy prefs file");

        // Act
        move_prefs_file_aside_at(&canonical)
            .expect("the rename succeeds")
            .expect("an existing file reports where it went");
        super::super::migrate_legacy_prefs(&canonical, &legacy);

        // Assert
        let after = std::fs::read(&canonical).expect("the canonical path is occupied");
        assert!(
            !String::from_utf8_lossy(&after).contains("legacy"),
            "the legacy file must not be copied back over the reset, got {}",
            String::from_utf8_lossy(&after)
        );
        assert!(
            check_prefs_file_at(&canonical).is_ok(),
            "the reset file must still load clean after the migration runs"
        );
    }

    /// `Path::exists()` swallows every errno, so it reports `false` for a
    /// file it merely cannot stat — and the caller then tells the user
    /// "there was no preferences file to move aside", clears the banner,
    /// and leaves every write refused for the session. A path that cannot
    /// be reached is a failure, never an absence. A regular file standing
    /// in for a directory component is the portable way to force that:
    /// the syscalls come back `ENOTDIR`, not `ENOENT`.
    #[test]
    fn moving_aside_reports_a_failure_it_cannot_tell_apart_from_absence() {
        // Arrange
        let (dir, _unused) = temp_prefs();
        let blocking_file = dir.path().join("signex");
        std::fs::write(&blocking_file, b"not a directory").expect("seed a blocking regular file");
        let path = blocking_file.join("prefs.json");

        // Act
        let result = move_prefs_file_aside_at(&path);

        // Assert
        assert!(
            result.is_err(),
            "an unreachable prefs path must report a failure, not `Ok(None)`, got {:?}",
            result.map(|aside| aside.map(|p| p.display().to_string()))
        );
        assert_eq!(
            std::fs::read(&blocking_file).expect("the blocking file is untouched"),
            b"not a directory",
            "a failed move must change nothing on disk"
        );
        // The reason has to survive too. Treating "cannot stat" as
        // "occupied" and laddering past it would reach the end of the
        // slot list and report "all 100 .bak slots are already taken" —
        // a cause the user cannot act on, and a false one, on the single
        // screen whose job is to say what is wrong with their file.
        let message = result.expect_err("the move failed").to_string();
        assert!(
            !message.contains("slots"),
            "the real errno must reach the user, not an exhausted-slot story: {message}"
        );
    }

    /// `set_extension` would turn `prefs.json` into `prefs.bak` and lose
    /// which file it came from; the suffix goes on the whole file name.
    #[test]
    fn moving_aside_appends_bak_to_the_whole_file_name() {
        // Arrange
        let (dir, path) = temp_prefs();
        std::fs::write(&path, b"[]").expect("seed a prefs file with an array root");
        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some("prefs.json")
        );

        // Act
        let aside = move_prefs_file_aside_at(&path)
            .expect("the rename succeeds")
            .expect("an existing file reports where it went");

        // Assert
        assert_eq!(
            aside.file_name().and_then(|name| name.to_str()),
            Some("prefs.json.bak"),
            "expected the suffix on the whole file name, got {}",
            aside.display()
        );
        assert_eq!(
            aside.parent(),
            Some(dir.path()),
            "the file must stay beside the original, got {}",
            aside.display()
        );
    }

    /// The keymap backup returns `Ok(None)` when a `.bak` exists, which
    /// is right there — that backup IS the original. Here it would trap
    /// the user on their second reset, so the search ladders instead.
    #[test]
    fn moving_aside_never_overwrites_an_existing_backup() {
        // Arrange
        let (_dir, path) = temp_prefs();
        let existing_bak = path.with_file_name("prefs.json.bak");
        std::fs::write(&existing_bak, b"the first broken file").expect("seed an existing backup");
        std::fs::write(&path, b"the second broken file").expect("seed a malformed prefs file");

        // Act
        let aside = move_prefs_file_aside_at(&path)
            .expect("the rename succeeds")
            .expect("an existing file reports where it went");

        // Assert
        assert_eq!(
            std::fs::read(&existing_bak).expect("the first backup still exists"),
            b"the first broken file",
            "an existing backup must never be overwritten"
        );
        assert_eq!(
            aside.file_name().and_then(|name| name.to_str()),
            Some("prefs.json.bak.2"),
            "the search must ladder to a free slot, got {}",
            aside.display()
        );
        assert_eq!(
            std::fs::read(&aside).expect("the second backup exists"),
            b"the second broken file"
        );
    }

    #[test]
    fn moving_aside_is_a_no_op_when_there_is_no_file() {
        // Arrange
        let (dir, path) = temp_prefs();
        assert!(!path.exists(), "the tempdir starts without a prefs file");

        // Act
        let aside = move_prefs_file_aside_at(&path).expect("an absent file is not a failure");

        // Assert
        assert!(aside.is_none(), "there was nothing to move aside");
        assert!(
            !path.with_file_name("prefs.json.bak").exists(),
            "no stray backup may be created for a file that was never there"
        );
        assert_eq!(
            std::fs::read_dir(dir.path())
                .expect("the tempdir is readable")
                .count(),
            0,
            "the directory must be left exactly as it was"
        );
    }

    /// The end-to-end shape of #602: a refused write, the recovery, and
    /// a write that lands again — with the user's original still on disk.
    #[test]
    fn writes_resume_after_the_broken_file_is_moved_aside() {
        // Arrange
        let (_dir, path) = temp_prefs();
        std::fs::write(&path, br#"{"theme": "signex", "dock": {"left": []}"#)
            .expect("seed a malformed prefs file");
        let before = std::fs::read(&path).expect("read the seeded bytes back");
        update_prefs_json(&path, "theme", |prefs| {
            prefs.insert("theme".to_string(), serde_json::json!("dark"));
        });
        assert_eq!(
            std::fs::read(&path).expect("the prefs file still exists"),
            before,
            "precondition: the write is refused while the file is broken"
        );

        // Act
        let aside = move_prefs_file_aside_at(&path)
            .expect("the rename succeeds")
            .expect("an existing file reports where it went");
        update_prefs_json(&path, "theme", |prefs| {
            prefs.insert("theme".to_string(), serde_json::json!("dark"));
        });

        // Assert
        let prefs = read_prefs_object(&path);
        assert_eq!(
            prefs.get("theme"),
            Some(&serde_json::json!("dark")),
            "the next write must land in a fresh file"
        );
        assert_eq!(
            std::fs::read(&aside).expect("the moved-aside file exists"),
            before,
            "the user's original bytes must survive untouched beside the fresh file"
        );
    }
}
