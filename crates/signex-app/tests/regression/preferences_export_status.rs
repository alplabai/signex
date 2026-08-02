//! #533 Class C — a failed export write must reach the user.
//!
//! Both Preferences exports (Appearance ▸ Export Theme, Keyboard
//! Shortcuts ▸ Export Profile) wrote to a file the user had just picked
//! in a save dialog and then discarded the `io::Result` with
//! `let _ = f.write(...).await`. A read-only volume, a full disk, or a
//! path that vanished between the pick and the write all produced the
//! exact same silent `Message::Noop` as a successful export, so the
//! user was left believing a file had been written that had not.
//!
//! The `rfd::AsyncFileDialog` pick itself still needs a human — these
//! tests drive the completion messages the async task now emits, which
//! is where the reporting lives.

use signex_app::app::{Message, PreferencesMsg, Signex};
use signex_app::preferences::PrefMsg;
use std::path::PathBuf;

fn inner(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

#[test]
fn theme_export_failure_is_reported_and_names_the_cause() {
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));

    let _ = app.update(inner(PrefMsg::ThemeExportFinished(Err(
        "/ro/custom-theme.json: Read-only file system (os error 30)".to_string(),
    ))));

    let status = &app.ui_state.preferences_theme_status;
    assert!(
        status.contains("Could not export theme"),
        "the failure must be named as an export failure, got {status:?}"
    );
    assert!(
        status.contains("Read-only file system (os error 30)"),
        "the underlying io::Error must survive to the user, got {status:?}"
    );
}

#[test]
fn theme_export_success_names_the_written_path() {
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));

    let _ = app.update(inner(PrefMsg::ThemeExportFinished(Ok(PathBuf::from(
        "/tmp/custom-theme.json",
    )))));

    assert!(
        app.ui_state
            .preferences_theme_status
            .contains("/tmp/custom-theme.json"),
        "success must name where the file landed, got {:?}",
        app.ui_state.preferences_theme_status
    );
}

/// The status line is feedback about one export, not an edit. Reporting
/// it must never trip the unsaved-changes guard, or a failed export
/// would trap the user in a dialog that refuses to close.
#[test]
fn reporting_an_export_outcome_does_not_mark_the_dialog_dirty() {
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    assert!(
        !app.ui_state.preferences_dirty,
        "fixture precondition: a freshly opened dialog is clean"
    );

    let _ = app.update(inner(PrefMsg::ThemeExportFinished(Err("boom".to_string()))));
    let _ = app.update(inner(PrefMsg::KeymapExportFinished(
        Err("boom".to_string()),
    )));

    assert!(
        !app.ui_state.preferences_dirty,
        "an export status line is not an unsaved change"
    );
}

#[test]
fn keymap_export_failure_is_reported_and_names_the_cause() {
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));

    let _ = app.update(inner(PrefMsg::KeymapExportFinished(Err(
        "/ro/custom.toml: No space left on device (os error 28)".to_string(),
    ))));

    let status = &app.ui_state.preferences_keymap_status;
    assert!(
        status.contains("Could not export keyboard shortcuts"),
        "the failure must be named as an export failure, got {status:?}"
    );
    assert!(
        status.contains("No space left on device (os error 28)"),
        "the underlying io::Error must survive to the user, got {status:?}"
    );
}

#[test]
fn keymap_export_success_names_the_written_path() {
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));

    let _ = app.update(inner(PrefMsg::KeymapExportFinished(Ok(PathBuf::from(
        "/tmp/my-profile.toml",
    )))));

    assert!(
        app.ui_state
            .preferences_keymap_status
            .contains("/tmp/my-profile.toml"),
        "success must name where the file landed, got {:?}",
        app.ui_state.preferences_keymap_status
    );
}

/// Both status lines are transient. Opening the dialog reseeds every
/// draft from the live value; a result from a previous session must not
/// come back with it and describe a write that is no longer on screen.
#[test]
fn opening_preferences_clears_a_stale_export_status() {
    let (mut app, _t) = Signex::new();
    app.ui_state.preferences_theme_status = "Exported theme to /old/path.json.".to_string();
    app.ui_state.preferences_keymap_status =
        "Could not export keyboard shortcuts: boom".to_string();

    let _ = app.update(Message::Preferences(PreferencesMsg::Open));

    assert!(
        app.ui_state.preferences_theme_status.is_empty(),
        "a reseed must drop the Appearance status, got {:?}",
        app.ui_state.preferences_theme_status
    );
    assert!(
        app.ui_state.preferences_keymap_status.is_empty(),
        "a reseed must drop the Keyboard Shortcuts status, got {:?}",
        app.ui_state.preferences_keymap_status
    );
}
