//! #1378 — a recorder holds a slot into the profile it was opened on, so
//! anything that changes the profile set must close it.

use crate::app::{Message, PreferencesMsg, Signex};
use crate::keymap::{BindingSlot, KeymapEditorModel};
use crate::preferences::PrefMsg;

fn app_recording_a_custom_profile_row() -> Signex {
    let (mut app, _boot) = Signex::new();
    let mut editor = KeymapEditorModel::built_ins().unwrap();
    editor
        .create_custom_from_active("custom-1", "Custom 1")
        .unwrap();
    let row = editor
        .rows()
        .into_iter()
        .find(|row| row.trigger == "Ctrl+W")
        .expect("altium binds Ctrl+W");
    app.ui_state.preferences_keymap_editor = editor;
    app.ui_state.preferences_keymap_recorder = Some(crate::app::KeymapRecorderState::new(
        row.slot,
        row.command.clone().unwrap(),
        row.label,
        row.context,
        row.trigger,
    ));
    app
}

fn send(app: &mut Signex, msg: PrefMsg) {
    let _ = app.update(Message::Preferences(PreferencesMsg::Inner(msg)));
}

#[test]
fn switching_profile_closes_the_recorder() {
    let mut app = app_recording_a_custom_profile_row();
    send(&mut app, PrefMsg::KeymapProfileSelected("altium".into()));
    assert!(app.ui_state.preferences_keymap_recorder.is_none());
}

#[test]
fn creating_a_custom_profile_closes_the_recorder() {
    let mut app = app_recording_a_custom_profile_row();
    send(&mut app, PrefMsg::KeymapCreateCustomProfile);
    assert!(app.ui_state.preferences_keymap_recorder.is_none());
}

#[test]
fn deleting_the_active_profile_closes_the_recorder() {
    let mut app = app_recording_a_custom_profile_row();
    send(&mut app, PrefMsg::KeymapDeleteActiveProfile);
    assert!(app.ui_state.preferences_keymap_recorder.is_none());
}

#[test]
fn importing_a_profile_closes_the_recorder() {
    let mut app = app_recording_a_custom_profile_row();
    send(
        &mut app,
        PrefMsg::KeymapProfileLoaded("not a profile".into()),
    );
    assert!(
        app.ui_state.preferences_keymap_recorder.is_none(),
        "closed even though the import itself failed"
    );
}

#[test]
fn a_stale_slot_is_refused_not_applied_to_another_binding() {
    let mut app = app_recording_a_custom_profile_row();
    let mut recorder = app.ui_state.preferences_keymap_recorder.clone().unwrap();
    recorder.slot = BindingSlot {
        binding: usize::MAX,
        trigger: 0,
    };
    recorder.strokes = vec![
        crate::keymap::KeyStroke::from_iced(
            &iced::keyboard::Key::Character("x".into()),
            iced::keyboard::Modifiers::default(),
        )
        .unwrap(),
    ];
    app.ui_state.preferences_keymap_recorder = Some(recorder);
    let before = app.ui_state.preferences_keymap_editor.clone();

    send(&mut app, PrefMsg::KeymapRecorderApply);

    assert_eq!(
        app.ui_state.preferences_keymap_editor.profile_set(),
        before.profile_set()
    );
    assert!(
        app.ui_state
            .preferences_keymap_status
            .contains("no longer exists")
    );
}
