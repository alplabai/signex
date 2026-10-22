use super::{
    AppCommandId, CompiledKeymap, KeyStroke, KeyToken, Modifiers, ProfileLoadError,
    ShortcutBinding, ShortcutBindingAction, ShortcutContext, ShortcutProfile, ShortcutProfileKind,
    ShortcutProfileSet, ShortcutTrigger, back_up_profile_file_at, config_path_for_dir,
    discard_profile_backup_at, export_custom_profile, import_custom_profile, load_profile_set_at,
    read_backup_profiles_at, restore_profiles_at, save_profile_set_at,
};
use std::str::FromStr;

#[test]
fn loads_built_in_profiles() {
    let set = ShortcutProfileSet::built_ins().unwrap();
    assert_eq!(set.profiles().count(), 2);
    assert_eq!(set.active_profile().id, "altium");
}

#[test]
fn lookup_supports_pending_multi_stroke_sequences() {
    let set = ShortcutProfileSet::built_ins().unwrap();
    let keymap = set.compile_active();
    let p = KeyStroke {
        modifiers: Modifiers::default(),
        key: KeyToken::Character("p".to_string()),
    };
    let lookup = keymap.lookup(&[p], &[ShortcutContext::Schematic]);
    assert!(lookup.pending);
    assert!(lookup.command.is_none());
}

#[test]
fn copies_built_in_profile_as_custom() {
    let set = ShortcutProfileSet::built_ins().unwrap();
    let custom = set
        .active_profile()
        .copy_as_custom("my-altium", "My Altium")
        .unwrap();
    assert_eq!(custom.kind, ShortcutProfileKind::Custom);
    assert_eq!(custom.base_profile.as_deref(), Some("altium"));
    assert_eq!(custom.bindings.len(), set.active_profile().bindings.len());
}

#[test]
fn later_unbind_suppresses_earlier_command_binding() {
    let command = AppCommandId::new("save_document").unwrap();
    let trigger = ShortcutTrigger::parse("Ctrl+S").unwrap();
    let profile = ShortcutProfile {
        id: "test".to_string(),
        name: "Test".to_string(),
        kind: ShortcutProfileKind::Custom,
        schema_version: 1,
        description: None,
        base_profile: None,
        bindings: vec![
            ShortcutBinding {
                action: ShortcutBindingAction::Command(command.clone()),
                context: ShortcutContext::Global,
                triggers: vec![trigger.clone()],
            },
            ShortcutBinding {
                action: ShortcutBindingAction::Unbind(command),
                context: ShortcutContext::Global,
                triggers: vec![trigger],
            },
        ],
    };

    let keymap = CompiledKeymap::compile(&profile);
    let stroke = KeyStroke::from_str("Ctrl+S").unwrap();
    let lookup = keymap.lookup(&[stroke], &[ShortcutContext::Global]);
    assert!(lookup.matched);
    assert!(lookup.command.is_none());
    assert!(
        keymap
            .shortcut_label(&AppCommandId::new("save_document").unwrap())
            .is_none()
    );
}

#[test]
fn exports_and_imports_custom_profile_toml() {
    let set = ShortcutProfileSet::built_ins().unwrap();
    let custom = set
        .active_profile()
        .copy_as_custom("my-altium", "My Altium")
        .unwrap();

    let exported = export_custom_profile(&custom).unwrap();

    assert!(exported.contains("[signex_settings]"));
    assert!(exported.contains("[keyboard_shortcuts]"));
    assert!(exported.contains("profile_kind = \"custom\""));
    assert!(!exported.contains("\nlabel ="));
    assert!(!exported.contains("\ncategory ="));

    let imported = import_custom_profile(&exported).unwrap();
    assert_eq!(imported, custom);
}

