use super::*;
use crate::keymap::KeymapEditorModel;

#[test]
fn classic_f1_and_escape_bindings_are_flagged_overridden() {
    let mut editor = KeymapEditorModel::built_ins().unwrap();
    editor.set_active_profile("classic").unwrap();
    let flagged = |command: &str| {
        editor
            .rows()
            .iter()
            .filter(|row| row.command.as_ref().map(|c| c.as_str()) == Some(command))
            .map(|row| shadow_note(signex_i18n::I18n::en_us_ref(), &row.trigger))
            .collect::<Vec<_>>()
    };
    // Classic binds F1 to zoom_in_at_cursor and Escape to cancel_current_tool.
    assert_eq!(flagged("zoom_in_at_cursor").iter().flatten().count(), 1);
    assert!(flagged("cancel_current_tool").iter().all(Option::is_some));
    assert!(!flagged("cancel_current_tool").is_empty());
    assert!(shadow_note(signex_i18n::I18n::en_us_ref(), "Ctrl+F1").is_some());
    assert_eq!(shadow_note(signex_i18n::I18n::en_us_ref(), "P W"), None);
    assert_eq!(
        shadow_note(signex_i18n::I18n::en_us_ref(), "DoubleClick"),
        None
    );
}

#[test]
fn recorder_warns_only_for_shadowed_strokes() {
    assert!(recorder_shadow_warning("Esc").is_some());
    assert!(recorder_shadow_warning("X Esc").is_some());
    assert!(recorder_shadow_warning("Ctrl+K").is_none());
}

fn editor_with(platform: crate::keymap::Platform, bindings: &[(&str, &str)]) -> KeymapEditorModel {
    use crate::keymap::{
        AppCommandId, ShortcutBinding, ShortcutBindingAction, ShortcutContext, ShortcutProfile,
        ShortcutProfileKind, ShortcutTrigger,
    };
    KeymapEditorModel::from(ShortcutProfile {
        id: "t".to_string(),
        name: "T".to_string(),
        kind: ShortcutProfileKind::Custom,
        schema_version: 1,
        description: None,
        base_profile: None,
        bindings: bindings
            .iter()
            .map(|(trigger, id)| ShortcutBinding {
                action: ShortcutBindingAction::Command(AppCommandId::new(*id).unwrap()),
                context: ShortcutContext::Global,
                triggers: vec![ShortcutTrigger::parse(trigger).unwrap()],
                platform: None,
            })
            .collect(),
    })
    .with_platform(platform)
}

/// `Mod+K` and a literal key that lands on the same stroke are one
/// conflict, whatever each row's text says (m1).
#[test]
fn a_mod_row_and_a_literal_row_on_one_key_both_show_the_conflict() {
    use crate::keymap::Platform;
    // The second row is typed as a draft in a spelling that is neither
    // `Mod+K` nor how the row is shown, so only a stroke comparison sees
    // that it is the same key.
    for (platform, typed) in [
        (Platform::MacOs, "Cmd+K"),
        (Platform::Windows, "Control+K"),
        (Platform::Linux, "Control+K"),
    ] {
        let mut editor = editor_with(platform, &[("Mod+K", "find"), ("F9", "open_preferences")]);
        let slot = editor.rows()[1].slot;
        editor.edit_active_trigger(slot, typed.to_string()).unwrap();
        let conflicts = editor.active_conflicts();
        assert_eq!(conflicts.len(), 1, "{platform:?}");
        for row in editor.rows() {
            assert_eq!(
                row_conflict_kinds(&row, &conflicts, platform),
                (true, false),
                "{platform:?} {}",
                row.trigger
            );
        }
    }
}

/// m2: a chord macOS owns is a conflict marked as the system's.
#[test]
fn a_macos_reserved_chord_is_marked_reserved_on_its_row() {
    use crate::keymap::Platform;
    let editor = editor_with(
        Platform::MacOs,
        &[("Mod+Q", "cycle_unit"), ("Mod+S", "save_document")],
    );
    let conflicts = editor.active_conflicts();
    let kinds: Vec<_> = editor
        .rows()
        .iter()
        .map(|row| row_conflict_kinds(row, &conflicts, Platform::MacOs))
        .collect();
    assert_eq!(kinds, [(true, true), (false, false)]);
    let elsewhere = editor_with(Platform::Windows, &[("Mod+Q", "cycle_unit")]);
    assert!(elsewhere.active_conflicts().is_empty());
}

// ─── #145: chips, sort, states, Show, Delete and the recorder popover ───

