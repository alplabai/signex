//! Mockup row 145: a key for any command, built-in or extension, saved in a
//! custom profile; the built-in presets stay read-only.

use super::{
    AppCommandId, ExtensionCommandName, ExtensionCommandNames, KeymapEditorModel, Platform,
    ProfileLoadError, ShortcutBindingAction, ShortcutContext, ShortcutProfileSet, ShortcutTrigger,
    read_profile_file, write_profile_file,
};
use signex_i18n::I18n;

fn key(text: &str) -> ShortcutTrigger {
    ShortcutTrigger::parse(text).unwrap()
}

fn custom_editor() -> KeymapEditorModel {
    let mut editor = KeymapEditorModel::built_ins()
        .unwrap()
        .with_platform(Platform::Windows);
    editor
        .create_custom_from_active("custom-1", "Custom 1")
        .unwrap();
    editor
}

fn every_command(_: &AppCommandId) -> bool {
    true
}

fn sample_extension() -> (AppCommandId, ExtensionCommandNames) {
    let id = AppCommandId::new("ext.sample.say_hello").unwrap();
    let mut names = ExtensionCommandNames::default();
    names.insert(
        id.clone(),
        ExtensionCommandName {
            extension: "sample".to_string(),
            label: signex_i18n::shortcut_ids::PREF_KEYMAP_OK,
            category: "sample",
        },
    );
    (id, names)
}

/// The first catalog command the active profile gives no key.
fn unbound_command(editor: &KeymapEditorModel) -> AppCommandId {
    editor
        .unassigned_rows_in_with(
            I18n::en_us_ref(),
            &ExtensionCommandNames::default(),
            every_command,
        )
        .into_iter()
        .find_map(|row| row.command)
        .expect("the standard profile leaves some command without a key")
}

#[test]
fn every_command_without_a_key_gets_one_unbound_row() {
    let editor = custom_editor();
    let none = ExtensionCommandNames::default();
    let rows = editor.unassigned_rows_in_with(I18n::en_us_ref(), &none, every_command);
    let bound: Vec<AppCommandId> = editor
        .rows()
        .into_iter()
        .filter_map(|row| row.command)
        .collect();
    assert!(!rows.is_empty());
    for row in &rows {
        let command = row.command.as_ref().unwrap();
        assert!(row.unassigned && row.keyboard_editable && row.trigger.is_empty());
        assert!(!bound.contains(command), "{command} already has a row");
    }
    let listed = rows.len()
        + bound
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len();
    assert!(listed >= super::all_command_ids().count());
    // `offers` narrows the catalog.
    assert!(
        editor
            .unassigned_rows_in_with(I18n::en_us_ref(), &none, |_| false)
            .is_empty()
    );
}

#[test]
fn a_key_for_a_command_with_none_is_a_new_binding_after_the_others() {
    let mut editor = custom_editor();
    let command = unbound_command(&editor);
    let before = editor.active_profile().bindings.len();

    let slot = editor
        .assign_active_trigger(
            command.clone(),
            ShortcutContext::Global,
            key("Ctrl+Alt+F12"),
        )
        .unwrap();

    assert_eq!(slot.binding, before);
    let row = editor
        .rows()
        .into_iter()
        .find(|row| row.slot == slot)
        .expect("the new binding is a row");
    assert_eq!(row.command.as_ref(), Some(&command));
    assert_eq!(row.trigger_config, "Ctrl+Alt+F12");
    assert!(
        editor
            .unassigned_rows_in_with(
                I18n::en_us_ref(),
                &ExtensionCommandNames::default(),
                every_command
            )
            .iter()
            .all(|row| row.command.as_ref() != Some(&command)),
        "it is no longer listed as unbound"
    );
}

#[test]
fn the_built_in_presets_stay_read_only() {
    let mut editor = KeymapEditorModel::built_ins().unwrap();
    let command = unbound_command(&editor);
    let before = editor.profile_set().clone();
    let slot = editor.rows()[0].slot;

    assert!(matches!(
        editor.assign_active_trigger(command.clone(), ShortcutContext::Global, key("F12")),
        Err(ProfileLoadError::BuiltInMutation(_))
    ));
    assert!(matches!(
        editor.remove_active_trigger(slot),
        Err(ProfileLoadError::BuiltInMutation(_))
    ));
    assert!(matches!(
        editor.clear_active_command_keys(&command),
        Err(ProfileLoadError::BuiltInMutation(_))
    ));
    assert_eq!(editor.profile_set(), &before);
}

#[test]
fn a_key_given_to_a_second_command_is_a_conflict_naming_the_stroke_and_both() {
    let mut editor = custom_editor();
    let none = ExtensionCommandNames::default();
    let mut unbound = editor
        .unassigned_rows_in_with(I18n::en_us_ref(), &none, every_command)
        .into_iter()
        .filter_map(|row| row.command);
    let (first, second) = (unbound.next().unwrap(), unbound.next().unwrap());
    let before = editor.active_conflicts().len();
    editor
        .assign_active_trigger(first.clone(), ShortcutContext::Global, key("Ctrl+Alt+F11"))
        .unwrap();
    assert_eq!(editor.active_conflicts().len(), before);

    editor
        .assign_active_trigger(second.clone(), ShortcutContext::Global, key("Ctrl+Alt+F11"))
        .unwrap();

    let conflicts = editor.active_conflicts();
    assert_eq!(conflicts.len(), before + 1);
    let clash = conflicts
        .iter()
        .find(|conflict| {
            [&conflict.first_command, &conflict.second_command] == [&first, &second]
                || [&conflict.first_command, &conflict.second_command] == [&second, &first]
        })
        .expect("the clash names both commands");
    let ShortcutTrigger::KeySequence(strokes) = key("Ctrl+Alt+F11") else {
        unreachable!()
    };
    let resolved: Vec<_> = strokes
        .iter()
        .map(|stroke| stroke.resolved_for(Platform::Windows))
        .collect();
    assert_eq!(clash.sequence, resolved);
    assert_eq!(clash.context, ShortcutContext::Global);
}

