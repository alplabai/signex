use super::{
    AppCommandId, BindingSlot, CommandGroup, KeyStroke, KeymapEditorModel, ShortcutContext,
    ShortcutProfileSet, metadata_for,
};
use std::str::FromStr;

/// Slot of the row bound to `command` with display text `trigger`.
fn slot_of(editor: &KeymapEditorModel, command: &str, trigger: &str) -> BindingSlot {
    editor
        .rows()
        .into_iter()
        .find(|row| {
            row.command.as_ref().map(AppCommandId::as_str) == Some(command)
                && row.trigger == trigger
        })
        .unwrap_or_else(|| panic!("no row `{command}` bound to `{trigger}`"))
        .slot
}

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

    let slot = slot_of(&editor, "save_document", "Ctrl+S");
    editor
        .edit_active_trigger(slot, "Ctrl+Alt+S".into())
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
    let slot = slot_of(&editor, "save_document", "Ctrl+S");

    let error = editor
        .edit_active_trigger(slot, "Ctrl+Alt+S".into())
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
    let slot = slot_of(&editor, "save_document", "Ctrl+S");
    editor
        .edit_active_trigger(slot, "Ctrl+Alt+S".into())
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
    let slot = slot_of(&editor, "save_document", "Ctrl+S");
    let _ = editor.edit_active_trigger(slot, "Ctrl+Alt+S".into());

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

// #1378 — altium.toml binds `place_wire` twice in the global context
// (`P W` and `Ctrl+W`). Editing one row used to rewrite the last binding
// for the command+context, and the drafts were shared between the rows.
#[test]
fn editing_one_of_two_bindings_for_a_command_edits_only_that_one() {
    let mut editor = KeymapEditorModel::built_ins().unwrap();
    editor
        .create_custom_from_active("custom-altium", "Custom Altium")
        .unwrap();
    let first = slot_of(&editor, "place_wire", "P W");
    let second = slot_of(&editor, "place_wire", "Ctrl+W");
    assert_ne!(first, second);

    editor
        .edit_active_trigger(second, "Ctrl+Alt+W".into())
        .unwrap();

    let triggers = |editor: &KeymapEditorModel| {
        editor
            .rows()
            .into_iter()
            .filter(|row| row.command.as_ref().map(AppCommandId::as_str) == Some("place_wire"))
            .map(|row| row.trigger)
            .collect::<Vec<_>>()
    };
    let mut now = triggers(&editor);
    now.sort();
    assert_eq!(now, ["Ctrl+Alt+W", "P W"]);
}

#[test]
fn an_invalid_draft_marks_only_its_own_row() {
    let mut editor = KeymapEditorModel::built_ins().unwrap();
    editor
        .create_custom_from_active("custom-altium", "Custom Altium")
        .unwrap();
    let first = slot_of(&editor, "place_wire", "P W");
    let second = slot_of(&editor, "place_wire", "Ctrl+W");

    assert!(editor.edit_active_trigger(first, "Ctrl+".into()).is_err());

    let invalid = |slot| {
        editor
            .rows()
            .into_iter()
            .find(|row| row.slot == slot)
            .map(|row| !row.trigger_valid)
            .unwrap()
    };
    assert!(invalid(first));
    assert!(!invalid(second), "the sibling row must not share the draft");
}

#[test]
fn editing_a_slot_that_no_longer_exists_is_an_error() {
    let mut editor = KeymapEditorModel::built_ins().unwrap();
    editor
        .create_custom_from_active("custom-altium", "Custom Altium")
        .unwrap();
    let gone = BindingSlot {
        binding: usize::MAX,
        trigger: 0,
    };
    assert!(
        editor
            .edit_active_trigger(gone, "Ctrl+Alt+W".into())
            .is_err()
    );
}