/// A custom profile on Windows whose bindings sit in the given contexts.
fn editor_in_contexts(
    bindings: &[(&str, &str, crate::keymap::ShortcutContext)],
) -> KeymapEditorModel {
    use crate::keymap::{
        AppCommandId, ShortcutBinding, ShortcutBindingAction, ShortcutProfile, ShortcutProfileKind,
        ShortcutTrigger,
    };
    KeymapEditorModel::from(ShortcutProfile {
        id: "t".to_string(),
        name: "T".to_string(),
        kind: ShortcutProfileKind::Custom,
        schema_version: 1,
        description: None,
        base_profile: None,
        bindings: bindings
            .iter()
            .map(|(trigger, id, context)| ShortcutBinding {
                action: ShortcutBindingAction::Command(AppCommandId::new(*id).unwrap()),
                context: *context,
                triggers: vec![ShortcutTrigger::parse(trigger).unwrap()],
                platform: None,
            })
            .collect(),
    })
    .with_platform(crate::keymap::Platform::Windows)
}

fn shown(editor: &KeymapEditorModel, filter: KeymapContextFilter) -> Vec<String> {
    let platform = crate::keymap::Platform::Windows;
    let conflicts = editor.active_conflicts();
    filters::visible_rows(
        editor,
        signex_i18n::I18n::en_us_ref(),
        "",
        filter,
        &conflicts,
        platform,
    )
    .into_iter()
    .map(|row| {
        row.command
            .map(|c| c.as_str().to_string())
            .unwrap_or_default()
    })
    .collect()
}

#[test]
fn each_context_chip_shows_only_the_rows_of_its_context() {
    use crate::keymap::ShortcutContext as C;
    let editor = editor_in_contexts(&[
        ("Ctrl+F", "find", C::Global),
        ("P W", "place_wire", C::Schematic),
        ("Shift+R", "toggle_ratsnest", C::Pcb),
        ("F9", "open_preferences", C::Footprint),
        ("F10", "cycle_unit", C::Library),
        ("F11", "save_document", C::Modal),
    ]);
    assert_eq!(shown(&editor, KeymapContextFilter::Global), ["find"]);
    assert_eq!(
        shown(&editor, KeymapContextFilter::Schematic),
        ["place_wire"]
    );
    assert_eq!(
        shown(&editor, KeymapContextFilter::Pcb),
        ["toggle_ratsnest"]
    );
    assert_eq!(
        shown(&editor, KeymapContextFilter::Footprint),
        ["open_preferences"]
    );
    assert_eq!(shown(&editor, KeymapContextFilter::Library), ["cycle_unit"]);
    // A dialog binding has no chip of its own: it shows under All only.
    assert_eq!(shown(&editor, KeymapContextFilter::All).len(), 6);
    for chip in KeymapContextFilter::ALL {
        if chip != KeymapContextFilter::All {
            assert!(!shown(&editor, chip).contains(&"save_document".to_string()));
        }
    }
}

#[test]
fn the_conflicts_chip_lists_exactly_the_conflicting_rows() {
    use crate::keymap::ShortcutContext as C;
    let editor = editor_in_contexts(&[
        ("Ctrl+Shift+V", "find", C::Global),
        ("F9", "open_preferences", C::Global),
        ("Ctrl+Shift+V", "save_document", C::Global),
        // The same stroke in another context is not a clash.
        ("Ctrl+Shift+V", "cycle_unit", C::Pcb),
    ]);
    assert_eq!(editor.active_conflicts().len(), 1);
    let mut conflicting = shown(&editor, KeymapContextFilter::Conflicts);
    conflicting.sort();
    assert_eq!(conflicting, ["find", "save_document"]);

    let clean = editor_in_contexts(&[("F9", "open_preferences", C::Global)]);
    assert!(shown(&clean, KeymapContextFilter::Conflicts).is_empty());
}

#[test]
fn rows_are_sorted_by_command_ascending() {
    use crate::keymap::ShortcutContext as C;
    let editor = editor_in_contexts(&[
        ("Ctrl+S", "save_document", C::Global),
        ("Ctrl+F", "find", C::Global),
        ("F9", "open_preferences", C::Global),
    ]);
    let conflicts = editor.active_conflicts();
    let labels: Vec<String> = filters::visible_rows(
        &editor,
        signex_i18n::I18n::en_us_ref(),
        "",
        KeymapContextFilter::All,
        &conflicts,
        crate::keymap::Platform::Windows,
    )
    .into_iter()
    .map(|row| row.label)
    .collect();
    let mut sorted = labels.clone();
    signex_i18n::I18n::en_us_ref().sort_for_display(&mut sorted, |label| label.as_str());
    assert_eq!(labels, sorted);
    assert_eq!(labels.len(), 3);
}