#[test]
fn persistence_round_trip_keeps_bundled_profiles_and_active_custom_profile() {
    let tmp = tempfile::tempdir().unwrap();
    let path = config_path_for_dir(tmp.path());
    let mut set = ShortcutProfileSet::built_ins().unwrap();
    let custom = set
        .active_profile()
        .copy_as_custom("my-altium", "My Altium")
        .unwrap();
    set.insert_custom_profile(custom).unwrap();
    set.set_active_profile("my-altium").unwrap();

    save_profile_set_at(&path, &set).unwrap();

    let source = std::fs::read_to_string(&path).unwrap();
    assert!(source.contains("active_profile = \"my-altium\""));
    assert!(source.contains("profile_id = \"my-altium\""));
    assert!(!source.contains("profile_id = \"altium\""));
    assert!(!source.contains("profile_id = \"classic\""));

    let loaded = load_profile_set_at(&path).unwrap();
    assert_eq!(loaded.active_profile().id, "my-altium");
    assert!(loaded.profile("altium").is_some());
    assert!(loaded.profile("classic").is_some());
    assert!(loaded.profile("my-altium").is_some());

    // A successful atomic save strands no `.tmp` sibling.
    assert!(!crate::test_support::has_stray_tmp(path.parent().unwrap()));
}

/// `save_profile_set_at` must go through `atomic_write`, not `fs::write`:
/// a failed save leaves the user's previously saved custom keymap profiles
/// fully intact instead of truncating them.
///
/// Discriminator: denying new-file creation in the destination's parent
/// directory makes `atomic_write`'s `File::create(&tmp)` fail before it can
/// touch the destination, regardless of the unique per-writer temp name it
/// picks (#416), and the call returns `Err`. A plain `fs::write` would
/// ignore that and clobber the old file — so this test fails on a revert.
#[test]
fn save_profile_set_at_leaves_previous_profiles_intact_when_write_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let path = config_path_for_dir(tmp.path());

    let mut first = ShortcutProfileSet::built_ins().unwrap();
    let custom = first
        .active_profile()
        .copy_as_custom("keeper", "Keeper")
        .unwrap();
    first.insert_custom_profile(custom).unwrap();
    first.set_active_profile("keeper").unwrap();
    save_profile_set_at(&path, &first).unwrap();
    let before = std::fs::read_to_string(&path).unwrap();

    let _deny = crate::test_support::DenyNewFiles::on(path.parent().unwrap());

    let mut second = ShortcutProfileSet::built_ins().unwrap();
    let other = second
        .active_profile()
        .copy_as_custom("clobberer", "Clobberer")
        .unwrap();
    second.insert_custom_profile(other).unwrap();
    second.set_active_profile("clobberer").unwrap();

    assert!(save_profile_set_at(&path, &second).is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    assert_eq!(
        load_profile_set_at(&path).unwrap().active_profile().id,
        "keeper"
    );
}

#[test]
fn import_rejects_built_in_profile_documents() {
    let source = r#"
[signex_settings]
application = "signex"
file_kind = "keyboard_shortcuts"
version = 1

[keyboard_shortcuts]
schema_version = 1
profile_id = "altium"
profile_name = "Altium"
profile_kind = "built_in"
"#;

    let err = import_custom_profile(source).unwrap_err().to_string();
    assert!(err.contains("built-in profile `altium` cannot be modified"));
}

#[test]
fn persistence_rejects_custom_profile_shadowing_built_in_id() {
    let tmp = tempfile::tempdir().unwrap();
    let path = config_path_for_dir(tmp.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        r#"
[signex_settings]
application = "signex"
file_kind = "keyboard_shortcuts"
version = 1

[keyboard_shortcuts]
active_profile = "altium"

[[keyboard_shortcuts.profiles]]
schema_version = 1
profile_id = "altium"
profile_name = "Shadow"
profile_kind = "custom"
"#,
    )
    .unwrap();

    let err = load_profile_set_at(&path).unwrap_err().to_string();
    assert!(err.contains("built-in profile `altium` cannot be modified"));
}

// ─── #595 — a refused load must stay visible, and must never be
// ─── silently overwritten by the built-in fallback on the next save.

/// Write a shortcuts file at `path` holding one custom profile, and
/// return the exact bytes that landed on disk. Shared by the backup
/// tests so each one starts from a real, schema-valid saved document
/// rather than a hand-written blob that rots against the format.
fn seed_saved_profiles(path: &std::path::Path, profile_id: &str) -> Vec<u8> {
    let mut set = ShortcutProfileSet::built_ins().unwrap();
    let custom = set
        .active_profile()
        .copy_as_custom(profile_id, "Seeded")
        .unwrap();
    set.insert_custom_profile(custom).unwrap();
    set.set_active_profile(profile_id).unwrap();
    save_profile_set_at(path, &set).unwrap();
    std::fs::read(path).unwrap()
}

