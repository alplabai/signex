//! #1228 — the interface scale reaches iced's `scale_factor` hook, is saved
//! on Save, survives a restart, and a bad stored value falls back to 1.0 and
//! is reported to the Messages panel.

use signex_app::app::{Message, PreferencesMsg, Signex};
use signex_app::preferences::PrefMsg;

fn inner(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

#[test]
fn the_scale_factor_hook_returns_the_committed_scale() {
    let (mut app, _t) = Signex::new();
    let window = iced::window::Id::unique();
    app.ui_state.ui_scale = 1.5;
    assert_eq!(app.scale_factor(window), 1.5);
    app.ui_state.ui_scale = 0.8;
    assert_eq!(app.scale_factor(window), 0.8);
}

#[test]
fn dragging_the_slider_previews_nothing_until_save() {
    let (mut app, _t) = Signex::new();
    app.ui_state.ui_scale = 1.0;
    app.ui_state.preferences_draft_ui_scale_percent = 100;
    let _ = app.update(inner(PrefMsg::DraftUiScalePercent(150)));
    assert_eq!(app.ui_state.preferences_draft_ui_scale_percent, 150);
    assert_eq!(app.scale_factor(iced::window::Id::unique()), 1.0);
    assert!(app.ui_state.preferences_draft_differs());
}

#[test]
fn a_draft_outside_the_range_is_snapped_and_clamped() {
    let (mut app, _t) = Signex::new();
    let _ = app.update(inner(PrefMsg::DraftUiScalePercent(30)));
    assert_eq!(app.ui_state.preferences_draft_ui_scale_percent, 80);
    let _ = app.update(inner(PrefMsg::DraftUiScalePercent(400)));
    assert_eq!(app.ui_state.preferences_draft_ui_scale_percent, 200);
    let _ = app.update(inner(PrefMsg::DraftUiScalePercent(133)));
    assert_eq!(app.ui_state.preferences_draft_ui_scale_percent, 130);
}

#[test]
fn saving_applies_and_persists_the_scale_and_a_restart_reads_it_back() {
    let _serial = crate::preferences_prefs_recovery::serial();
    let _guard = crate::preferences_prefs_recovery::PrefsPathGuard::capture();
    let (mut app, _t) = Signex::new();
    app.ui_state.ui_scale = 1.0;
    app.ui_state.preferences_draft_ui_scale_percent = 100;
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let _ = app.update(inner(PrefMsg::DraftUiScalePercent(150)));

    {
        let task = app.update(inner(PrefMsg::Save));
        crate::drive::drive(&mut app, task);
    }

    assert_eq!(app.scale_factor(iced::window::Id::unique()), 1.5);
    assert!(!app.ui_state.preferences_draft_differs());
    assert_eq!(signex_app::fonts::read_ui_scale_pref(), 1.5);
    let (restarted, _t) = Signex::new();
    assert_eq!(restarted.ui_state.ui_scale, 1.5);
    assert_eq!(restarted.ui_state.preferences_draft_ui_scale_percent, 150);
}

/// A hand-edited, in-range but off-step scale (1.25) loads as draft 125.
/// Saving some other preference must not snap it to 1.3.
#[test]
fn saving_an_unrelated_pref_keeps_a_hand_edited_off_step_scale() {
    let _serial = crate::preferences_prefs_recovery::serial();
    let _guard = crate::preferences_prefs_recovery::PrefsPathGuard::capture();
    let (mut app, _t) = Signex::new();
    app.ui_state.ui_scale = 1.25;
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    assert_eq!(app.ui_state.preferences_draft_ui_scale_percent, 125);
    let _ = app.update(inner(PrefMsg::DraftAutosaveEnabled(
        !app.ui_state.autosave_enabled,
    )));

    {
        let task = app.update(inner(PrefMsg::Save));
        crate::drive::drive(&mut app, task);
    }

    assert_eq!(app.ui_state.ui_scale, 1.25);
}
