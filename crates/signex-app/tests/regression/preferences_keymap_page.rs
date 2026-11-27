//! #2621, mockup row 145 — the Keyboard shortcuts page. The context chips and the
//! conflict banner's Show are page view state: they filter the table, never
//! touch the keymap working copy, never mark the dialog dirty, and start
//! over each time Preferences opens. Delete stays refused on a built-in.

use signex_app::app::{Message, PreferencesMsg, Signex};
use signex_app::preferences::{KeymapViewMsg, KeymapViewState, PrefMsg};

fn inner(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

fn opened() -> Signex {
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));
    app
}

#[test]
fn show_in_the_conflict_banner_turns_on_the_conflicts_chip() {
    let mut app = opened();
    assert_eq!(
        app.ui_state.preferences_pages.keymap_view,
        KeymapViewState::default(),
        "fixture precondition: the page opens on All"
    );
    let editor_before = app.ui_state.preferences_keymap_editor.clone();

    let _ = app.update(inner(PrefMsg::KeymapView(KeymapViewMsg::ConflictsShown)));

    assert!(app.ui_state.preferences_pages.keymap_view.shows_conflicts());
    assert_eq!(app.ui_state.preferences_keymap_editor, editor_before);
    assert!(
        !app.ui_state.preferences_dirty,
        "a filter is not an unsaved change"
    );
}

#[test]
fn picking_all_turns_the_conflicts_chip_off_again() {
    let mut app = opened();
    let _ = app.update(inner(PrefMsg::KeymapView(KeymapViewMsg::ConflictsShown)));
    let all = KeymapViewState::default().context_filter;

    let _ = app.update(inner(PrefMsg::KeymapView(
        KeymapViewMsg::ContextFilterPicked(all),
    )));

    assert_eq!(
        app.ui_state.preferences_pages.keymap_view,
        KeymapViewState::default()
    );
    assert!(!app.ui_state.preferences_dirty);
}

#[test]
fn reopening_preferences_starts_the_page_on_all() {
    let mut app = opened();
    let _ = app.update(inner(PrefMsg::KeymapView(KeymapViewMsg::ConflictsShown)));
    let _ = app.update(inner(PrefMsg::Close));
    let _ = app.update(Message::Preferences(PreferencesMsg::Open));

    assert_eq!(
        app.ui_state.preferences_pages.keymap_view,
        KeymapViewState::default()
    );
}

#[test]
fn delete_on_a_built_in_profile_changes_nothing() {
    let mut app = opened();
    let _ = app.update(inner(PrefMsg::KeymapProfileSelected("classic".into())));
    assert!(
        !app.ui_state
            .preferences_keymap_editor
            .active_profile_is_custom(),
        "fixture precondition: Classic is a built-in profile"
    );
    let before = app.ui_state.preferences_keymap_editor.clone();
    let dirty_before = app.ui_state.preferences_dirty;

    let _ = app.update(inner(PrefMsg::KeymapDeleteActiveProfile));

    assert_eq!(
        app.ui_state.preferences_keymap_editor.profile_set(),
        before.profile_set()
    );
    assert_eq!(app.ui_state.preferences_dirty, dirty_before);
}

/// #2495 (Caner, 2026-11-25): a command with no key gets one from the
/// recorder, in a custom profile; on a built-in it is refused.
#[test]
fn a_command_without_a_key_gets_one_in_a_custom_profile_only() {
    use signex_app::keymap::{ExtensionCommandNames, KeyStroke};
    let mut app = opened();
    let _ = app.update(inner(PrefMsg::KeymapCreateCustomProfile));
    let row = app
        .ui_state
        .preferences_keymap_editor
        .unassigned_rows_in_with(
            &app.ui_state.i18n,
            &ExtensionCommandNames::default(),
            |_| true,
        )
        .into_iter()
        .next()
        .expect("some command has no key");
    let command = row.command.clone().unwrap();
    let open = PrefMsg::KeymapRecorderOpen {
        slot: row.slot,
        command: command.clone(),
        label: row.label,
        context: row.context,
        trigger: row.trigger_config,
        unassigned: true,
    };
    let stroke = KeyStroke::from_iced(
        &iced::keyboard::Key::Character("q".into()),
        iced::keyboard::Modifiers::CTRL | iced::keyboard::Modifiers::ALT,
    )
    .unwrap();

    let _ = app.update(inner(open.clone()));
    let _ = app.update(inner(PrefMsg::KeymapRecorderKeyPressed(stroke.clone())));
    let _ = app.update(inner(PrefMsg::KeymapRecorderApply));

    let bound = |app: &Signex| {
        app.ui_state
            .preferences_keymap_editor
            .rows_in(&app.ui_state.i18n)
            .iter()
            .any(|row| row.command.as_ref() == Some(&command))
    };
    assert!(bound(&app), "the command has its key");

    let _ = app.update(inner(PrefMsg::KeymapProfileSelected("classic".into())));
    let before = app.ui_state.preferences_keymap_editor.clone();
    let _ = app.update(inner(open));
    let _ = app.update(inner(PrefMsg::KeymapRecorderKeyPressed(stroke)));
    let _ = app.update(inner(PrefMsg::KeymapRecorderApply));
    assert_eq!(
        app.ui_state.preferences_keymap_editor.profile_set(),
        before.profile_set(),
        "a built-in profile is read-only"
    );
}