/// A shortcuts file that exists but cannot be parsed must surface as
/// `Err`, not as a silent fall back to the built-ins — that `Err` is what
/// raises the Preferences banner and arms the backup-before-save guard.
#[test]
fn load_reports_error_when_the_file_cannot_be_parsed() {
    let tmp = tempfile::tempdir().unwrap();
    let path = config_path_for_dir(tmp.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "this is not = valid = toml [[[").unwrap();

    let error = load_profile_set_at(&path).unwrap_err();
    assert!(
        matches!(error, ProfileLoadError::Toml(_)),
        "expected a TOML parse failure, got {error:?}"
    );
}

/// A well-formed file whose `active_profile` no longer resolves fails in
/// `apply_to` -> `set_active_profile`. The user's custom profiles are all
/// still in that file, so this must not be mistaken for "no shortcuts".
#[test]
fn load_reports_error_when_the_active_profile_id_does_not_resolve() {
    let tmp = tempfile::tempdir().unwrap();
    let path = config_path_for_dir(tmp.path());
    seed_saved_profiles(&path, "my-altium");

    // Round-trip a real saved document and edit only the active id, so
    // the fixture stays valid against the current schema.
    let saved = std::fs::read_to_string(&path).unwrap();
    let rotted = saved.replace(
        "active_profile = \"my-altium\"",
        "active_profile = \"renamed-elsewhere\"",
    );
    assert_ne!(
        saved, rotted,
        "fixture did not rewrite the active profile id — the saved key name changed"
    );
    std::fs::write(&path, &rotted).unwrap();

    let error = load_profile_set_at(&path).unwrap_err();
    assert!(
        matches!(&error, ProfileLoadError::UnknownActiveProfile(id) if id == "renamed-elsewhere"),
        "expected an unresolved active profile, got {error:?}"
    );
}

/// The non-error path must stay non-error: a missing file is a fresh
/// install, not a failure, and must never raise the banner.
#[test]
fn load_succeeds_when_the_file_is_absent() {
    let tmp = tempfile::tempdir().unwrap();
    let path = config_path_for_dir(tmp.path());
    assert!(!path.exists());

    let set = load_profile_set_at(&path).unwrap();
    assert_eq!(set.active_profile().id, "altium");
    assert!(!path.exists(), "loading must not create the file");
}

/// The copy aside must be byte-identical, so the custom profiles this
/// process could not parse survive the save that follows.
#[test]
fn back_up_preserves_the_original_profiles() {
    let tmp = tempfile::tempdir().unwrap();
    let path = config_path_for_dir(tmp.path());
    let original = seed_saved_profiles(&path, "keeper");

    let bak = back_up_profile_file_at(&path).unwrap().unwrap();

    assert!(bak.exists(), "backup was not created at {}", bak.display());
    assert_eq!(std::fs::read(&bak).unwrap(), original);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        original,
        "the original must be copied, not moved"
    );
}

/// The double-Apply hazard: the second Apply would copy the already
/// overwritten file over the backup and destroy the only surviving copy
/// of the user's profiles. An existing `.bak` is the original — never
/// clobber it.
#[test]
fn back_up_refuses_to_overwrite_an_existing_backup() {
    let tmp = tempfile::tempdir().unwrap();
    let path = config_path_for_dir(tmp.path());
    let bak = back_up_profile_file_at(&path).unwrap();
    assert!(bak.is_none(), "nothing to back up yet");

    let mut bak_name = path.file_name().unwrap().to_os_string();
    bak_name.push(".bak");
    let bak_path = path.with_file_name(bak_name);

    // The `.bak` holds the user's real profiles; the live file has
    // already been replaced by the built-in-only fallback.
    seed_saved_profiles(&path, "the-only-copy");
    let rescued = std::fs::read(&path).unwrap();
    std::fs::rename(&path, &bak_path).unwrap();
    seed_saved_profiles(&path, "built-in-fallback");
    let clobberer = std::fs::read(&path).unwrap();
    assert_ne!(rescued, clobberer, "fixture must differ from the backup");

    let result = back_up_profile_file_at(&path).unwrap();

    assert!(
        result.is_none(),
        "an existing backup must be reported as nothing-to-do, got {result:?}"
    );
    assert_eq!(
        std::fs::read(&bak_path).unwrap(),
        rescued,
        "the existing backup was clobbered — the user's only surviving copy is gone"
    );
}

