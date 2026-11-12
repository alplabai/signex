use super::*;
use crate::app::Signex;
use crate::config_root::isolated::IsolatedConfigRoot;
use crate::keymap::{
    AppCommandId, ShortcutProfileKind, ShortcutTrigger, TomlShortcutProfile, migrate_active_to_mod,
};

const LEGACY: &str = r#"
[signex_settings]
application = "signex"
file_kind = "keyboard_shortcuts"
version = 1

[keyboard_shortcuts]
schema_version = 1
profile_id = "mine"
profile_name = "Mine"
profile_kind = "custom"

[[keyboard_shortcuts.sections]]
context = "global"

[keyboard_shortcuts.sections.bindings]
"Ctrl+S" = "save_document"
"Ctrl+Q" = "cycle_unit"
"#;

fn legacy_set() -> ShortcutProfileSet {
    let profile = TomlShortcutProfile::parse(LEGACY)
        .unwrap()
        .into_profile()
        .unwrap();
    assert_eq!(profile.kind, ShortcutProfileKind::Custom);
    let mut set = ShortcutProfileSet::built_ins().unwrap();
    set.insert_custom_profile(profile).unwrap();
    set.set_active_profile("mine").unwrap();
    set
}

fn asked(json: &str) -> serde_json::Value {
    serde_json::from_str(json).unwrap()
}

fn written(set: ShortcutProfileSet, source: ShortcutProfileSet) -> KeymapModMigration {
    KeymapModMigration {
        source,
        set,
        skipped: Vec::new(),
        backup: Some("keyboard_shortcuts.toml.bak".to_string()),
    }
}

fn deliver(app: &mut Signex, result: Result<KeymapModMigration, String>) -> Task<Message> {
    app.update(Message::Ui(UiMsg::KeymapModMigrationWritten(result)))
}

fn app_with_legacy_profile() -> Signex {
    let (mut app, _boot) = Signex::new();
    app.ui_state.set_keymap_profiles(legacy_set());
    app
}

#[test]
fn the_prompt_is_built_only_on_macos_for_a_custom_profile_not_asked_before() {
    let set = legacy_set();
    assert_eq!(
        keymap_mod_prompt_boot(&set, None, Platform::MacOs).units(),
        1
    );
    for other in [Platform::Windows, Platform::Linux] {
        assert_eq!(keymap_mod_prompt_boot(&set, None, other).units(), 0);
    }
    let built_in = ShortcutProfileSet::built_ins().unwrap();
    assert_eq!(
        keymap_mod_prompt_boot(&built_in, None, Platform::MacOs).units(),
        0
    );
}

#[test]
fn an_answered_prompt_is_never_built_again() {
    let prefs = asked(r#"{"keymap_mod_prompt_asked": true}"#);
    assert_eq!(
        keymap_mod_prompt_boot(&legacy_set(), Some(&prefs), Platform::MacOs).units(),
        0
    );
}

/// `Signex::new` is what a headless boot starts from, so it must not carry
/// the prompt: a legacy custom profile boots to the same task as no profile.
/// (The prompt is macOS-only, so this only bites on a Mac host.)
#[test]
fn a_boot_that_is_not_the_gui_boot_carries_no_prompt() {
    let units_with = {
        let _root = IsolatedConfigRoot::new();
        crate::keymap::save_profile_set(&legacy_set()).unwrap();
        let (app, boot) = Signex::new();
        // Positive control: the setup really loaded the legacy custom profile.
        assert!(crate::keymap::offers_mod_migration(
            &app.ui_state.keymap_profiles,
            Platform::MacOs
        ));
        boot.units()
    };
    let units_without = {
        let _root = IsolatedConfigRoot::new();
        let (_app, boot) = Signex::new();
        boot.units()
    };
    assert_eq!(units_with, units_without);
}

/// The GUI boot on a Mac adds exactly the prompt unit; no task runs, so no
/// dialog opens. This guards Linux CI, where `Platform::current()` never asks.
#[test]
fn a_gui_boot_on_macos_adds_the_prompt_unit_and_on_windows_does_not() {
    let _root = IsolatedConfigRoot::new();
    crate::keymap::save_profile_set(&legacy_set()).unwrap();
    let recovery = tempfile::tempdir().unwrap();
    let units_on = |platform: Platform| {
        let (mut app, boot) = Signex::new();
        // `new` takes the real recovery folder; `boot_from` must not touch it.
        app.document_state
            .autosave
            .set_dir(Some(recovery.path().to_path_buf()));
        Signex::boot_from_on(app, boot, Vec::new(), platform)
            .1
            .units()
    };
    assert_eq!(units_on(Platform::MacOs), units_on(Platform::Windows) + 1);
}

#[test]
fn use_command_backs_the_file_up_then_writes_the_moved_rows() {
    let _root = IsolatedConfigRoot::new();
    let path = crate::keymap::config_path().unwrap();
    crate::keymap::save_profile_set(&legacy_set()).unwrap();
    let before = std::fs::read_to_string(&path).unwrap();
    let moved = migrate_active_to_mod(&legacy_set()).unwrap();

    let done = write_migration(legacy_set(), moved).unwrap();

    let backup = done.backup.expect("an existing file is backed up first");
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), before);
    assert_eq!(done.source, legacy_set());
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("Mod+S") && !text.contains("Ctrl+S"), "{text}");
    assert!(text.contains("Ctrl+Q"), "collision stays: {text}");
    let reloaded = crate::keymap::load_profile_set().unwrap();
    assert_eq!(reloaded.active_profile(), done.set.active_profile());
}

