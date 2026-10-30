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

// Restore and Discard act on the backup the Preferences row shows, never on
// a fresh scan: a scan after Apply finds the `.bak.N` Apply just wrote, the
// only copy of the file it overwrote.
mod backup_row {
    use super::*;
    use crate::config_root::isolated::IsolatedConfigRoot;
    use std::path::PathBuf;

    // Every other test that presses Save writes the keymap file under the
    // shared per-process redirect root, so these take a private root for
    // the length of the test (#1750) rather than clearing a shared one.
    fn clean_slate() -> (IsolatedConfigRoot, PathBuf, PathBuf) {
        let root = IsolatedConfigRoot::new();
        let live = crate::keymap::config_path().expect("test redirect root");
        let mut bak = live.file_name().unwrap().to_os_string();
        bak.push(".bak");
        (root, live.clone(), live.with_file_name(bak))
    }

    fn slot(live: &std::path::Path, suffix: &str) -> PathBuf {
        let mut name = live.file_name().unwrap().to_os_string();
        name.push(suffix);
        live.with_file_name(name)
    }

    #[test]
    fn apply_after_a_failed_load_points_the_row_at_the_backup_it_wrote_and_discard_deletes_that_one()
     {
        let (_root, live, bak) = clean_slate();
        std::fs::write(&bak, "older backup").unwrap();
        std::fs::write(&live, "this is [not a shortcuts file").unwrap();
        let (mut app, _boot) = Signex::new();
        app.ui_state.keymap_load_error = Some("broken".to_string());
        app.refresh_keymap_backup();
        assert_eq!(
            app.ui_state.keymap_backup.as_deref(),
            Some(bak.display().to_string().as_str())
        );

        send(&mut app, PrefMsg::Save);

        let second = slot(&live, ".bak.2");
        assert!(second.exists(), "Apply backed the broken file up");
        assert_eq!(
            app.ui_state.keymap_backup.as_deref(),
            Some(second.display().to_string().as_str()),
            "the row must name the backup Apply just wrote"
        );

        send(&mut app, PrefMsg::KeymapDiscardBackup);

        assert!(!second.exists(), "the row's backup is the one deleted");
        assert!(bak.exists(), "the older backup is untouched");
        assert_eq!(
            app.ui_state.keymap_backup.as_deref(),
            Some(bak.display().to_string().as_str()),
            "the row falls back to the remaining backup"
        );
    }

    #[test]
    fn discard_refuses_a_displayed_backup_that_is_gone_and_touches_nothing_else() {
        let (_root, live, bak) = clean_slate();
        let other = slot(&live, ".bak.2");
        std::fs::write(&other, "someone else's backup").unwrap();
        let (mut app, _boot) = Signex::new();
        app.ui_state.keymap_backup = Some(bak.display().to_string());

        send(&mut app, PrefMsg::KeymapDiscardBackup);

        assert!(other.exists(), "a scan must not retarget the click");
        assert_eq!(
            app.ui_state.keymap_backup.as_deref(),
            Some(other.display().to_string().as_str()),
            "the row is refreshed to what exists"
        );
        assert!(app.ui_state.preferences_keymap_status.contains("no longer"));
    }
}