/// No file, nothing to preserve — and no stray `.bak` left behind.
#[test]
fn back_up_is_a_no_op_when_no_file_exists() {
    let tmp = tempfile::tempdir().unwrap();
    let path = config_path_for_dir(tmp.path());
    assert!(!path.exists());

    assert!(back_up_profile_file_at(&path).unwrap().is_none());

    let mut bak_name = path.file_name().unwrap().to_os_string();
    bak_name.push(".bak");
    assert!(!path.with_file_name(bak_name).exists());
}

/// Guards the `OsString` handling: the backup appends to the whole file
/// name, so the stem AND the `.toml` extension survive. `set_extension`
/// would silently produce `keyboard_shortcuts.bak` instead.
#[test]
fn back_up_appends_bak_to_the_whole_file_name() {
    let tmp = tempfile::tempdir().unwrap();
    let path = config_path_for_dir(tmp.path());
    assert_eq!(path.file_name().unwrap(), "keyboard_shortcuts.toml");
    seed_saved_profiles(&path, "named");

    let bak = back_up_profile_file_at(&path).unwrap().unwrap();

    assert_eq!(bak.file_name().unwrap(), "keyboard_shortcuts.toml.bak");
    assert_eq!(bak.parent(), path.parent());
}

/// The whole of #595 end to end, in the order the user hits it: a
/// recoverable file rots, the load fails, the app boots on the built-ins,
/// and the user opens Preferences and presses Apply to rebuild what
/// vanished. That Apply serialises only `Custom` profiles and the fallback
/// has none, so the write itself is unavoidably profile-free — the
/// property that has to hold is that their profiles are still recoverable
/// afterwards, and that pressing Apply a second time does not take that
/// away too.
#[test]
fn the_users_profiles_survive_an_apply_after_a_failed_load() {
    // Arrange — a real saved file with a custom profile, then one rotted
    // `active_profile` id. Every profile body in it is still intact.
    let tmp = tempfile::tempdir().unwrap();
    let path = config_path_for_dir(tmp.path());
    seed_saved_profiles(&path, "keeper");
    let rotted = std::fs::read_to_string(&path)
        .unwrap()
        .replace("active_profile = \"keeper\"", "active_profile = \"gone\"");
    std::fs::write(&path, &rotted).unwrap();
    assert!(
        rotted.contains("keeper"),
        "the rotted fixture must still carry the custom profile"
    );
    assert!(load_profile_set_at(&path).is_err(), "the load must fail");

    // Act — what the Apply handler does while `keymap_load_error` is set:
    // copy aside first, then save the built-in fallback it is holding.
    let fallback = ShortcutProfileSet::built_ins().unwrap();
    let bak = back_up_profile_file_at(&path).unwrap().unwrap();
    save_profile_set_at(&path, &fallback).unwrap();

    // Assert — the live file lost the profile, as it must; the backup did
    // not, which is the difference between a recoverable mistake and data
    // loss.
    let live = std::fs::read_to_string(&path).unwrap();
    assert!(
        !live.contains("keeper"),
        "saving a custom-profile-free set necessarily drops it from the live file"
    );
    assert_eq!(
        std::fs::read_to_string(&bak).unwrap(),
        rotted,
        "the backup must be the user's original file, byte for byte"
    );

    // Act again — the second Apply, which is what used to be survivable
    // only by accident.
    assert_eq!(
        back_up_profile_file_at(&path).unwrap(),
        None,
        "a second Apply must find the backup already taken"
    );
    save_profile_set_at(&path, &fallback).unwrap();

    // Assert — the backup still holds the originals.
    assert_eq!(
        std::fs::read_to_string(&bak).unwrap(),
        rotted,
        "the second Apply must not copy the overwritten file over the backup"
    );
}

// ─── #603 — the backup has to be recoverable from inside the app, and
// ─── nothing but an explicit user action may remove it.