#[test]
fn a_failed_move_leaves_the_keymap_alone_reports_to_messages_and_keeps_the_flag_unset() {
    let mut app = app_with_legacy_profile();
    let before = app.ui_state.keymap_profiles.clone();
    let needle = "Shortcuts were not moved to \u{2318}: disk said no";
    let seen = crate::diagnostics::logged_count(needle);

    let task = deliver(&mut app, Err("disk said no".to_string()));

    assert_eq!(app.ui_state.keymap_profiles, before);
    assert_eq!(crate::diagnostics::logged_count(needle), seen + 1);
    assert_eq!(
        task.units(),
        0,
        "no asked flag is written for a failed move"
    );
}

#[test]
fn a_finished_move_is_adopted_reported_and_remembered() {
    let mut app = app_with_legacy_profile();
    let source = app.ui_state.keymap_profiles.clone();
    let moved = migrate_active_to_mod(&source).unwrap().set;
    let needle = "the standard commands now use \u{2318}.";
    let seen = crate::diagnostics::logged_count(needle);

    let task = deliver(&mut app, Ok(written(moved.clone(), source)));

    assert_eq!(app.ui_state.keymap_profiles, moved);
    assert_eq!(crate::diagnostics::logged_count(needle), seen + 1);
    assert!(
        task.units() >= 1,
        "the asked flag is written once it landed"
    );
}

/// An Apply landed while the file was written: the moved copy was computed
/// from an older set and must not replace the live one.
#[test]
fn a_stale_result_is_not_adopted_and_the_question_stays_open() {
    let mut app = app_with_legacy_profile();
    let source = ShortcutProfileSet::built_ins().unwrap();
    let moved = migrate_active_to_mod(&legacy_set()).unwrap().set;
    let before = app.ui_state.keymap_profiles.clone();
    let needle = "not adopted: shortcuts changed meanwhile; earlier file kept as keyboard_shortcuts.toml.bak";
    let seen = crate::diagnostics::logged_count(needle);

    let task = deliver(&mut app, Ok(written(moved, source)));

    assert_eq!(
        app.ui_state.keymap_profiles, before,
        "the stale set is dropped"
    );
    assert_eq!(crate::diagnostics::logged_count(needle), seen + 1);
    assert_eq!(
        task.units(),
        1,
        "one task puts the live profiles back over the file; the flag stays unset"
    );
}

/// The stale move had already replaced the file; saving the live profiles
/// back leaves the file equal to them.
#[test]
fn the_resave_puts_the_live_profiles_back_over_the_stale_file() {
    let _root = IsolatedConfigRoot::new();
    let older = legacy_set();
    let live = {
        let mut set = ShortcutProfileSet::built_ins().unwrap();
        set.set_active_profile("altium").unwrap();
        set
    };
    let stale = migrate_active_to_mod(&older).unwrap();
    write_migration(older, stale).unwrap();

    resave_live(&live).unwrap();

    let on_disk = crate::keymap::load_profile_set().unwrap();
    assert_eq!(on_disk.active_profile_id(), live.active_profile_id());
    assert!(
        on_disk.profile("mine").is_none(),
        "the stale profile is gone from the file"
    );
}

