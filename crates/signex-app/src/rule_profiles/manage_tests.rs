//! Library management on disk (#1608): Save as with an explicit overwrite,
//! create / duplicate / rename / delete, in-place edits, the machine's rule
//! set, listing, and the rule set a new project starts with.

use signex_erc::project_rules::profile::{BUILTIN_PROFILE_NAME, SOURCE_BUILTIN, builtin_profile};
use signex_types::project::{ErcRules, RuleProfile};

use super::*;

fn dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn rules(rule: &str, severity: &str) -> ErcRules {
    let mut erc = ErcRules::default();
    erc.severity.insert(rule.into(), severity.into());
    erc
}

fn read(path: &Path) -> RuleProfile {
    RuleProfile::from_json_bytes(&std::fs::read(path).unwrap()).unwrap()
}

fn saved(outcome: SaveOutcome) -> SavedProfile {
    match outcome {
        SaveOutcome::Saved(saved) => saved,
        SaveOutcome::Exists => panic!("expected the profile to be written"),
    }
}

#[test]
fn input_validation_names_the_problem() {
    assert!(
        validate_profile_input("  Acme  ", " 2.1 ")
            .is_ok_and(|v| v == ("Acme".into(), "2.1".into()))
    );
    for (name, version, needle) in [
        ("", "1.0", "name"),
        ("   ", "1.0", "name"),
        ("Acme", "", "version"),
        ("Acme", "  ", "version"),
        ("Signex Default", "1.0", "reserved"),
        ("user default", "1.0", "reserved"),
        ("bad\nname", "1.0", "control"),
        ("Acme", "1\t0", "control"),
    ] {
        let err = validate_profile_input(name, version).unwrap_err();
        assert!(
            err.to_lowercase().contains(needle),
            "{name:?}/{version:?}: {err}"
        );
    }
    let long = "x".repeat(MAX_NAME_CHARS + 1);
    assert!(
        validate_profile_input(&long, "1.0")
            .unwrap_err()
            .contains("too long")
    );
    let long = "9".repeat(MAX_VERSION_CHARS + 1);
    assert!(
        validate_profile_input("Acme", &long)
            .unwrap_err()
            .contains("too long")
    );
}

#[test]
fn save_as_writes_a_new_profile_with_the_chosen_name_and_version() {
    let tmp = dir();
    let out =
        saved(save_named_profile(tmp.path(), " Acme ", "2.1", rules("a", "error"), false).unwrap());
    let on_disk = read(&tmp.path().join("acme.snxrules"));
    assert_eq!(
        (on_disk.name.as_str(), on_disk.version.as_str()),
        ("Acme", "2.1")
    );
    assert_eq!(out.reference.name, "Acme");
    assert_eq!(out.reference.version, "2.1");
    assert_eq!(out.reference.sha256, on_disk.content_sha256());
}

#[test]
fn a_taken_name_is_reported_and_nothing_is_written_without_confirmation() {
    let tmp = dir();
    saved(save_named_profile(tmp.path(), "Acme", "1.0", rules("a", "error"), false).unwrap());
    let path = tmp.path().join("acme.snxrules");
    let before = std::fs::read(&path).unwrap();

    let again = save_named_profile(tmp.path(), "acme", "9.9", rules("b", "off"), false).unwrap();
    assert_eq!(again, SaveOutcome::Exists);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "the file is untouched"
    );
    assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 1);
}

#[test]
fn a_confirmed_overwrite_replaces_that_profile_and_only_that_one() {
    let tmp = dir();
    saved(save_named_profile(tmp.path(), "Acme", "1.0", rules("a", "error"), false).unwrap());
    saved(save_named_profile(tmp.path(), "Other", "1.0", rules("o", "info"), false).unwrap());
    let other = std::fs::read(tmp.path().join("other.snxrules")).unwrap();

    let out =
        saved(save_named_profile(tmp.path(), "Acme", "2.0", rules("b", "off"), true).unwrap());
    let on_disk = read(&tmp.path().join("acme.snxrules"));
    assert_eq!(on_disk.version, "2.0");
    assert_eq!(on_disk.erc, rules("b", "off"));
    assert_eq!(out.reference.sha256, on_disk.content_sha256());
    assert_eq!(
        std::fs::read(tmp.path().join("other.snxrules")).unwrap(),
        other
    );
}

