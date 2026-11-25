//! #1378 — a recorder holds a slot into the profile it was opened on, so
//! anything that changes the profile set must close it.

use crate::app::{Message, PreferencesMsg, Signex};
use crate::keymap::{BindingSlot, KeymapEditorModel};
use crate::preferences::PrefMsg;

fn app_recording_a_custom_profile_row() -> Signex {
    let (mut app, _boot) = Signex::new();
    let mut editor = KeymapEditorModel::built_ins()
        .unwrap()
        .with_platform(crate::keymap::Platform::Windows);
    editor
        .create_custom_from_active("custom-1", "Custom 1")
        .unwrap();
    let row = editor
        .rows()
        .into_iter()
        .find(|row| row.trigger == "Ctrl+W")
        .expect("signex_standard binds Ctrl+W");
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
    send(
        &mut app,
        PrefMsg::KeymapProfileSelected("signex_standard".into()),
    );
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

// Mockup row 145 — the page's chips and the banner's Show are view state only.
mod view_state {
    use super::*;
    use crate::preferences::{KeymapViewMsg, KeymapViewState};

    #[test]
    fn show_turns_on_the_conflicts_chip_without_dirtying_the_dialog() {
        let (mut app, _boot) = Signex::new();
        let _ = app.update(Message::Preferences(PreferencesMsg::Open));
        assert!(!app.ui_state.preferences_dirty);

        let _ = app.handle_preferences_keymap_view(KeymapViewMsg::ConflictsShown);

        assert!(app.ui_state.preferences_pages.keymap_view.shows_conflicts());
        assert!(!app.ui_state.preferences_dirty);
        assert!(app.ui_state.preferences_keymap_status.is_empty());
    }

    #[test]
    fn picking_a_chip_leaves_the_working_copy_and_the_recorder_alone() {
        let mut app = app_recording_a_custom_profile_row();
        let before = app.ui_state.preferences_keymap_editor.clone();
        let recorder = app.ui_state.preferences_keymap_recorder.clone();

        let _ = app.handle_preferences_keymap_view(KeymapViewMsg::ConflictsShown);
        let mut expected = KeymapViewState::default();
        expected.apply(KeymapViewMsg::ConflictsShown);
        assert_eq!(app.ui_state.preferences_pages.keymap_view, expected);

        let _ = app.handle_preferences_keymap_view(KeymapViewMsg::ContextFilterPicked(
            KeymapViewState::default().context_filter,
        ));
        assert_eq!(
            app.ui_state.preferences_pages.keymap_view,
            KeymapViewState::default()
        );
        assert_eq!(app.ui_state.preferences_keymap_editor, before);
        assert_eq!(app.ui_state.preferences_keymap_recorder, recorder);
    }
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

// Mockup row 145: the recorder gives any command a key, saved in the custom profile.
mod recorder_flow {
    use super::*;
    use crate::config_root::isolated::IsolatedConfigRoot;
    use crate::keymap::{AppCommandId, ExtensionCommandNames, KeyStroke};
    use iced::keyboard;

    fn stroke(key: keyboard::Key, modifiers: keyboard::Modifiers) -> KeyStroke {
        KeyStroke::from_iced(&key, modifiers).unwrap()
    }

    fn press(key: keyboard::Key) -> keyboard::Event {
        keyboard::Event::KeyPressed {
            key: key.clone(),
            modified_key: key,
            physical_key: keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Unidentified,
            ),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::default(),
            text: None,
            repeat: false,
        }
    }

    /// The recorder opened, through the row's own Edit message, on a
    /// command the custom profile gives no key.
    fn app_recording_an_unbound_command() -> (Signex, AppCommandId) {
        let (mut app, _boot) = Signex::new();
        app.ui_state.first_run_tour_open = false;
        let mut editor = KeymapEditorModel::built_ins()
            .unwrap()
            .with_platform(crate::keymap::Platform::Windows);
        editor
            .create_custom_from_active("custom-1", "Custom 1")
            .unwrap();
        let row = editor
            .unassigned_rows_in_with(
                signex_i18n::I18n::en_us_ref(),
                &ExtensionCommandNames::default(),
                crate::app::command::bridge::is_dispatchable,
            )
            .into_iter()
            .next()
            .expect("some command has no key");
        app.ui_state.preferences_keymap_editor = editor;
        let command = row.command.clone().unwrap();
        send(
            &mut app,
            PrefMsg::KeymapRecorderOpen {
                slot: row.slot,
                command: command.clone(),
                label: row.label,
                context: row.context,
                trigger: row.trigger_config,
                unassigned: row.unassigned,
            },
        );
        (app, command)
    }

    #[test]
    fn esc_is_recorded_as_a_key_and_the_recorder_stays_open() {
        let (mut app, _) = app_recording_an_unbound_command();
        let main = app.ui_state.main_window_id.unwrap();
        let esc = press(keyboard::Key::Named(keyboard::key::Named::Escape));

        let routed = app
            .route_key_for_test(main, &esc)
            .expect("the recorder claims Esc");
        let _ = app.update(routed);

        let recorder = app
            .ui_state
            .preferences_keymap_recorder
            .as_ref()
            .expect("Esc does not close the recorder");
        assert_eq!(
            recorder.strokes,
            [stroke(
                keyboard::Key::Named(keyboard::key::Named::Escape),
                keyboard::Modifiers::default()
            )]
        );
    }

    #[test]
    fn cancel_closes_the_recorder_and_changes_nothing() {
        let (mut app, _) = app_recording_an_unbound_command();
        let before = app.ui_state.preferences_keymap_editor.clone();
        send(
            &mut app,
            PrefMsg::KeymapRecorderKeyPressed(stroke(
                keyboard::Key::Character("q".into()),
                keyboard::Modifiers::CTRL | keyboard::Modifiers::ALT,
            )),
        );

        send(&mut app, PrefMsg::KeymapRecorderCancel);

        assert!(app.ui_state.preferences_keymap_recorder.is_none());
        assert_eq!(app.ui_state.preferences_keymap_editor, before);
    }

    #[test]
    fn ok_binds_the_command_and_save_writes_it_to_the_custom_profile() {
        let _root = IsolatedConfigRoot::new();
        let (mut app, command) = app_recording_an_unbound_command();
        send(
            &mut app,
            PrefMsg::KeymapRecorderKeyPressed(stroke(
                keyboard::Key::Character("q".into()),
                keyboard::Modifiers::CTRL | keyboard::Modifiers::ALT,
            )),
        );

        send(&mut app, PrefMsg::KeymapRecorderApply);

        assert!(app.ui_state.preferences_keymap_recorder.is_none());
        assert!(app.ui_state.preferences_dirty, "a new key is an unsaved change");
        send(&mut app, PrefMsg::Save);
        let path = crate::keymap::config_path().unwrap();
        let saved = crate::keymap::load_profile_set_at(&path).unwrap();
        let profile = saved.active_profile();
        assert_eq!(profile.id, "custom-1");
        let binding = profile
            .bindings
            .iter()
            .find(|binding| binding.action.command() == Some(&command))
            .expect("the key is saved in the custom profile");
        assert_eq!(binding.triggers[0].config_text(), "Ctrl+Alt+Q");
        assert!(
            app.ui_state.active_keymap.shortcut_label(&command).is_some(),
            "the running keymap has it"
        );
    }

    #[test]
    fn ok_with_nothing_recorded_refuses_a_command_without_a_key() {
        let (mut app, _) = app_recording_an_unbound_command();
        let before = app.ui_state.preferences_keymap_editor.clone();

        send(&mut app, PrefMsg::KeymapRecorderApply);

        assert_eq!(app.ui_state.preferences_keymap_editor, before);
        assert!(app.ui_state.preferences_keymap_recorder.is_some());
    }

    #[test]
    fn clear_then_ok_takes_the_key_away() {
        let mut app = app_recording_a_custom_profile_row();
        let command = app
            .ui_state
            .preferences_keymap_recorder
            .as_ref()
            .unwrap()
            .command
            .clone();
        let count = app
            .ui_state
            .preferences_keymap_editor
            .active_profile()
            .bindings
            .len();

        send(&mut app, PrefMsg::KeymapRecorderClear);
        send(&mut app, PrefMsg::KeymapRecorderApply);

        assert!(app.ui_state.preferences_keymap_recorder.is_none());
        let editor = &app.ui_state.preferences_keymap_editor;
        assert_eq!(editor.active_profile().bindings.len(), count - 1);
        assert!(
            editor
                .rows()
                .iter()
                .all(|row| !(row.command.as_ref() == Some(&command) && row.trigger == "Ctrl+W"))
        );
    }

    #[test]
    fn create_copies_a_built_in_and_delete_removes_only_the_copy() {
        let (mut app, _boot) = Signex::new();
        let built_ins = KeymapEditorModel::built_ins().unwrap();
        app.ui_state.preferences_keymap_editor = built_ins.clone();

        send(&mut app, PrefMsg::KeymapCreateCustomProfile);

        let editor = &app.ui_state.preferences_keymap_editor;
        assert!(editor.active_profile_is_custom());
        assert_eq!(
            editor.active_profile().bindings,
            built_ins.active_profile().bindings
        );
        for profile in built_ins.profile_set().profiles() {
            assert_eq!(
                editor.profile_set().profiles().find(|p| p.id == profile.id),
                Some(profile),
                "the built-in is unchanged"
            );
        }

        send(&mut app, PrefMsg::KeymapDeleteActiveProfile);

        assert_eq!(
            app.ui_state.preferences_keymap_editor.profile_set(),
            built_ins.profile_set()
        );
    }

    #[test]
    fn import_adds_the_file_as_a_custom_profile() {
        let (mut app, _boot) = Signex::new();
        let mut source_editor = KeymapEditorModel::built_ins().unwrap();
        source_editor
            .create_custom_from_active("shared", "Shared")
            .unwrap();
        let source = crate::keymap::export_custom_profile(source_editor.active_profile()).unwrap();

        send(&mut app, PrefMsg::KeymapProfileLoaded(source));

        assert!(
            app.ui_state
                .preferences_keymap_editor
                .profiles()
                .iter()
                .any(|profile| profile.id == "shared"
                    && profile.kind == crate::keymap::ShortcutProfileKind::Custom)
        );
        assert!(app.ui_state.preferences_dirty);
    }
}
