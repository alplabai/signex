use super::{
    AppCommandId, CompiledKeymap, KeyStroke, KeyToken, Modifiers, ShortcutBinding,
    ShortcutBindingAction, ShortcutContext, ShortcutProfile, ShortcutProfileKind,
    ShortcutProfileSet, ShortcutTrigger, config_path_for_dir, export_custom_profile,
    import_custom_profile, load_profile_set_at, save_profile_set_at,
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
    let mut tmp_name = path.file_name().unwrap().to_os_string();
    tmp_name.push(".tmp");
    assert!(!path.with_file_name(tmp_name).exists());
}

/// `save_profile_set_at` must go through `atomic_write`, not `fs::write`:
/// a failed save leaves the user's previously saved custom keymap profiles
/// fully intact instead of truncating them.
///
/// Discriminator: pre-creating a *directory* at `<path>.tmp` makes
/// `atomic_write`'s `File::create(&tmp)` fail before it can touch the
/// destination, and the call returns `Err`. A plain `fs::write` would ignore
/// the sibling, succeed, and clobber the old file — so this test fails on a
/// revert.
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

    let mut tmp_name = path.file_name().unwrap().to_os_string();
    tmp_name.push(".tmp");
    std::fs::create_dir_all(path.with_file_name(tmp_name)).unwrap();

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