#[test]
fn overwrite_flag_on_a_free_name_just_creates_it() {
    let tmp = dir();
    saved(save_named_profile(tmp.path(), "Fresh", "1.0", rules("a", "error"), true).unwrap());
    assert!(tmp.path().join("fresh.snxrules").is_file());
}

#[test]
fn a_name_held_under_another_file_name_is_the_one_overwritten() {
    let tmp = dir();
    let mut held = builtin_profile();
    held.name = "Demo rules".into();
    let hand_named = tmp.path().join("renamed-by-hand.snxrules");
    std::fs::write(&hand_named, held.to_json_bytes()).unwrap();
    let before = std::fs::read(&hand_named).unwrap();

    assert_eq!(
        save_named_profile(tmp.path(), "Demo rules", "1.0", rules("a", "error"), false).unwrap(),
        SaveOutcome::Exists
    );
    assert_eq!(std::fs::read(&hand_named).unwrap(), before);

    saved(save_named_profile(tmp.path(), "Demo rules", "1.0", rules("a", "error"), true).unwrap());
    assert_eq!(read(&hand_named).erc, rules("a", "error"));
    assert_eq!(
        std::fs::read_dir(tmp.path()).unwrap().count(),
        1,
        "no second file"
    );
}

#[test]
fn a_file_in_the_way_that_is_not_a_readable_profile_is_never_overwritten() {
    let tmp = dir();
    let in_the_way = tmp.path().join("demo-rules.snxrules");
    std::fs::write(&in_the_way, b"{not json").unwrap();
    for overwrite in [false, true] {
        let err = save_named_profile(
            tmp.path(),
            "Demo rules",
            "1.0",
            rules("a", "error"),
            overwrite,
        )
        .unwrap_err();
        assert!(err.contains("demo-rules.snxrules"), "{err}");
        assert_eq!(std::fs::read(&in_the_way).unwrap(), b"{not json");
    }
}

#[test]
fn save_as_refuses_the_shipped_and_machine_names() {
    let tmp = dir();
    for name in [
        BUILTIN_PROFILE_NAME,
        "signex default",
        USER_DEFAULT_PROFILE_NAME,
    ] {
        let err =
            save_named_profile(tmp.path(), name, "1.0", rules("a", "error"), true).unwrap_err();
        assert!(err.contains("reserved"), "{err}");
    }
    assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
}