#[test]
fn removing_a_key_drops_an_emptied_binding_and_keeps_later_drafts_on_their_rows() {
    let mut editor = custom_editor();
    let rows = editor.rows();
    let (first, later) = (rows[0].slot, rows[2].slot);
    editor
        .edit_active_trigger(later, "not a key ++".to_string())
        .unwrap_err();
    let count = editor.active_profile().bindings.len();

    editor.remove_active_trigger(first).unwrap();

    assert_eq!(editor.active_profile().bindings.len(), count - 1);
    let moved = editor
        .rows()
        .into_iter()
        .find(|row| !row.trigger_valid)
        .expect("the invalid draft is still shown");
    assert_eq!(moved.slot.binding, later.binding - 1);
    assert_eq!(moved.trigger, "not a key ++");
    assert!(matches!(
        editor.remove_active_trigger(super::BindingSlot {
            binding: usize::MAX,
            trigger: 0
        }),
        Err(ProfileLoadError::UnknownBindingSlot)
    ));
}

#[test]
fn clearing_a_command_takes_all_its_keys() {
    let mut editor = custom_editor();
    let command = AppCommandId::new("place_wire").unwrap();
    assert!(
        editor
            .rows()
            .iter()
            .any(|row| row.command.as_ref() == Some(&command))
    );

    let removed = editor.clear_active_command_keys(&command).unwrap();

    assert!(removed >= 1);
    assert!(
        editor
            .rows()
            .iter()
            .all(|row| row.command.as_ref() != Some(&command))
    );
    assert_eq!(editor.clear_active_command_keys(&command).unwrap(), 0);
}

#[test]
fn an_installed_extension_command_is_listed_and_bound_under_its_id() {
    let (id, names) = sample_extension();
    let mut editor = custom_editor();
    let row = editor
        .unassigned_rows_in_with(I18n::en_us_ref(), &names, |_| false)
        .into_iter()
        .find(|row| row.command.as_ref() == Some(&id))
        .expect("an installed extension command is listed without a key");
    assert!(
        row.label.contains("sample") && row.label.contains("OK"),
        "{}",
        row.label
    );
    assert_eq!(row.category, "sample");

    editor
        .assign_active_trigger(id.clone(), row.context, key("Ctrl+Alt+H"))
        .unwrap();

    let source = super::export_custom_profile(editor.active_profile()).unwrap();
    assert!(source.contains("ext.sample.say_hello"), "{source}");
}

#[test]
fn a_binding_to_an_uninstalled_extension_is_kept_and_not_offered_again() {
    let (id, names) = sample_extension();
    let mut editor = custom_editor();
    editor
        .assign_active_trigger(id.clone(), ShortcutContext::Global, key("Ctrl+Alt+H"))
        .unwrap();
    let none = ExtensionCommandNames::default();

    let rows = editor.rows_in_with(I18n::en_us_ref(), &none);
    let kept = rows
        .iter()
        .find(|row| row.command.as_ref() == Some(&id))
        .expect("the binding stays a row");
    assert!(kept.label.contains("unavailable"), "{}", kept.label);
    assert!(
        editor
            .unassigned_rows_in_with(I18n::en_us_ref(), &none, every_command)
            .iter()
            .all(|row| row.command.as_ref() != Some(&id))
    );
    // Installed again, it is bound, not unassigned.
    assert!(
        editor
            .unassigned_rows_in_with(I18n::en_us_ref(), &names, every_command)
            .iter()
            .all(|row| row.command.as_ref() != Some(&id))
    );

    // The saved file keeps it.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("keyboard_shortcuts.toml");
    super::save_profile_set_at(&path, editor.profile_set()).unwrap();
    let loaded: ShortcutProfileSet = super::load_profile_set_at(&path).unwrap();
    assert!(
        loaded
            .active_profile()
            .bindings
            .iter()
            .any(|binding| binding.action == ShortcutBindingAction::Command(id.clone()))
    );
}

#[test]
fn a_profile_file_round_trips_through_export_and_import() {
    let mut editor = custom_editor();
    let command = unbound_command(&editor);
    editor
        .assign_active_trigger(command, ShortcutContext::Global, key("Ctrl+Alt+F12"))
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mine.toml");

    write_profile_file(&path, editor.active_profile()).unwrap();
    let read = read_profile_file(&path).unwrap();

    assert_eq!(&read, editor.active_profile());
    // A built-in preset is not exported; Create copies it first.
    let built_in = KeymapEditorModel::built_ins().unwrap();
    assert!(matches!(
        write_profile_file(&dir.path().join("b.toml"), built_in.active_profile()),
        Err(ProfileLoadError::BuiltInMutation(_))
    ));
    assert!(!dir.path().join("b.toml").exists());
}