#[test]
fn skipped_chords_are_listed_in_messages() {
    let mut app = app_with_legacy_profile();
    let source = app.ui_state.keymap_profiles.clone();
    let moved = migrate_active_to_mod(&source).unwrap().set;
    let mut done = written(moved, source);
    done.skipped = vec!["Ctrl+Skipped".to_string()];
    let needle = "already bound: Ctrl+Skipped.";
    let seen = crate::diagnostics::logged_count(needle);

    let _ = deliver(&mut app, Ok(done));

    assert_eq!(crate::diagnostics::logged_count(needle), seen + 1);
}

fn dirty_draft() -> crate::keymap::KeymapEditorModel {
    let mut draft = crate::keymap::KeymapEditorModel::new(legacy_set());
    let cycle = AppCommandId::new("cycle_unit").unwrap();
    let slot = draft
        .rows()
        .into_iter()
        .find(|row| row.command.as_ref() == Some(&cycle))
        .expect("the legacy profile binds cycle_unit")
        .slot;
    draft
        .edit_active_trigger(slot, "Ctrl+Alt+Q".into())
        .unwrap();
    draft
}

/// With Preferences open, its draft still holds the Control rows: Apply would
/// write them back over the migrated file. The draft is moved too, edits kept.
#[test]
fn an_open_preferences_draft_is_rebased_onto_the_moved_profile() {
    let mut app = app_with_legacy_profile();
    app.ui_state.preferences_open = true;
    app.ui_state.preferences_keymap_editor = dirty_draft();
    let source = app.ui_state.keymap_profiles.clone();
    let moved = migrate_active_to_mod(&source).unwrap().set;

    let _ = deliver(&mut app, Ok(written(moved, source)));

    let draft = app.ui_state.preferences_keymap_editor.active_profile();
    assert_eq!(
        crate::keymap::standard_ctrl_rows(draft),
        0,
        "Apply would revert the move"
    );
    let text: Vec<String> = draft
        .bindings
        .iter()
        .flat_map(|b| b.triggers.iter().map(ShortcutTrigger::config_text))
        .collect();
    assert!(text.iter().any(|t| t == "Mod+S"), "{text:?}");
    assert!(
        text.iter().any(|t| t == "Ctrl+Alt+Q"),
        "the pending edit is kept: {text:?}"
    );
}

/// With Preferences closed the draft is only a stale seed: it is re-seeded
/// from the live set, without a "draft moved" message.
#[test]
fn a_closed_preferences_draft_is_reseeded_quietly() {
    let mut app = app_with_legacy_profile();
    assert!(!app.ui_state.preferences_open);
    app.ui_state.preferences_keymap_editor = dirty_draft();
    let source = app.ui_state.keymap_profiles.clone();
    let moved = migrate_active_to_mod(&source).unwrap().set;
    let needle = "the open Preferences draft was moved";
    let seen = crate::diagnostics::logged_count(needle);

    let _ = deliver(&mut app, Ok(written(moved.clone(), source)));

    assert_eq!(
        app.ui_state.preferences_keymap_editor,
        crate::keymap::KeymapEditorModel::new(moved)
    );
    assert_eq!(crate::diagnostics::logged_count(needle), seen);
}

#[test]
fn keeping_control_changes_nothing_says_so_and_remembers_at_once() {
    let mut app = app_with_legacy_profile();
    let before = app.ui_state.keymap_profiles.clone();
    let seen = crate::diagnostics::logged_count("kept Control for the standard commands");

    let task = app.handle_keymap_mod_prompt_answered(false);

    assert_eq!(app.ui_state.keymap_profiles, before);
    assert_eq!(
        crate::diagnostics::logged_count("kept Control for the standard commands"),
        seen + 1
    );
    assert!(task.units() >= 1, "the asked flag is persisted by a Task");
}

/// "Use ⌘" only starts the write; the flag waits for the result.
#[test]
fn use_command_does_not_remember_before_the_write_lands() {
    let mut app = app_with_legacy_profile();
    let task = app.handle_keymap_mod_prompt_answered(true);
    assert_eq!(task.units(), 1, "one task: the write, no asked flag yet");
}