#[test]
fn concurrent_saves_of_one_name_write_it_once() {
    let tmp = dir();
    let path = tmp.path().to_path_buf();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|i| {
            let (path, barrier) = (path.clone(), barrier.clone());
            std::thread::spawn(move || {
                barrier.wait();
                save_named_profile(&path, "Same", "1.0", rules("r", &format!("sev{i}")), false)
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let winners = results
        .iter()
        .filter(|r| matches!(r, Ok(SaveOutcome::Saved(_))))
        .count();
    assert_eq!(winners, 1, "{results:?}");
    assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 1);
    assert_eq!(read(&tmp.path().join("same.snxrules")).name, "Same");
}

#[test]
fn create_and_duplicate_copy_the_rules_and_never_replace() {
    let tmp = dir();
    let mut source = builtin_profile();
    source.version = "3.0".into();
    source
        .erc
        .severity
        .insert("unused_pin".into(), "off".into());
    create_profile_from(tmp.path(), &source, "Copy").unwrap();
    let copy = read(&tmp.path().join("copy.snxrules"));
    assert_eq!((copy.name.as_str(), copy.version.as_str()), ("Copy", "3.0"));
    assert_eq!(copy.erc, source.erc);

    let err = create_profile_from(tmp.path(), &source, "COPY").unwrap_err();
    assert!(err.contains("already exists"), "{err}");
    assert_eq!(read(&tmp.path().join("copy.snxrules")), copy);
}

#[test]
fn rename_moves_the_profile_and_keeps_its_rules() {
    let tmp = dir();
    save_named_profile(tmp.path(), "Old", "1.4", rules("a", "error"), false).unwrap();
    rename_profile(tmp.path(), "Old", "New name").unwrap();
    assert!(!tmp.path().join("old.snxrules").exists());
    let renamed = read(&tmp.path().join("new-name.snxrules"));
    assert_eq!(
        (renamed.name.as_str(), renamed.version.as_str()),
        ("New name", "1.4")
    );
    assert_eq!(renamed.erc, rules("a", "error"));
}

#[test]
fn rename_refuses_a_taken_name_and_the_protected_profiles() {
    let tmp = dir();
    save_named_profile(tmp.path(), "A", "1.0", rules("a", "error"), false).unwrap();
    save_named_profile(tmp.path(), "B", "1.0", rules("b", "error"), false).unwrap();
    let (a, b) = (
        std::fs::read(tmp.path().join("a.snxrules")).unwrap(),
        std::fs::read(tmp.path().join("b.snxrules")).unwrap(),
    );
    assert!(
        rename_profile(tmp.path(), "A", "b")
            .unwrap_err()
            .contains("already exists")
    );
    assert!(
        rename_profile(tmp.path(), "A", BUILTIN_PROFILE_NAME)
            .unwrap_err()
            .contains("reserved")
    );
    assert!(rename_profile(tmp.path(), USER_DEFAULT_PROFILE_NAME, "Mine").is_err());
    assert!(
        rename_profile(tmp.path(), "Nope", "Mine")
            .unwrap_err()
            .contains("no profile")
    );
    assert_eq!(std::fs::read(tmp.path().join("a.snxrules")).unwrap(), a);
    assert_eq!(std::fs::read(tmp.path().join("b.snxrules")).unwrap(), b);
}

#[test]
fn rename_can_change_only_the_capitalisation() {
    let tmp = dir();
    save_named_profile(tmp.path(), "acme", "1.0", rules("a", "error"), false).unwrap();
    rename_profile(tmp.path(), "acme", "ACME").unwrap();
    assert_eq!(read(&tmp.path().join("acme.snxrules")).name, "ACME");
    assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 1);
}

#[test]
fn delete_removes_only_that_profile_and_protects_the_machine_set() {
    let tmp = dir();
    save_named_profile(tmp.path(), "A", "1.0", rules("a", "error"), false).unwrap();
    save_named_profile(tmp.path(), "B", "1.0", rules("b", "error"), false).unwrap();
    write_user_default(tmp.path(), rules("u", "off")).unwrap();

    delete_profile(tmp.path(), "A").unwrap();
    assert!(!tmp.path().join("a.snxrules").exists());
    assert!(tmp.path().join("b.snxrules").exists());
    assert!(delete_profile(tmp.path(), USER_DEFAULT_PROFILE_NAME).is_err());
    assert!(delete_profile(tmp.path(), BUILTIN_PROFILE_NAME).is_err());
    assert!(
        delete_profile(tmp.path(), "A")
            .unwrap_err()
            .contains("no profile")
    );
    assert!(tmp.path().join("user-default.snxrules").exists());
}

#[test]
fn an_edit_is_applied_to_the_file_as_it_is_and_never_recreates_a_deleted_one() {
    use signex_erc::{RuleKind, Severity};
    let tmp = dir();
    save_named_profile(tmp.path(), "Acme", "1.0", rules("a", "error"), false).unwrap();

    // The file changed behind the screen's back (another window's Save as, a
    // hand edit): the edit lands on that, not on an older copy.
    let mut elsewhere = read(&tmp.path().join("acme.snxrules"));
    elsewhere.erc.severity.insert("b".into(), "info".into());
    std::fs::write(tmp.path().join("acme.snxrules"), elsewhere.to_json_bytes()).unwrap();
    let done = edit_profile_in_place(
        tmp.path(),
        "Acme",
        RuleEdit::Severity(RuleKind::UnusedPin, Severity::Off),
    )
    .unwrap();
    assert_eq!(done.erc.severity["b"], "info", "the other change is kept");
    assert_eq!(done.erc.severity["unused_pin"], "off");
    assert_eq!(read(&tmp.path().join("acme.snxrules")), done);

    delete_profile(tmp.path(), "Acme").unwrap();
    let edit = RuleEdit::PinCell(1, 1, signex_erc::Severity::Off);
    let err = edit_profile_in_place(tmp.path(), "Acme", edit).unwrap_err();
    assert!(err.contains("no longer in the library"), "{err}");
    assert!(!tmp.path().join("acme.snxrules").exists());
    assert!(edit_profile_in_place(tmp.path(), BUILTIN_PROFILE_NAME, edit).is_err());
}

#[test]
fn an_edit_never_replaces_a_file_it_cannot_read() {
    let tmp = dir();
    let path = tmp.path().join("acme.snxrules");
    std::fs::write(&path, b"{not json").unwrap();
    let err = edit_profile_in_place(
        tmp.path(),
        "Acme",
        RuleEdit::PinCell(1, 1, signex_erc::Severity::Off),
    )
    .unwrap_err();
    assert!(!err.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), b"{not json");
}

