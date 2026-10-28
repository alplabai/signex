use std::collections::HashMap;

use signex_erc::project_rules::profile::{BUILTIN_PROFILE_NAME, SOURCE_BUILTIN, builtin_profile};
use signex_erc::{RuleKind, Severity};
use signex_types::project::{RuleProfile, RuleProfileRef};

use super::*;

fn dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

/// Every profile in the library except the shipped one, in listing order.
fn list_profiles(dir: &Path) -> Vec<RuleProfile> {
    list_library(Some(dir))
        .profiles
        .into_iter()
        .filter(|l| l.kind != ProfileKind::Builtin)
        .map(|l| l.profile)
        .collect()
}

/// Write `profile` to its default file name, replacing what is there.
fn save_profile(dir: &Path, profile: &RuleProfile) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(file_name_for(&profile.name));
    signex_types::atomic_io::atomic_write(&path, &profile.to_json_bytes())?;
    Ok(path)
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
fn a_migration_left_as_an_empty_placeholder_is_finished_on_the_next_launch() {
    let tmp = dir();
    let path = tmp.path().join(file_name_for(USER_DEFAULT_PROFILE_NAME));
    std::fs::write(&path, b"").unwrap();
    let severity = HashMap::from([(RuleKind::DanglingWire, Severity::Off)]);
    let out = migrate_prefs_to_default_profile(tmp.path(), &severity, &HashMap::new()).unwrap();
    assert_eq!(out, ProfileMigration::Migrated);
    let profiles = list_profiles(tmp.path());
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].erc.severity["dangling_wire"], "off");
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
fn a_newer_schema_profile_reports_that_instead_of_missing() {
    let tmp = dir();
    std::fs::write(
        tmp.path().join("company.snxrules"),
        br#"{"schema": 99, "name": "Company"}"#,
    )
    .unwrap();
    let reference = RuleProfileRef {
        name: "Company".into(),
        source: "user".into(),
        ..Default::default()
    };
    let err = resolve_profile(Some(tmp.path()), &reference).unwrap_err();
    assert!(err.contains("written by a newer Signex"), "{err}");

    let other = RuleProfileRef {
        name: "Other".into(),
        source: "user".into(),
        ..Default::default()
    };
    // The newer-schema file might hold "Other" too, so "not in the library"
    // cannot be claimed.
    let why = resolve_profile(Some(tmp.path()), &other).unwrap_err();
    assert!(
        why.contains("may hold it") && why.contains("company.snxrules"),
        "{why}"
    );
}

#[test]
fn two_files_with_one_name_are_reported_as_ambiguous() {
    let tmp = dir();
    let mut profile = builtin_profile();
    profile.name = "Company".into();
    save_profile(tmp.path(), &profile).unwrap();
    std::fs::write(tmp.path().join("copy.snxrules"), profile.to_json_bytes()).unwrap();
    let reference = RuleProfileRef {
        name: "Company".into(),
        source: "user".into(),
        ..Default::default()
    };
    let why = resolve_profile(Some(tmp.path()), &reference).unwrap_err();
    assert!(why.contains("ambiguous"), "{why}");
    assert!(
        why.contains("company.snxrules") && why.contains("copy.snxrules"),
        "{why}"
    );
}

#[test]
fn a_profile_file_the_os_will_not_read_is_reported_not_missing() {
    let tmp = dir();
    // A directory with the profile's file name cannot be read as a file.
    std::fs::create_dir(tmp.path().join("company.snxrules")).unwrap();
    let reference = RuleProfileRef {
        name: "Company".into(),
        source: "user".into(),
        ..Default::default()
    };
    let why = resolve_profile(Some(tmp.path()), &reference).unwrap_err();
    assert!(why.contains("could not be opened"), "{why}");
}

#[test]
fn an_unreadable_file_under_another_name_is_listed_not_ignored() {
    let tmp = dir();
    std::fs::write(tmp.path().join("odd-name.snxrules"), b"{not json").unwrap();
    let reference = RuleProfileRef {
        name: "Company".into(),
        source: "user".into(),
        ..Default::default()
    };
    let why = resolve_profile(Some(tmp.path()), &reference).unwrap_err();
    assert!(
        why.contains("odd-name.snxrules") && why.contains("could not be read"),
        "{why}"
    );

    // A readable match still wins over an unrelated broken file.
    let mut profile = builtin_profile();
    profile.name = "Company".into();
    save_profile(tmp.path(), &profile).unwrap();
    assert!(
        resolve_profile(Some(tmp.path()), &reference)
            .unwrap()
            .is_some()
    );
}

#[test]
fn an_empty_placeholder_from_a_crash_is_not_a_corrupt_profile() {
    let tmp = dir();
    let stale = tmp.path().join("company.snxrules");
    std::fs::write(&stale, b"").unwrap();
    let reference = RuleProfileRef {
        name: "Company".into(),
        source: "user".into(),
        ..Default::default()
    };
    assert!(
        resolve_profile(Some(tmp.path()), &reference)
            .unwrap()
            .is_none()
    );
    assert!(list_profiles(tmp.path()).is_empty());
    // Save as refuses the name rather than replace what it cannot read.
    let err = save_named_profile(tmp.path(), "Company", "1.0", some_rules("a"), false).unwrap_err();
    assert!(err.contains("company.snxrules"), "{err}");
    assert!(stale.exists(), "the placeholder is left alone");
}

#[cfg(unix)]
#[test]
fn migration_never_overwrites_a_file_that_appeared_after_the_check() {
    let tmp = dir();
    // A dangling symlink: `exists()` says no, an exclusive create says taken.
    let target = tmp.path().join("elsewhere");
    let link = tmp.path().join(file_name_for(USER_DEFAULT_PROFILE_NAME));
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let severity = HashMap::from([(RuleKind::DanglingWire, Severity::Off)]);
    let out = migrate_prefs_to_default_profile(tmp.path(), &severity, &HashMap::new()).unwrap();
    assert_eq!(out, ProfileMigration::AlreadyDone);
    assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
    assert!(!target.exists());
}
