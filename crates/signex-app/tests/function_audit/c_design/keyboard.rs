//! Row 145: Preferences > Keyboard shortcuts. Profiles, the recorder, the
//! conflict banner, import and export. Key changes wait for Apply or OK.

use signex_app::keymap::{BindingSlot, KeyStroke, KeymapEditorRow, ShortcutTrigger};
use signex_app::preferences::{KeymapViewMsg, KeymapViewState, PrefMsg, PrefNav};

use super::support::{Prefs, opened};

fn is_recordable(row: &KeymapEditorRow) -> bool {
    row.keyboard_editable
        && row.command.is_some()
        && row.trigger_valid
        && matches!(
            ShortcutTrigger::parse(&row.trigger_config),
            Ok(ShortcutTrigger::KeySequence(strokes)) if strokes.len() == 1
        )
}

/// Fork the active profile and return its first recordable row.
fn custom_row(prefs: &mut Prefs) -> KeymapEditorRow {
    prefs.send(PrefMsg::KeymapCreateCustomProfile);
    assert!(
        prefs
            .app
            .ui_state
            .preferences_keymap_editor
            .active_profile_is_custom(),
        "fixture: Create makes the custom profile active"
    );
    prefs
        .app
        .ui_state
        .preferences_keymap_editor
        .rows_in(signex_i18n::I18n::en_us_ref())
        .into_iter()
        .find(is_recordable)
        .expect("a recordable row")
}

fn row_at(prefs: &Prefs, slot: BindingSlot) -> KeymapEditorRow {
    prefs
        .app
        .ui_state
        .preferences_keymap_editor
        .rows_in(signex_i18n::I18n::en_us_ref())
        .into_iter()
        .find(|row| row.slot == slot)
        .expect("the row")
}

fn open_recorder(prefs: &mut Prefs, row: &KeymapEditorRow) {
    prefs.send(PrefMsg::KeymapRecorderOpen {
        slot: row.slot,
        command: row.command.clone().expect("a command row"),
        label: row.label.clone(),
        context: row.context,
        trigger: row.trigger_config.clone(),
    });
}

fn record(prefs: &mut Prefs, stroke: &KeyStroke) {
    prefs.send(PrefMsg::KeymapRecorderStart);
    prefs.send(PrefMsg::KeymapRecorderKeyPressed(stroke.clone()));
    prefs.send(PrefMsg::KeymapRecorderStop);
}

fn stroke() -> KeyStroke {
    "Ctrl+Alt+K".parse::<KeyStroke>().expect("a keystroke")
}

fn trigger_of(row: &KeymapEditorRow) -> ShortcutTrigger {
    ShortcutTrigger::parse(&row.trigger_config).expect("a valid trigger")
}

fn profile_count(prefs: &Prefs) -> usize {
    prefs
        .app
        .ui_state
        .preferences_keymap_editor
        .profiles()
        .len()
}

fn active_id(prefs: &Prefs) -> String {
    prefs
        .app
        .ui_state
        .preferences_keymap_editor
        .active_profile()
        .id
        .clone()
}

#[test]
fn create_makes_a_custom_copy_as_an_unsaved_change_of_this_page() {
    let mut prefs = opened();
    let count = profile_count(&prefs);

    prefs.send(PrefMsg::KeymapCreateCustomProfile);

    assert_eq!(profile_count(&prefs), count + 1);
    assert!(
        prefs
            .app
            .ui_state
            .preferences_keymap_editor
            .active_profile_is_custom()
    );
    assert_eq!(prefs.dirty_pages(), [PrefNav::Keyboard]);
}

#[test]
fn picking_another_profile_is_a_draft_until_apply() {
    let mut prefs = opened();
    let saved = prefs
        .app
        .ui_state
        .keymap_profiles
        .active_profile()
        .id
        .clone();
    let other = prefs
        .app
        .ui_state
        .preferences_keymap_editor
        .profiles()
        .into_iter()
        .find(|profile| !profile.active)
        .expect("a second built-in profile")
        .id;

    prefs.send(PrefMsg::KeymapProfileSelected(other.clone()));

    assert_eq!(active_id(&prefs), other);
    assert_eq!(
        prefs.app.ui_state.keymap_profiles.active_profile().id,
        saved
    );
    assert!(prefs.is_dirty());

    prefs.apply();

    assert_eq!(
        prefs.app.ui_state.keymap_profiles.active_profile().id,
        other
    );
}