#[test]
fn reset_to_shipped_restores_every_known_rule_and_keeps_unknown_ones() {
    let tmp = dir();
    let mut erc = rules("unused_pin", "off");
    erc.severity.insert("future_rule".into(), "error".into());
    erc.pin_matrix.insert("output/output".into(), "off".into());
    save_named_profile(tmp.path(), "Acme", "1.0", erc, false).unwrap();
    let done = edit_profile_in_place(tmp.path(), "Acme", RuleEdit::ResetToShipped).unwrap();
    assert_eq!(
        done.erc.severity["unused_pin"],
        builtin_profile().erc.severity["unused_pin"]
    );
    assert!(done.erc.pin_matrix.is_empty());
    assert_eq!(done.erc.severity["future_rule"], "error");
}

#[test]
fn the_machine_rule_set_is_created_then_updated_keeping_what_it_does_not_know() {
    let tmp = dir();
    assert_eq!(load_user_default(tmp.path()).unwrap(), None);
    write_user_default(tmp.path(), rules("unused_pin", "off")).unwrap();
    let first = load_user_default(tmp.path()).unwrap().unwrap();
    assert_eq!(first.name, USER_DEFAULT_PROFILE_NAME);
    assert_eq!(first.erc.severity["unused_pin"], "off");

    // A newer build's rule id and key, and a hand-edited version, survive.
    let mut hand = first.clone();
    hand.version = "7".into();
    hand.erc
        .severity
        .insert("future_rule".into(), "error".into());
    std::fs::write(
        tmp.path().join("user-default.snxrules"),
        hand.to_json_bytes(),
    )
    .unwrap();
    write_user_default(tmp.path(), rules("unused_pin", "info")).unwrap();
    let after = load_user_default(tmp.path()).unwrap().unwrap();
    assert_eq!(after.version, "7");
    assert_eq!(after.erc.severity["unused_pin"], "info");
    assert_eq!(after.erc.severity["future_rule"], "error");
}

#[test]
fn an_unreadable_machine_rule_file_is_reported_and_left_alone() {
    let tmp = dir();
    let path = tmp.path().join("user-default.snxrules");
    std::fs::write(&path, b"{not json").unwrap();
    assert!(load_user_default(tmp.path()).is_err());
    assert!(write_user_default(tmp.path(), rules("a", "off")).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"{not json");
}

#[test]
fn the_listing_puts_the_shipped_then_machine_profile_first_and_reports_bad_files() {
    let tmp = dir();
    save_named_profile(tmp.path(), "Zeta", "1.0", rules("a", "error"), false).unwrap();
    save_named_profile(tmp.path(), "alpha", "1.0", rules("a", "error"), false).unwrap();
    write_user_default(tmp.path(), rules("a", "off")).unwrap();
    std::fs::write(tmp.path().join("broken.snxrules"), b"{nope").unwrap();
    std::fs::write(
        tmp.path().join("future.snxrules"),
        br#"{"schema": 99, "name": "Future"}"#,
    )
    .unwrap();
    // A second file that repeats a name is reported, not listed twice.
    let mut dup = read(&tmp.path().join("zeta.snxrules"));
    dup.version = "2".into();
    std::fs::write(tmp.path().join("zeta-copy.snxrules"), dup.to_json_bytes()).unwrap();
    // A file squatting on the shipped name is reported, not listed.
    let mut squat = builtin_profile();
    squat.name = "signex default".into();
    std::fs::write(tmp.path().join("squat.snxrules"), squat.to_json_bytes()).unwrap();

    let listing = list_library(Some(tmp.path()));
    let order: Vec<(&str, ProfileKind)> = listing
        .profiles
        .iter()
        .map(|l| (l.profile.name.as_str(), l.kind))
        .collect();
    assert_eq!(
        order,
        vec![
            (BUILTIN_PROFILE_NAME, ProfileKind::Builtin),
            (USER_DEFAULT_PROFILE_NAME, ProfileKind::UserDefault),
            ("alpha", ProfileKind::User),
            ("Zeta", ProfileKind::User),
        ]
    );
    let problems = listing.problems.join("\n");
    for needle in [
        "broken.snxrules",
        "newer Signex",
        "repeats the name",
        "squat.snxrules",
    ] {
        assert!(problems.contains(needle), "{needle} in {problems}");
    }
    // Reading the library changed nothing.
    assert_eq!(
        std::fs::read(tmp.path().join("broken.snxrules")).unwrap(),
        b"{nope"
    );
}