#[test]
fn the_search_and_the_chip_both_narrow_the_rows() {
    use crate::keymap::ShortcutContext as C;
    let editor = editor_in_contexts(&[("Ctrl+F", "find", C::Global), ("F", "find", C::Schematic)]);
    let conflicts = editor.active_conflicts();
    let rows = filters::visible_rows(
        &editor,
        signex_i18n::I18n::en_us_ref(),
        "ctrl",
        KeymapContextFilter::Schematic,
        &conflicts,
        crate::keymap::Platform::Windows,
    );
    assert!(rows.is_empty(), "Ctrl+F is global, F has no Ctrl");
}

#[test]
fn row_states_follow_conflicts_unbinds_and_the_profile_kind() {
    use crate::keymap::{Platform, ShortcutContext as C};
    use filters::RowState;
    let editor = editor_in_contexts(&[
        ("Ctrl+K", "find", C::Global),
        ("Ctrl+K", "save_document", C::Global),
        ("F9", "open_preferences", C::Global),
    ]);
    let conflicts = editor.active_conflicts();
    let states: Vec<RowState> = editor
        .rows()
        .iter()
        .map(|row| RowState::of(row, &conflicts, Platform::Windows, true))
        .collect();
    assert_eq!(
        states,
        [RowState::Conflict, RowState::Conflict, RowState::Editable]
    );
    let read_only: Vec<RowState> = editor
        .rows()
        .iter()
        .map(|row| RowState::of(row, &[], Platform::Windows, false))
        .collect();
    assert!(read_only.iter().all(|s| *s == RowState::CreateCustom));
}

#[test]
fn show_in_the_banner_turns_on_the_conflicts_chip() {
    let mut view = KeymapViewState::default();
    assert_eq!(view.context_filter, KeymapContextFilter::All);
    assert!(!view.shows_conflicts());
    view.apply(KeymapViewMsg::ConflictsShown);
    assert!(view.shows_conflicts());
    view.apply(KeymapViewMsg::ContextFilterPicked(
        KeymapContextFilter::Schematic,
    ));
    assert_eq!(view.context_filter, KeymapContextFilter::Schematic);
    assert!(!view.shows_conflicts());
}

#[test]
fn delete_is_disabled_for_built_in_profiles() {
    let mut editor = KeymapEditorModel::built_ins().unwrap();
    assert!(!profile_row::delete_enabled(&editor));
    editor
        .create_custom_from_active("custom-1", "Custom 1")
        .unwrap();
    assert!(profile_row::delete_enabled(&editor));
}

#[test]
fn profile_entries_name_the_kind_and_the_binding_count() {
    let i18n = signex_i18n::I18n::en_us_ref();
    let mut editor = KeymapEditorModel::built_ins().unwrap();
    editor
        .create_custom_from_active("custom-1", "Mine")
        .unwrap();
    for profile in editor.profiles() {
        let option = profile_row::KeymapProfileOption::new(i18n, &profile);
        let count = format!("{} bindings", i18n.count(profile.binding_count as i64));
        assert!(option.label.contains(&count), "{}", option.label);
        assert_eq!(
            option.label.contains("(custom)"),
            profile.kind == crate::keymap::ShortcutProfileKind::Custom,
            "{}",
            option.label
        );
        assert!(option.label.starts_with(&profile.name), "{}", option.label);
    }
}

#[test]
fn only_custom_keyboard_rows_offer_edit() {
    let custom = editor_with(crate::keymap::Platform::Windows, &[("Ctrl+K", "find")]);
    let row = &custom.rows()[0];
    assert!(matches!(
        table::edit_message(row, true),
        Some(PrefMsg::KeymapRecorderOpen { slot, .. }) if slot == row.slot
    ));
    assert!(table::edit_message(row, false).is_none());
}

#[test]
fn the_recorder_popover_keeps_clear_stop_cancel_and_ok() {
    use signex_i18n::shortcut_ids as sids;
    let [clear, toggle, cancel, ok] = recorder::recorder_actions(true);
    assert_eq!(clear.label, sids::PREF_KEYMAP_CLEAR);
    assert!(matches!(clear.message, PrefMsg::KeymapRecorderClear));
    assert_eq!(toggle.label, sids::PREF_KEYMAP_STOP);
    assert!(matches!(toggle.message, PrefMsg::KeymapRecorderStop));
    assert_eq!(cancel.label, sids::PREF_KEYMAP_CANCEL);
    assert!(matches!(cancel.message, PrefMsg::KeymapRecorderCancel));
    assert_eq!(ok.label, sids::PREF_KEYMAP_OK);
    assert!(matches!(ok.message, PrefMsg::KeymapRecorderApply));
    assert!(ok.is_primary);
    assert!(!clear.is_primary && !toggle.is_primary && !cancel.is_primary);

    // Once stopped, the same place records again.
    let [_, record, _, _] = recorder::recorder_actions(false);
    assert_eq!(record.label, sids::PREF_KEYMAP_RECORD);
    assert!(matches!(record.message, PrefMsg::KeymapRecorderStart));
}
