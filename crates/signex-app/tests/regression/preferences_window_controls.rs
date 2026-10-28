//! The Window controls placement is a Preferences draft like its neighbours:
//! picking one must not take effect (or persist) before Save, and Discard
//! puts it back. No test here touches the filesystem — `PrefMsg::Save`
//! writes the shared prefs path (see `preferences_symbol_drafts.rs`).

use signex_app::app::{Message, PreferencesMsg, Signex};
use signex_app::chrome::controls::WindowControlsPlacement;
use signex_app::preferences::PrefMsg;

fn inner(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

fn other(current: WindowControlsPlacement) -> WindowControlsPlacement {
    match current {
        WindowControlsPlacement::Auto => WindowControlsPlacement::Trailing,
        WindowControlsPlacement::Leading | WindowControlsPlacement::Trailing => {
            WindowControlsPlacement::Auto
        }
    }
}

#[test]
fn choosing_a_placement_is_a_draft_until_save() {
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let committed = app.ui_state.window_controls_placement;
    let wanted = other(committed);

    let _ = app.update(inner(PrefMsg::DraftWindowControlsPlacement(wanted)));

    assert_eq!(
        app.ui_state.preferences_draft_window_controls_placement,
        wanted
    );
    assert_eq!(
        app.ui_state.window_controls_placement, committed,
        "an unsaved draft must not change the live placement"
    );
    assert!(app.ui_state.preferences_dirty, "the footer must light up");
}

#[test]
fn discarding_puts_the_placement_draft_back() {
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    let committed = app.ui_state.window_controls_placement;
    let _ = app.update(inner(PrefMsg::DraftWindowControlsPlacement(other(
        committed,
    ))));

    let _ = app.update(inner(PrefMsg::DiscardAndClose));

    assert_eq!(
        app.ui_state.preferences_draft_window_controls_placement,
        committed
    );
    assert!(!app.ui_state.preferences_dirty);
}
