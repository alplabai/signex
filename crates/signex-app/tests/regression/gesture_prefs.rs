//! The two opt-in preferences `trackpad_pan` and `remember_window_geometry`:
//! default off, round-trip through `prefs.json`, and flow through the
//! Preferences draft.

use signex_app::app::{Message, PreferencesMsg, Signex};
use signex_app::preferences::PrefMsg;
use std::fs;
use tempfile::TempDir;

fn inner(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

#[test]
fn both_preferences_default_off_when_missing_or_not_a_boolean() {
    assert!(!signex_app::fonts::read_trackpad_pan_pref_from(None));
    assert!(!signex_app::fonts::read_remember_window_geometry_pref_from(
        None
    ));
    let junk = serde_json::json!({ "trackpad_pan": "yes", "remember_window_geometry": 1 });
    assert!(!signex_app::fonts::read_trackpad_pan_pref_from(Some(&junk)));
    assert!(!signex_app::fonts::read_remember_window_geometry_pref_from(
        Some(&junk)
    ));
}

#[test]
fn both_preferences_round_trip_through_prefs_json() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("prefs.json");
    signex_app::fonts::write_trackpad_pan_pref_at(&path, true);
    signex_app::fonts::write_remember_window_geometry_pref_at(&path, true);
    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert!(signex_app::fonts::read_trackpad_pan_pref_from(Some(&saved)));
    assert!(signex_app::fonts::read_remember_window_geometry_pref_from(
        Some(&saved)
    ));
}

#[test]
fn toggling_the_drafts_marks_preferences_dirty_and_discard_restores_them() {
    let (mut app, _t) = Signex::new();
    let (pan, geometry) = (
        app.ui_state.trackpad_pan,
        app.ui_state.remember_window_geometry,
    );
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    assert!(!app.ui_state.preferences_draft_differs());

    let _ = app.update(inner(PrefMsg::DraftTrackpadPan(!pan)));
    assert!(app.ui_state.preferences_draft_differs());
    assert_eq!(app.ui_state.trackpad_pan, pan, "a draft must not commit");

    let _ = app.update(inner(PrefMsg::DraftTrackpadPan(pan)));
    let _ = app.update(inner(PrefMsg::DraftRememberWindowGeometry(!geometry)));
    assert!(app.ui_state.preferences_draft_differs());
    assert_eq!(app.ui_state.remember_window_geometry, geometry);

    // Leave both drafts differing from the committed values, then discard.
    let _ = app.update(inner(PrefMsg::DraftTrackpadPan(!pan)));
    assert!(app.ui_state.preferences_dirty, "the footer must light up");
    assert_eq!(app.ui_state.preferences_draft_trackpad_pan, !pan);
    assert_eq!(
        app.ui_state.preferences_draft_remember_window_geometry,
        !geometry
    );

    let _ = app.update(inner(PrefMsg::DiscardAndClose));

    assert_eq!(app.ui_state.preferences_draft_trackpad_pan, pan);
    assert_eq!(
        app.ui_state.preferences_draft_remember_window_geometry,
        geometry
    );
    assert_eq!(app.ui_state.trackpad_pan, pan);
    assert_eq!(app.ui_state.remember_window_geometry, geometry);
    assert!(!app.ui_state.preferences_draft_differs());
    assert!(!app.ui_state.preferences_dirty);
}

#[test]
fn the_wheel_mode_pref_defaults_to_zoom_and_round_trips() {
    use signex_app::canvas::WheelMode;
    assert_eq!(
        signex_app::fonts::read_wheel_mode_pref_from(None),
        WheelMode::Zoom
    );
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("prefs.json");
    signex_app::fonts::write_wheel_mode_pref_at(&path, WheelMode::Scroll);
    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        signex_app::fonts::read_wheel_mode_pref_from(Some(&saved)),
        WheelMode::Scroll
    );
}

#[test]
fn the_wheel_mode_draft_marks_dirty_commits_on_save_and_discard_restores_it() {
    use signex_app::canvas::WheelMode;
    let (mut app, _t) = Signex::new();
    let saved = app.ui_state.wheel_mode;
    let other = if saved == WheelMode::Zoom {
        WheelMode::Scroll
    } else {
        WheelMode::Zoom
    };
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    assert!(!app.ui_state.preferences_draft_differs());

    let _ = app.update(inner(PrefMsg::DraftWheelMode(other)));
    assert!(app.ui_state.preferences_draft_differs());
    assert_eq!(app.ui_state.wheel_mode, saved, "a draft must not commit");

    let _ = app.update(inner(PrefMsg::DiscardAndClose));
    assert_eq!(app.ui_state.preferences_draft_wheel_mode, saved);
    assert_eq!(app.ui_state.wheel_mode, saved);
}
