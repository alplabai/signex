use std::collections::HashMap;

use signex_erc::project_rules::profile::{BUILTIN_PROFILE_NAME, SOURCE_BUILTIN, builtin_profile};
use signex_erc::{RuleKind, Severity};
use signex_types::project::RuleProfileRef;

use super::*;

fn dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn file_names_are_safe_and_stable() {
    assert_eq!(
        file_name_for("Acme Corp / Rules!"),
        "acme-corp-rules.snxrules"
    );
    assert_eq!(file_name_for("  "), "profile.snxrules");
    assert_eq!(file_name_for("User Default"), "user-default.snxrules");
}

#[test]
fn a_saved_profile_is_listed_and_resolved_by_name() {
    let tmp = dir();
    let mut profile = builtin_profile();
    profile.name = "Company".into();
    profile
        .erc
        .severity
        .insert("future_rule".into(), "error".into());
    let path = save_profile(tmp.path(), &profile).unwrap();
    assert_eq!(path.file_name().unwrap(), "company.snxrules");
    assert_eq!(list_profiles(tmp.path()), vec![profile.clone()]);

    let reference = RuleProfileRef {
        name: "Company".into(),
        source: "user".into(),
        ..Default::default()
    };
    let (found, source) = resolve_profile(Some(tmp.path()), &reference)
        .unwrap()
        .unwrap();
    assert_eq!(found, profile);
    assert_eq!(source, "user");
}

#[test]
fn resolving_the_shipped_profile_needs_no_library() {
    let reference = RuleProfileRef {
        name: BUILTIN_PROFILE_NAME.into(),
        source: SOURCE_BUILTIN.into(),
        ..Default::default()
    };
    let (found, _) = resolve_profile(None, &reference).unwrap().unwrap();
    assert_eq!(found, builtin_profile());
    let other = RuleProfileRef {
        name: "Nope".into(),
        source: SOURCE_BUILTIN.into(),
        ..Default::default()
    };
    assert!(resolve_profile(None, &other).unwrap().is_none());
}

#[test]
fn an_unparseable_file_is_skipped_and_left_alone() {
    let tmp = dir();
    let bad = tmp.path().join("broken.snxrules");
    std::fs::write(&bad, b"{not json").unwrap();
    assert!(list_profiles(tmp.path()).is_empty());
    assert_eq!(std::fs::read(&bad).unwrap(), b"{not json");
}

#[test]
fn prefs_migrate_into_the_default_profile_once() {
    let tmp = dir();
    let severity = HashMap::from([(RuleKind::DanglingWire, Severity::Off)]);
    let pins = HashMap::new();
    let out = migrate_prefs_to_default_profile(tmp.path(), &severity, &pins).unwrap();
    assert_eq!(out, ProfileMigration::Migrated);
    let profiles = list_profiles(tmp.path());
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].name, USER_DEFAULT_PROFILE_NAME);
    assert_eq!(profiles[0].erc.severity["dangling_wire"], "off");

    // A later launch with different prefs must not overwrite the user's file.
    let changed = HashMap::from([(RuleKind::DanglingWire, Severity::Error)]);
    let out = migrate_prefs_to_default_profile(tmp.path(), &changed, &pins).unwrap();
    assert_eq!(out, ProfileMigration::AlreadyDone);
    assert_eq!(
        list_profiles(tmp.path())[0].erc.severity["dangling_wire"],
        "off"
    );
}

#[test]
fn empty_prefs_migrate_nothing() {
    let tmp = dir();
    let out =
        migrate_prefs_to_default_profile(tmp.path(), &HashMap::new(), &HashMap::new()).unwrap();
    assert_eq!(out, ProfileMigration::NothingToMigrate);
    assert!(list_profiles(tmp.path()).is_empty());
}

fn some_rules(rule: &str) -> signex_types::project::ErcRules {
    let mut erc = signex_types::project::ErcRules::default();
    erc.severity.insert(rule.into(), "error".into());
    erc
}

#[test]
fn save_as_profile_never_overwrites_an_existing_file() {
    let tmp = dir();
    let (first, first_path) = save_new_profile(tmp.path(), "Demo rules", some_rules("a")).unwrap();
    let before = std::fs::read(&first_path).unwrap();

    let (second, second_path) =
        save_new_profile(tmp.path(), "Demo rules", some_rules("b")).unwrap();
    let (third, _) = save_new_profile(tmp.path(), "Demo rules", some_rules("c")).unwrap();

    assert_eq!(first.name, "Demo rules");
    assert_eq!(second.name, "Demo rules 2");
    assert_eq!(third.name, "Demo rules 3");
    assert_ne!(first_path, second_path);
    assert_eq!(std::fs::read(&first_path).unwrap(), before);
    assert_eq!(list_profiles(tmp.path()).len(), 3);
}

#[test]
fn save_as_profile_never_uses_the_shipped_name() {
    let tmp = dir();
    for requested in [BUILTIN_PROFILE_NAME, "signex default"] {
        let (saved, _) = save_new_profile(tmp.path(), requested, some_rules("a")).unwrap();
        assert!(!saved.name.eq_ignore_ascii_case(BUILTIN_PROFILE_NAME));
        assert!(saved.name.ends_with(" 2") || saved.name.ends_with(" 3"));
    }
}
