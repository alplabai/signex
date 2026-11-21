//! Preferences ▸ Appearance ▸ Selected option: it persists, a bad saved value
//! reads Neutral, and Apply and a theme switch reach every window at once.

use crate::app::{Message, PreferencesMsg, Signex, UiMsg};
use crate::fonts::{read_selected_segment_pref_from, write_selected_segment_pref_at};
use crate::preferences::PrefMsg;
use signex_types::theme::{Color, SelectedSegmentLook, ThemeId, theme_tokens};

fn send(app: &mut Signex, msg: PrefMsg) {
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(msg)));
}

/// An app in Graphite Dark with one undocked window's panel context.
fn dark_app_with_undocked() -> (Signex, iced::window::Id) {
    let (mut app, _boot) = Signex::new();
    let undocked = iced::window::Id::unique();
    app.document_state
        .window_panel_ctx
        .insert(undocked, app.document_state.panel_ctx.clone());
    send(&mut app, PrefMsg::DraftTheme(ThemeId::GraphiteDark));
    send(&mut app, PrefMsg::Save);
    (app, undocked)
}

/// The selected-segment fill of the main and the undocked window.
fn fills(app: &Signex, undocked: iced::window::Id) -> [Option<Color>; 2] {
    let document = &app.document_state;
    [
        document.panel_ctx.tokens.extended.selected_segment,
        document.window_panel_ctx[&undocked]
            .tokens
            .extended
            .selected_segment,
    ]
}

#[test]
fn the_preference_defaults_to_neutral_and_a_bad_value_reads_neutral() {
    assert_eq!(
        read_selected_segment_pref_from(None),
        SelectedSegmentLook::Neutral
    );
    for bad in [
        serde_json::json!({ "selected_segment": "sideways" }),
        serde_json::json!({ "selected_segment": 7 }),
        serde_json::json!({}),
    ] {
        assert_eq!(
            read_selected_segment_pref_from(Some(&bad)),
            SelectedSegmentLook::Neutral,
            "{bad}"
        );
    }
}

#[test]
fn the_preference_persists_and_round_trips() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("prefs.json");
    for look in [SelectedSegmentLook::Accent, SelectedSegmentLook::Neutral] {
        write_selected_segment_pref_at(&path, look);
        let saved: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(read_selected_segment_pref_from(Some(&saved)), look);
    }
}

#[test]
fn apply_changes_the_chip_in_the_main_and_undocked_windows() {
    let (mut app, undocked) = dark_app_with_undocked();
    let neutral = Some(Color::from_hex("#3D4047"));
    let accent = Some(Color::from_hex("#C2552D"));
    assert_eq!(fills(&app, undocked), [neutral, neutral]);

    send(
        &mut app,
        PrefMsg::DraftSelectedSegment(SelectedSegmentLook::Accent),
    );
    assert_eq!(
        fills(&app, undocked),
        [neutral, neutral],
        "a draft changes nothing until Apply"
    );

    send(&mut app, PrefMsg::Save);
    assert_eq!(app.ui_state.selected_segment, SelectedSegmentLook::Accent);
    assert_eq!(fills(&app, undocked), [accent, accent]);
}

#[test]
fn a_theme_switch_updates_the_chip_at_once_in_every_window() {
    let (mut app, undocked) = dark_app_with_undocked();
    send(
        &mut app,
        PrefMsg::DraftSelectedSegment(SelectedSegmentLook::Accent),
    );
    send(&mut app, PrefMsg::Save);

    let _ = app.update(Message::Ui(UiMsg::ThemeChanged(ThemeId::GraphiteLight)));
    let light = theme_tokens(ThemeId::GraphiteLight)
        .extended
        .selected_segment;
    assert_eq!(fills(&app, undocked), [light, light]);

    let _ = app.update(Message::Ui(UiMsg::ThemeChanged(ThemeId::GraphiteDark)));
    let accent = Some(Color::from_hex("#C2552D"));
    assert_eq!(fills(&app, undocked), [accent, accent]);
}