#[test]
fn picking_a_profile_that_does_not_exist_is_refused_and_says_so() {
    let mut prefs = opened();
    let before = active_id(&prefs);

    prefs.send(PrefMsg::KeymapProfileSelected(
        "no-such-profile".to_string(),
    ));

    assert_eq!(active_id(&prefs), before);
    assert!(
        prefs
            .app
            .ui_state
            .preferences_keymap_status
            .starts_with("Could not switch profile"),
        "{}",
        prefs.app.ui_state.preferences_keymap_status
    );
    assert!(!prefs.is_dirty());
}

#[test]
fn delete_removes_a_custom_profile() {
    let mut prefs = opened();
    let count = profile_count(&prefs);
    prefs.send(PrefMsg::KeymapCreateCustomProfile);

    prefs.send(PrefMsg::KeymapDeleteActiveProfile);

    assert_eq!(profile_count(&prefs), count);
    assert!(
        !prefs
            .app
            .ui_state
            .preferences_keymap_editor
            .active_profile_is_custom()
    );
}

#[test]
fn delete_is_refused_on_a_built_in_profile() {
    let mut prefs = opened();
    assert!(
        !prefs
            .app
            .ui_state
            .preferences_keymap_editor
            .active_profile_is_custom(),
        "fixture: a built-in is active"
    );
    let count = profile_count(&prefs);

    prefs.send(PrefMsg::KeymapDeleteActiveProfile);

    assert_eq!(profile_count(&prefs), count);
    assert!(!prefs.is_dirty());
}

#[test]
fn the_recorder_ok_rebinds_the_row_as_a_draft_and_apply_makes_it_live() {
    let mut prefs = opened();
    let row = custom_row(&mut prefs);
    open_recorder(&mut prefs, &row);
    record(&mut prefs, &stroke());

    prefs.send(PrefMsg::KeymapRecorderApply);

    assert!(prefs.app.ui_state.preferences_keymap_recorder.is_none());
    let wanted = ShortcutTrigger::KeySequence(vec![stroke()]);
    assert_eq!(trigger_of(&row_at(&prefs, row.slot)), wanted);
    assert_ne!(
        prefs.app.ui_state.keymap_profiles,
        *prefs.app.ui_state.preferences_keymap_editor.profile_set(),
        "not live before Apply"
    );

    prefs.apply();

    assert_eq!(
        prefs.app.ui_state.keymap_profiles,
        *prefs.app.ui_state.preferences_keymap_editor.profile_set()
    );
    assert!(!prefs.is_dirty());
    assert_eq!(
        prefs.app.ui_state.preferences_keymap_status,
        "Keyboard shortcuts saved."
    );
}

#[test]
fn the_recorder_cancel_leaves_the_binding_as_it_was() {
    let mut prefs = opened();
    let row = custom_row(&mut prefs);
    open_recorder(&mut prefs, &row);
    record(&mut prefs, &stroke());

    prefs.send(PrefMsg::KeymapRecorderCancel);

    assert!(prefs.app.ui_state.preferences_keymap_recorder.is_none());
    assert_eq!(row_at(&prefs, row.slot).trigger_config, row.trigger_config);
}

#[test]
fn the_recorder_ignores_a_key_while_it_is_stopped() {
    let mut prefs = opened();
    let row = custom_row(&mut prefs);
    open_recorder(&mut prefs, &row);
    prefs.send(PrefMsg::KeymapRecorderStop);
    let opened_with = prefs
        .app
        .ui_state
        .preferences_keymap_recorder
        .as_ref()
        .expect("the recorder")
        .strokes
        .clone();

    prefs.send(PrefMsg::KeymapRecorderKeyPressed(stroke()));

    assert_eq!(
        prefs
            .app
            .ui_state
            .preferences_keymap_recorder
            .as_ref()
            .expect("the recorder")
            .strokes,
        opened_with
    );
}

#[test]
fn the_recorder_ok_with_no_keystroke_is_refused_and_stays_open() {
    let mut prefs = opened();
    let row = custom_row(&mut prefs);
    open_recorder(&mut prefs, &row);
    prefs.send(PrefMsg::KeymapRecorderClear);

    prefs.send(PrefMsg::KeymapRecorderApply);

    assert!(prefs.app.ui_state.preferences_keymap_recorder.is_some());
    assert_eq!(
        prefs.app.ui_state.preferences_keymap_status,
        "Record at least one keystroke before applying."
    );
    assert_eq!(row_at(&prefs, row.slot).trigger_config, row.trigger_config);
}

