//! #602 — the Preferences prefs-file banner and its recovery action must
//! actually be wired to the handler.
//!
//! `crates/signex-app/src/fonts/prefs_file.rs` owns the filesystem
//! behaviour (rename, `.bak` laddering, the `{}` re-seed, writes resuming)
//! and tests it hermetically against tempdirs. What no test covered
//! anywhere was the wiring: opening Preferences re-probes the file, saving
//! re-probes it again, and `PrefMsg::ResetPrefsFile` reaches the handler at
//! all. The keymap side left the same gap — `keymap/profile_tests.rs`
//! re-enacts its handler by hand rather than driving
//! `app/handlers/preferences.rs`.
//!
//! **Sharing the prefs path, safely.** Integration tests run under the
//! `test-prefs-redirect` feature, so `config_root()` is ONE per-process
//! tempdir shared by every test in this binary, and `prefs_path()` caches
//! it in a `OnceLock`. Two tests here have to seed a broken file at that
//! shared path, because "the flag gets SET" is the direction that can
//! actually fail — a test that only ever sees a healthy file also passes
//! against an unconditional `prefs_load_error = None`, which would delete
//! the whole feature and leave the suite green.
//!
//! That is safe for two reasons, and both were checked rather than
//! assumed:
//!
//! 1. Every test in this module takes [`serial`] first, so no two of them
//!    disagree about what is at the shared path, and [`PrefsPathGuard`]
//!    puts it back on every exit path including a panicking assertion.
//! 2. No other registered test in this binary depends on that file. Every
//!    prefs test uses the `_at(tempdir)` variants
//!    (`tests/regression/prefs.rs`), no test drives `PrefMsg::Save`, and
//!    no test asserts on the shared prefs bytes. A reader that hits the
//!    seeded broken file falls back to exactly the defaults it already
//!    gets from the absent file, so no assertion anywhere changes.

use std::sync::Mutex;

use signex_app::app::{Message, PreferencesMsg, Signex};
use signex_app::preferences::PrefMsg;

fn inner(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

/// Serialises the tests in this module against each other. They share one
/// per-process prefs path and two of them seed a broken file there, so
/// `cargo test`'s default parallelism would otherwise let one test's
/// broken file fail another's healthy-file precondition — and worse, let
/// the recovery action rename the shared file out from under it.
static PREFS_PATH_LOCK: Mutex<()> = Mutex::new(());

/// A poisoned lock is not a reason to fail every remaining test: the guard
/// protects a file path, not an invariant that a panicking test can leave
/// half-applied, and [`PrefsPathGuard`] has already restored the path by
/// the time this lock is released.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    PREFS_PATH_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Snapshots the shared config files these tests can disturb and puts
/// them back when it goes out of scope — on the assertion-failure path
/// too, which is why this is a `Drop` guard and not a tail call at the end
/// of the test body.
///
/// The keymap file is in here because `PrefMsg::Save` persists the keymap
/// working copy as well as the prefs, so the one test that drives a real
/// Save would otherwise leave `keyboard_shortcuts.toml` behind for every
/// later `Signex::new()` in this binary to load.
struct PrefsPathGuard {
    path: std::path::PathBuf,
    before: Option<Vec<u8>>,
    keymap_path: Option<std::path::PathBuf>,
    keymap_before: Option<Vec<u8>>,
}

impl PrefsPathGuard {
    fn capture() -> Self {
        let path = signex_app::fonts::prefs_file_path();
        let before = std::fs::read(&path).ok();
        let keymap_path = signex_app::keymap::config_path();
        let keymap_before = keymap_path.as_ref().and_then(|p| std::fs::read(p).ok());
        Self {
            path,
            before,
            keymap_path,
            keymap_before,
        }
    }

    /// Put malformed JSON at the shared prefs path. `create_dir_all`
    /// first: on a fresh test config root the `signex` directory only
    /// comes into being when a writer creates it.
    fn seed_broken(&self) {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).expect("create the shared config directory");
        }
        std::fs::write(&self.path, br#"{"theme": "signex""#)
            .expect("seed a malformed shared prefs file");
    }
}

