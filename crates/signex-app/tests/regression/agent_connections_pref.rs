//! #1924 — "Allow agent connections" is off until the person turns it on,
//! changes nothing until Save, and is read back after a restart. While it is
//! off no listener exists.

use signex_app::app::{Message, PreferencesMsg, Signex};
use signex_app::preferences::PrefMsg;

fn inner(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

#[test]
fn it_is_off_by_default_and_a_draft_changes_nothing_until_save() {
    let _serial = crate::preferences_prefs_recovery::serial();
    let _guard = crate::preferences_prefs_recovery::PrefsPathGuard::capture();
    let (mut app, _t) = Signex::new();
    assert!(!app.ui_state.agent_connections_enabled, "off by default");
    let before = app.subscription().units();

    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let _ = app.update(inner(PrefMsg::DraftAgentConnectionsEnabled(true)));

    assert!(app.ui_state.preferences_draft_differs());
    assert!(!app.ui_state.agent_connections_enabled);
    assert_eq!(
        app.subscription().units(),
        before,
        "no listener from a draft"
    );
}

#[test]
fn saving_turns_the_listener_on_and_off_and_a_restart_reads_the_choice_back() {
    let _serial = crate::preferences_prefs_recovery::serial();
    let _guard = crate::preferences_prefs_recovery::PrefsPathGuard::capture();
    let (mut app, _t) = Signex::new();
    let off = app.subscription().units();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let _ = app.update(inner(PrefMsg::DraftAgentConnectionsEnabled(true)));
    {
        let task = app.update(inner(PrefMsg::Save));
        crate::drive::drive(&mut app, task);
    }
    assert!(app.ui_state.agent_connections_enabled);
    assert_eq!(app.subscription().units(), off + 1, "one listener while on");
    let (restarted, _t) = Signex::new();
    assert!(restarted.ui_state.agent_connections_enabled, "read back");
    assert!(
        restarted
            .ui_state
            .preferences_draft_agent_connections_enabled
    );

    let _ = app.update(inner(PrefMsg::DraftAgentConnectionsEnabled(false)));
    {
        let task = app.update(inner(PrefMsg::Save));
        crate::drive::drive(&mut app, task);
    }
    assert!(!app.ui_state.agent_connections_enabled);
    assert_eq!(app.subscription().units(), off, "no listener while off");
    let (restarted, _t) = Signex::new();
    assert!(!restarted.ui_state.agent_connections_enabled);
}
