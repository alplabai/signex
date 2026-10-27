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
