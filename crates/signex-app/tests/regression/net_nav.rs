//! Net navigation (#1270, #1271, #1272): the hover preference and the
//! highlight state through the public dispatcher.

use signex_app::app::{Message, NetNavMsg, Signex, UiMsg};
use std::fs;
use tempfile::TempDir;

#[test]
fn the_net_hover_preference_defaults_on_and_round_trips_through_prefs_json() {
    assert!(signex_app::fonts::read_net_hover_pref_from(None));
    let key_missing = serde_json::json!({ "theme": "signex" });
    assert!(signex_app::fonts::read_net_hover_pref_from(Some(
        &key_missing
    )));

    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("prefs.json");
    signex_app::fonts::write_net_hover_pref_at(&path, false);
    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert!(!signex_app::fonts::read_net_hover_pref_from(Some(&saved)));
}

#[test]
fn a_new_session_has_the_hover_on_and_no_highlight_and_clearing_changes_nothing() {
    let (mut app, _boot) = Signex::new();
    assert!(app.ui_state.net_hover_enabled);
    assert!(app.document_state.net_nav.highlight.is_none());

    let _ = app.update(Message::Ui(UiMsg::NetNav(
        NetNavMsg::HighlightClearRequested,
    )));

    assert!(app.document_state.net_nav.highlight.is_none());
}