/// The most likely reason the file failed to load is a dangling
/// `active_profile`, and it is fully recoverable: the profiles are all
/// there, only the pointer is stale. Refusing the whole restore over it
/// would strand the user's work in a file the app can read perfectly.
#[test]
fn a_backup_whose_active_profile_is_gone_still_restores_its_profiles() {
    // Arrange — a real saved document, rotted the same way #595's
    // end-to-end test rots it.
    let tmp = tempfile::tempdir().unwrap();
    let path = config_path_for_dir(tmp.path());
    let saved = seed_saved_profiles(&path, "keeper");
    let rotted = String::from_utf8(saved)
        .unwrap()
        .replace("active_profile = \"keeper\"", "active_profile = \"gone\"");
    let bak = path.with_file_name("keyboard_shortcuts.toml.bak");
    std::fs::write(&bak, &rotted).unwrap();
    assert!(
        load_profile_set_at(&bak).is_err(),
        "precondition: this backup is one the normal loader refuses"
    );

    // Act
    let restored = read_backup_profiles_at(&bak).expect("the profiles are readable");

    // Assert
    assert_eq!(
        restored.active_profile_reset.as_deref(),
        Some("gone"),
        "the unresolvable pointer must be reported, not hidden"
    );
    assert!(
        restored.set.profiles().any(|p| p.id == "keeper"),
        "the custom profile the user cares about must come back"
    );
}

/// An unparseable backup is not recoverable, and the restore has to say
/// so rather than half-importing or silently producing built-ins.
#[test]
fn an_unparseable_backup_fails_the_restore_and_changes_nothing() {
    // Arrange
    let tmp = tempfile::tempdir().unwrap();
    let path = config_path_for_dir(tmp.path());
    let live = seed_saved_profiles(&path, "keeper");
    let bak = path.with_file_name("keyboard_shortcuts.toml.bak");
    std::fs::write(&bak, b"this is not = valid = toml [[[").unwrap();

    // Act
    let outcome = read_backup_profiles_at(&bak);

    // Assert
    assert!(outcome.is_err(), "an unparseable backup must not restore");
    assert_eq!(
        std::fs::read(&path).unwrap(),
        live,
        "the live shortcuts file must be untouched by a failed restore"
    );
    assert_eq!(
        std::fs::read(&bak).unwrap(),
        b"this is not = valid = toml [[[",
        "the backup must be untouched by a failed restore"
    );
}

/// The `.bak` is never cleaned up, so it outlives the failure that made
/// it: months later the user may have a whole new set of profiles. A
/// restore that wrote straight over them would be the `prefs.json`
/// clobber of #594 with extra steps.
#[test]
fn restoring_moves_the_current_shortcuts_file_aside_instead_of_overwriting_it() {
    // Arrange — a live file with newer work, and an older backup.
    let tmp = tempfile::tempdir().unwrap();
    let path = config_path_for_dir(tmp.path());
    let newer = seed_saved_profiles(&path, "newer-work");
    let bak = path.with_file_name("keyboard_shortcuts.toml.bak");
    let older_path = tmp.path().join("older.toml");
    seed_saved_profiles(&older_path, "older-work");
    std::fs::copy(&older_path, &bak).unwrap();

    // Act
    let restored = read_backup_profiles_at(&bak).expect("the backup is readable");
    let aside = restore_profiles_at(&path, &restored.set).expect("the restore lands");

    // Assert
    let aside = aside.expect("the live file existed, so it must have been kept");
    assert_eq!(
        aside.file_name().and_then(|n| n.to_str()),
        Some("keyboard_shortcuts.toml.bak.2"),
        "the existing .bak is the original and must not be clobbered"
    );
    assert_eq!(
        std::fs::read(&aside).unwrap(),
        newer,
        "the newer work must survive the restore byte for byte"
    );
    assert!(
        load_profile_set_at(&path)
            .unwrap()
            .profiles()
            .any(|p| p.id == "older-work"),
        "the restored profiles must be the live ones now"
    );
}

/// Deleting is an explicit action and nothing else may do it — a
/// successful save removing the backup would throw the profiles away at
/// exactly the moment the user is most likely to want them back.
#[test]
fn discarding_removes_the_backup_and_is_a_no_op_when_there_is_none() {
    // Arrange
    let tmp = tempfile::tempdir().unwrap();
    let path = config_path_for_dir(tmp.path());
    seed_saved_profiles(&path, "keeper");
    let bak = path.with_file_name("keyboard_shortcuts.toml.bak");
    std::fs::copy(&path, &bak).unwrap();

    // Act / Assert
    assert!(discard_profile_backup_at(&bak).unwrap(), "it was there");
    assert!(!bak.exists(), "and it is gone");
    assert!(
        !discard_profile_backup_at(&bak).unwrap(),
        "a second discard reports nothing to do rather than failing"
    );
    assert!(
        path.exists(),
        "discarding the backup must never touch the live file"
    );
}
