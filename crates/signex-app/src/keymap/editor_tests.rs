use super::{AppCommandId, KeyStroke, KeymapEditorModel, ShortcutContext};
use std::str::FromStr;

#[test]
fn custom_profile_creation_copies_active_profile() {
    let mut editor = KeymapEditorModel::built_ins().unwrap();
    editor
        .create_custom_from_active("custom-altium", "Custom Altium")
        .unwrap();

    let profiles = editor.profiles();
    assert!(profiles.iter().any(|profile| profile.id == "custom-altium"));
    assert!(
        profiles
            .iter()
            .any(|profile| profile.id == "custom-altium" && profile.active)
    );
}

#[test]
fn editor_rows_mark_pointer_gestures_as_not_keyboard_editable() {
    let editor = KeymapEditorModel::built_ins().unwrap();
    let rows = editor.rows();
    let double_click = rows
        .iter()
        .find(|row| row.trigger == "DoubleClick")
        .expect("Altium profile should include DoubleClick reference trigger");
    assert!(!double_click.keyboard_editable);
}

#[test]
fn custom_profile_trigger_edit_updates_compiled_keymap() {
    let mut editor = KeymapEditorModel::built_ins().unwrap();
    editor
        .create_custom_from_active("custom-altium", "Custom Altium")
        .unwrap();
    let command = AppCommandId::new("save_document").unwrap();

    editor
        .edit_active_trigger(command.clone(), ShortcutContext::Global, "Ctrl+Alt+S".into())
        .unwrap();

    let keymap = editor.active_keymap();
    let lookup = keymap.lookup(
        &[KeyStroke::from_str("Ctrl+Alt+S").unwrap()],
        &[ShortcutContext::Global],
    );
    assert_eq!(lookup.command.as_ref(), Some(&command));
}

#[test]
fn built_in_profile_trigger_edit_is_rejected() {
    let mut editor = KeymapEditorModel::built_ins().unwrap();
    let command = AppCommandId::new("save_document").unwrap();

    let error = editor
        .edit_active_trigger(command, ShortcutContext::Global, "Ctrl+Alt+S".into())
        .unwrap_err()
        .to_string();

    assert!(error.contains("built-in profile `altium` cannot be modified"));
    assert!(editor.has_invalid_trigger_drafts());
}
