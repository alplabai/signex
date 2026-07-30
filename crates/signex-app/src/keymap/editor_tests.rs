use super::{
    AppCommandId, CommandGroup, KeyStroke, KeymapEditorModel, ShortcutContext, ShortcutProfileSet,
    metadata_for,
};
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
        .edit_active_trigger(
            command.clone(),
            ShortcutContext::Global,
            "Ctrl+Alt+S".into(),
        )
        .unwrap();

    let keymap = editor.active_keymap();
    let lookup = keymap.lookup(
        &[KeyStroke::from_str("Ctrl+Alt+S").unwrap()],
        &[ShortcutContext::Global],
    );
    assert_eq!(lookup.command.as_ref(), Some(&command));
}

#[test]
fn empty_search_query_returns_every_row() {
    let editor = KeymapEditorModel::built_ins().unwrap();
    assert_eq!(editor.filtered_rows("").len(), editor.rows().len());
    // Whitespace-only queries behave like an empty query and never panic.
    assert_eq!(editor.filtered_rows("   ").len(), editor.rows().len());
}

#[test]
fn search_filters_rows_by_label_command_or_trigger() {
    let editor = KeymapEditorModel::built_ins().unwrap();
    let matches = editor.filtered_rows("save");

    assert!(
        !matches.is_empty(),
        "the built-in profile should bind at least one save command"
    );
    // Case-insensitive; every surviving row matches on label, id or trigger.
    for row in &matches {
        let label = row.label.to_lowercase();
        let trigger = row.trigger.to_lowercase();
        let command = row
            .command
            .as_ref()
            .map(|command| command.as_str().to_lowercase())
            .unwrap_or_default();
        assert!(
            label.contains("save") || trigger.contains("save") || command.contains("save"),
            "row `{}` leaked through the `save` filter",
            row.label
        );
    }

    // A query that cannot match anything yields an empty set (no panic).
    assert!(editor.filtered_rows("zzz-no-such-command").is_empty());
}

#[test]
fn rows_carry_the_command_group_from_metadata() {
    let editor = KeymapEditorModel::built_ins().unwrap();
    let rows = editor.rows();
    assert!(!rows.is_empty());

    // Every row's group agrees with its command metadata; rows without
    // metadata fall back to General.
    for row in &rows {
        let expected = row
            .command
            .as_ref()
            .and_then(metadata_for)
            .map(|metadata| metadata.group)
            .unwrap_or(CommandGroup::General);
        assert_eq!(row.group, expected, "row `{}` mis-grouped", row.label);
    }
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

#[test]
fn fresh_editor_does_not_differ_from_its_live_set() {
    let live = ShortcutProfileSet::built_ins().unwrap();
    let editor = KeymapEditorModel::new(live.clone());
    assert!(!editor.differs_from(&live));
}

#[test]
fn valid_trigger_edit_marks_editor_as_differing() {
    let live = ShortcutProfileSet::built_ins().unwrap();
    let mut editor = KeymapEditorModel::new(live.clone());
    editor
        .create_custom_from_active("custom-altium", "Custom Altium")
        .unwrap();
    let command = AppCommandId::new("save_document").unwrap();
    editor
        .edit_active_trigger(command, ShortcutContext::Global, "Ctrl+Alt+S".into())
        .unwrap();

    assert!(editor.differs_from(&live));
}

#[test]
fn rejected_built_in_edit_still_counts_as_differing() {
    // The profile set is untouched (the edit was refused), but the invalid
    // draft is pending user input — closing now would silently drop it, so
    // the comparator must report dirty.
    let live = ShortcutProfileSet::built_ins().unwrap();
    let mut editor = KeymapEditorModel::new(live.clone());
    let command = AppCommandId::new("save_document").unwrap();
    let _ = editor.edit_active_trigger(command, ShortcutContext::Global, "Ctrl+Alt+S".into());

    assert!(editor.has_invalid_trigger_drafts());
    assert!(editor.differs_from(&live));
}

#[test]
fn switching_active_profile_marks_editor_as_differing() {
    let live = ShortcutProfileSet::built_ins().unwrap();
    let mut editor = KeymapEditorModel::new(live.clone());
    let inactive = editor
        .profiles()
        .into_iter()
        .find(|profile| !profile.active)
        .expect("built-ins ship more than one profile");
    editor.set_active_profile(&inactive.id).unwrap();

    assert!(editor.differs_from(&live));
}