// #1378 — an `Unbind` with no `Command` row to cancel (a trigger this
// profile never bound, or another context) stays listed, flagged unbound,
// so Preferences can still show it.
#[test]
fn an_unbind_with_no_matching_row_stays_listed_as_unbound() {
    use super::{
        ShortcutBinding, ShortcutBindingAction, ShortcutProfile, ShortcutProfileKind,
        ShortcutTrigger,
    };
    let command = AppCommandId::new("save_document").unwrap();
    let profile = ShortcutProfile {
        id: "t".to_string(),
        name: "T".to_string(),
        kind: ShortcutProfileKind::Custom,
        schema_version: 1,
        description: None,
        base_profile: None,
        bindings: vec![
            ShortcutBinding {
                action: ShortcutBindingAction::Unbind(command.clone()),
                context: ShortcutContext::Global,
                triggers: vec![ShortcutTrigger::parse("Ctrl+S").unwrap()],
            },
            ShortcutBinding {
                action: ShortcutBindingAction::Command(command),
                context: ShortcutContext::Global,
                triggers: vec![ShortcutTrigger::parse("F2").unwrap()],
            },
        ],
    };
    let editor = KeymapEditorModel::from(profile);
    let rows = editor.rows();
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[0].trigger, "Ctrl+S");
    assert!(rows[0].unbound, "the orphan unbind is flagged: {rows:?}");
    assert_eq!(rows[0].slot.binding, 0);
    assert_eq!(rows[1].trigger, "F2");
    assert!(!rows[1].unbound);
    // The slot addresses the real binding index.
    assert_eq!(rows[1].slot.binding, 1);
}

// A `Command` row that a later `Unbind` (same command, context, trigger)
// shadows does not work, so the editor must not present it as live.
#[test]
fn a_command_row_shadowed_by_a_later_unbind_is_flagged_unbound() {
    use super::{
        ShortcutBinding, ShortcutBindingAction, ShortcutProfile, ShortcutProfileKind,
        ShortcutTrigger,
    };
    let command = AppCommandId::new("print").unwrap();
    let binding = |action, trigger: &str| ShortcutBinding {
        action,
        context: ShortcutContext::Global,
        triggers: vec![ShortcutTrigger::parse(trigger).unwrap()],
    };
    let profile = ShortcutProfile {
        id: "t".to_string(),
        name: "T".to_string(),
        kind: ShortcutProfileKind::Custom,
        schema_version: 1,
        description: None,
        base_profile: None,
        bindings: vec![
            binding(ShortcutBindingAction::Command(command.clone()), "Ctrl+P"),
            binding(ShortcutBindingAction::Command(command.clone()), "F7"),
            binding(ShortcutBindingAction::Unbind(command), "Ctrl+P"),
        ],
    };
    let rows = KeymapEditorModel::from(profile).rows();
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert!(rows[0].unbound, "Ctrl+P is shadowed: {rows:?}");
    assert!(!rows[1].unbound, "F7 is live: {rows:?}");
}

// The unbind that cancels a listed row is that row's `unbound` flag, not a
// second row.
#[test]
fn an_unbind_that_cancels_a_listed_row_is_not_a_row_of_its_own() {
    use super::{
        ShortcutBinding, ShortcutBindingAction, ShortcutProfile, ShortcutProfileKind,
        ShortcutTrigger,
    };
    let command = AppCommandId::new("save_document").unwrap();
    let binding = |action, trigger: &str| ShortcutBinding {
        action,
        context: ShortcutContext::Global,
        triggers: vec![ShortcutTrigger::parse(trigger).unwrap()],
    };
    let profile = ShortcutProfile {
        id: "t".to_string(),
        name: "T".to_string(),
        kind: ShortcutProfileKind::Custom,
        schema_version: 1,
        description: None,
        base_profile: None,
        bindings: vec![
            binding(ShortcutBindingAction::Command(command.clone()), "Ctrl+S"),
            binding(ShortcutBindingAction::Unbind(command), "Ctrl+S"),
        ],
    };
    let rows = KeymapEditorModel::from(profile).rows();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert!(rows[0].unbound);
}