/// Put `path` back to `before` — content restored, or removed again when
/// it did not exist. A `NotFound` on the removal is already the state
/// being asked for.
fn restore(path: &std::path::Path, before: Option<&Vec<u8>>) -> std::io::Result<()> {
    match before {
        Some(bytes) => std::fs::write(path, bytes),
        None => match std::fs::remove_file(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        },
    }
}

impl Drop for PrefsPathGuard {
    fn drop(&mut self) {
        let mut failures: Vec<String> = Vec::new();
        if let Err(error) = restore(&self.path, self.before.as_ref()) {
            failures.push(format!(
                "could not restore the shared prefs file at {}: {error}",
                self.path.display()
            ));
        }
        if let Some(keymap_path) = self.keymap_path.as_deref()
            && let Err(error) = restore(keymap_path, self.keymap_before.as_ref())
        {
            failures.push(format!(
                "could not restore the shared keymap file at {}: {error}",
                keymap_path.display()
            ));
        }
        if failures.is_empty() {
            return;
        }
        let message = failures.join("; ");
        // Panicking while already unwinding aborts the whole test binary
        // and hides the assertion that actually failed, so only escalate
        // when this drop is not itself part of a failure.
        if std::thread::panicking() {
            eprintln!("{message}");
        } else {
            panic!("{message}");
        }
    }
}

/// The guard is the whole reason the recovery action is safe to run beside
/// the other tests, and it is also the behaviour that protects a user who
/// repaired the file by hand between the banner being painted and the
/// click. A refactor that drops it renames a healthy prefs.json away —
/// #594 again.
#[test]
fn resetting_a_healthy_prefs_file_moves_nothing_and_says_so() {
    // Arrange
    let _serial = serial();
    let guard = PrefsPathGuard::capture();
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let before = std::fs::read(&guard.path).ok();
    assert!(
        app.ui_state.prefs_load_error.is_none(),
        "fixture precondition: the shared test prefs file is healthy or absent, got {:?}",
        app.ui_state.prefs_load_error
    );

    // Act
    let _ = app.update(inner(PrefMsg::ResetPrefsFile));

    // Assert
    assert_eq!(
        app.ui_state.preferences_prefs_status,
        "Your preferences file is readable again — nothing was moved.",
        "a healthy file must be reported as healthy, not reset"
    );
    assert_eq!(
        std::fs::read(&guard.path).ok(),
        before,
        "a healthy prefs file must be left exactly as it was"
    );
    assert!(
        !guard.path.with_file_name("prefs.json.bak").exists(),
        "nothing may be moved aside while the file loads cleanly"
    );
}

/// The banner is state, and the action has to clear it — otherwise a user
/// who resets is left staring at the same warning with no way to dismiss
/// it. Pre-set the flag to a stale value the way a boot-time failure
/// would, then drive the message the button emits.
#[test]
fn resetting_clears_a_stale_load_error_flag() {
    // Arrange
    let _serial = serial();
    let _guard = PrefsPathGuard::capture();
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    app.ui_state.prefs_load_error = Some("is not valid JSON: expected `,` or `}`".to_string());

    // Act
    let _ = app.update(inner(PrefMsg::ResetPrefsFile));

    // Assert
    assert!(
        app.ui_state.prefs_load_error.is_none(),
        "the banner flag must be cleared, got {:?}",
        app.ui_state.prefs_load_error
    );
    assert!(
        !app.ui_state.preferences_prefs_status.is_empty(),
        "the user must be told what the action did"
    );
}