#[test]
fn with_no_library_folder_the_listing_still_offers_the_shipped_profile() {
    let listing = list_library(None);
    assert_eq!(listing.profiles.len(), 1);
    assert!(!listing.problems.is_empty());
}

// ---- seeding a new project ----

fn company(dir: &Path) -> RuleProfile {
    let mut profile = builtin_profile();
    profile.name = "Company".into();
    profile.version = "4".into();
    profile
        .erc
        .severity
        .insert("unused_pin".into(), "error".into());
    let path = dir.join("company.snxrules");
    std::fs::write(path, profile.to_json_bytes()).unwrap();
    profile
}

#[test]
fn a_new_project_copies_the_selected_profile_with_its_provenance() {
    let tmp = dir();
    let profile = company(tmp.path());
    let (seeded, notes) = rules_for_new_project(Some(tmp.path()), Some("Company"));
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(seeded.erc, profile.erc);
    let reference = seeded.profile.expect("provenance");
    assert_eq!(
        (
            reference.name.as_str(),
            reference.source.as_str(),
            reference.version.as_str()
        ),
        ("Company", "user", "4")
    );
    assert_eq!(reference.sha256, profile.content_sha256());
}

#[test]
fn a_new_project_with_no_choice_uses_the_machine_rules_when_there_are_some() {
    let tmp = dir();
    write_user_default(tmp.path(), rules("unused_pin", "off")).unwrap();
    let (seeded, notes) = rules_for_new_project(Some(tmp.path()), None);
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(seeded.erc.severity["unused_pin"], "off");
    assert_eq!(seeded.profile.unwrap().name, USER_DEFAULT_PROFILE_NAME);
}

#[test]
fn a_new_project_with_nothing_to_copy_gets_the_shipped_profile() {
    let tmp = dir();
    for dir in [Some(tmp.path()), None] {
        let (seeded, notes) = rules_for_new_project(dir, None);
        assert!(notes.is_empty(), "{notes:?}");
        let reference = seeded.profile.unwrap();
        assert_eq!(
            (reference.name.as_str(), reference.source.as_str()),
            (BUILTIN_PROFILE_NAME, SOURCE_BUILTIN)
        );
        assert_eq!(seeded.erc, builtin_profile().erc);
    }
    let (picked, _) = rules_for_new_project(Some(tmp.path()), Some("signex default"));
    assert_eq!(picked.profile.unwrap().source, SOURCE_BUILTIN);
}

#[test]
fn a_chosen_profile_that_is_gone_or_unreadable_falls_back_loudly() {
    let tmp = dir();
    let (seeded, notes) = rules_for_new_project(Some(tmp.path()), Some("Gone"));
    assert_eq!(seeded.profile.unwrap().name, BUILTIN_PROFILE_NAME);
    assert!(
        notes[0].contains("\"Gone\"") && notes[0].contains("not in the profile library"),
        "{notes:?}"
    );

    std::fs::write(tmp.path().join("gone.snxrules"), b"{nope").unwrap();
    let (seeded, notes) = rules_for_new_project(Some(tmp.path()), Some("Gone"));
    assert_eq!(seeded.profile.unwrap().name, BUILTIN_PROFILE_NAME);
    assert!(notes[0].contains("could not be read"), "{notes:?}");
    assert_eq!(
        std::fs::read(tmp.path().join("gone.snxrules")).unwrap(),
        b"{nope"
    );
}