#[test]
fn cancel_on_the_dialog_drops_a_recorded_binding_and_the_custom_profile() {
    let mut prefs = opened();
    let count = profile_count(&prefs);
    let saved = prefs.app.ui_state.keymap_profiles.clone();
    let row = custom_row(&mut prefs);
    open_recorder(&mut prefs, &row);
    record(&mut prefs, &stroke());
    prefs.send(PrefMsg::KeymapRecorderApply);

    prefs.cancel();

    assert_eq!(prefs.app.ui_state.keymap_profiles, saved);
    assert_eq!(profile_count(&prefs), count);
    assert!(!prefs.is_dirty());
}

#[test]
fn binding_a_key_another_command_uses_raises_a_conflict_and_show_filters_to_it() {
    let mut prefs = opened();
    let row = custom_row(&mut prefs);
    let taken = prefs
        .app
        .ui_state
        .preferences_keymap_editor
        .rows_in(signex_i18n::I18n::en_us_ref())
        .into_iter()
        .find(|other| {
            is_recordable(other)
                && other.slot != row.slot
                && other.context == row.context
                && other.command != row.command
                && other.trigger_config != row.trigger_config
        })
        .expect("another command in the same context");
    let before = prefs
        .app
        .ui_state
        .preferences_keymap_editor
        .active_conflicts()
        .len();
    let ShortcutTrigger::KeySequence(strokes) = trigger_of(&taken) else {
        panic!("a key row");
    };
    open_recorder(&mut prefs, &row);
    record(&mut prefs, &strokes[0]);
    prefs.send(PrefMsg::KeymapRecorderApply);

    assert!(
        prefs
            .app
            .ui_state
            .preferences_keymap_editor
            .active_conflicts()
            .len()
            > before
    );

    prefs.send(PrefMsg::KeymapView(KeymapViewMsg::ConflictsShown));

    assert!(
        prefs
            .app
            .ui_state
            .preferences_pages
            .keymap_view
            .shows_conflicts()
    );
}

#[test]
fn the_context_chips_and_the_search_are_not_unsaved_changes() {
    let mut prefs = opened();
    let all = KeymapViewState::default().context_filter;

    prefs.send(PrefMsg::KeymapView(KeymapViewMsg::ConflictsShown));
    prefs.send(PrefMsg::KeymapView(KeymapViewMsg::ContextFilterPicked(all)));
    prefs.send(PrefMsg::KeymapSearchChanged("paste".to_string()));

    assert_eq!(
        prefs.app.ui_state.preferences_pages.keymap_view,
        KeymapViewState::default()
    );
    assert_eq!(prefs.app.ui_state.preferences_keymap_search, "paste");
    assert!(!prefs.is_dirty());
}

#[test]
fn importing_a_file_that_is_no_profile_is_refused_and_changes_nothing() {
    let mut prefs = opened();
    let count = profile_count(&prefs);

    prefs.send(PrefMsg::KeymapProfileLoaded("not a profile".to_string()));

    assert_eq!(profile_count(&prefs), count);
    assert!(
        prefs
            .app
            .ui_state
            .preferences_keymap_status
            .starts_with("Could not import keyboard shortcuts"),
        "{}",
        prefs.app.ui_state.preferences_keymap_status
    );
    assert!(!prefs.is_dirty());
}

#[test]
fn an_exported_custom_profile_imports_back() {
    let mut prefs = opened();
    let count = profile_count(&prefs);
    prefs.send(PrefMsg::KeymapCreateCustomProfile);
    let id = active_id(&prefs);
    let source = signex_app::keymap::export_custom_profile(
        prefs
            .app
            .ui_state
            .preferences_keymap_editor
            .active_profile(),
    )
    .expect("a custom profile exports");
    prefs.send(PrefMsg::KeymapDeleteActiveProfile);
    assert_eq!(profile_count(&prefs), count);

    prefs.send(PrefMsg::KeymapProfileLoaded(source));

    assert_eq!(profile_count(&prefs), count + 1);
    assert!(
        prefs
            .app
            .ui_state
            .preferences_keymap_editor
            .profiles()
            .iter()
            .any(|profile| profile.id == id)
    );
    assert_eq!(
        prefs.app.ui_state.preferences_keymap_status,
        "Imported a custom keyboard shortcut profile."
    );
}

#[test]
fn the_export_result_is_reported_on_the_page() {
    let mut prefs = opened();
    let path = std::path::PathBuf::from("shortcuts.toml");

    prefs.send(PrefMsg::KeymapExportFinished(Ok(path.clone())));
    assert_eq!(
        prefs.app.ui_state.preferences_keymap_status,
        format!("Exported keyboard shortcuts to {}.", path.display())
    );

    prefs.send(PrefMsg::KeymapExportFinished(Err("disk full".to_string())));
    assert_eq!(
        prefs.app.ui_state.preferences_keymap_status,
        "Could not export keyboard shortcuts: disk full"
    );
}