/// Opening Preferences re-probes the file rather than trusting the boot
/// snapshot, so a hand repair is picked up without a restart.
///
/// Both directions, because only one of them can fail. Asserting that a
/// stale flag is cleared also passes against a handler changed to an
/// unconditional `prefs_load_error = None` — which deletes the feature and
/// keeps the suite green. Seeding a genuinely broken file and demanding
/// the flag be SET, naming the parse failure, is the half with teeth.
#[test]
fn opening_preferences_reprobes_the_prefs_file() {
    // Arrange
    let _serial = serial();
    let guard = PrefsPathGuard::capture();
    guard.seed_broken();
    let (mut app, _t) = Signex::new();

    // Act
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));

    // Assert
    let reported = app
        .ui_state
        .prefs_load_error
        .clone()
        .expect("a malformed prefs file must raise the flag when the dialog opens");
    assert!(
        reported.starts_with("is not valid JSON:"),
        "the banner must name the parse failure, got {reported}"
    );

    // Act — the other direction: repair the file (here, remove it, which
    // is the fresh-install state the writer starts from) and reopen.
    std::fs::remove_file(&guard.path).expect("remove the seeded malformed prefs file");
    let _ = app.update(Message::Preferences(PreferencesMsg::Close));
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));

    // Assert
    assert!(
        app.ui_state.prefs_load_error.is_none(),
        "a repaired file must clear the flag on the next open, got {:?}",
        app.ui_state.prefs_load_error
    );
}

/// A file that breaks *while* Preferences is open has no other route to
/// the banner: `handle_preferences_open_requested` early-returns when the
/// dialog is already up, so without the probe at the end of the Save arm
/// every write is refused while the dialog reports success — the exact
/// invisible-failure state #602 exists to remove.
#[test]
fn saving_reprobes_a_file_that_broke_while_the_dialog_was_open() {
    // Arrange
    let _serial = serial();
    let guard = PrefsPathGuard::capture();
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    assert!(
        app.ui_state.prefs_load_error.is_none(),
        "fixture precondition: the file is healthy when the dialog opens, got {:?}",
        app.ui_state.prefs_load_error
    );

    // Act — hand-edit the file into a syntax error beside the running app,
    // the workflow the banner's own "repair it by hand" advice invites,
    // then save without ever closing the dialog.
    guard.seed_broken();
    let _ = app.update(inner(PrefMsg::Save));

    // Assert
    let reported = app
        .ui_state
        .prefs_load_error
        .clone()
        .expect("a save whose prefs writes were refused must raise the banner");
    assert!(
        reported.starts_with("is not valid JSON:"),
        "the banner must name the parse failure, got {reported}"
    );
}

/// The status line is feedback about one action, not an edit — reporting
/// it must never trip the unsaved-changes guard, or the user is trapped in
/// a dialog that refuses to close. Same rule as the export status lines.
#[test]
fn reporting_the_reset_outcome_does_not_mark_the_dialog_dirty() {
    // Arrange
    let _serial = serial();
    let _guard = PrefsPathGuard::capture();
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    assert!(
        !app.ui_state.preferences_dirty,
        "fixture precondition: a freshly opened dialog is clean"
    );

    // Act
    let _ = app.update(inner(PrefMsg::ResetPrefsFile));

    // Assert
    // The status line comes first on purpose: `preferences_dirty` was
    // already `false`, so asserting only that it stayed `false` passes
    // against an empty `PrefMsg::ResetPrefsFile` arm. Demanding the
    // action reported something makes a no-op arm fail here instead —
    // the same guard the sibling reopening test uses.
    assert!(
        !app.ui_state.preferences_prefs_status.is_empty(),
        "precondition for the real assertion: the action leaves a status line"
    );
    assert!(
        !app.ui_state.preferences_dirty,
        "a recovery status line is not an unsaved change"
    );
}

/// Reopening the dialog is a fresh session for transient feedback: the
/// last reset's result must not still be sitting in the banner.
#[test]
fn reopening_preferences_clears_the_reset_status() {
    // Arrange
    let _serial = serial();
    let _guard = PrefsPathGuard::capture();
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let _ = app.update(inner(PrefMsg::ResetPrefsFile));
    assert!(
        !app.ui_state.preferences_prefs_status.is_empty(),
        "fixture precondition: the action leaves a status line"
    );
    let _ = app.update(Message::Preferences(PreferencesMsg::Close));

    // Act
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));

    // Assert
    assert!(
        app.ui_state.preferences_prefs_status.is_empty(),
        "a reopened dialog must not show the last session's result, got {:?}",
        app.ui_state.preferences_prefs_status
    );
}
