//! Preferences ▸ Rule Profiles (#1608): list and select, edit with the shared
//! rule editor, the default picker, and the delete question.

use std::sync::Arc;

use signex_erc::{RuleKind, Severity};
use signex_types::project::ErcRules;

use super::super::erc::execute_profile_write;
use crate::app::state::rule_profile_ui::QueuedWrite;
use crate::preferences::{ProfileAction, ProfilesMsg};
use crate::rule_profiles::{self, ProfileKind};

fn app() -> crate::app::Signex {
    let (mut app, _) = crate::app::Signex::new();
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    app
}

fn rules(rule: &str, severity: &str) -> ErcRules {
    let mut erc = ErcRules::default();
    erc.severity.insert(rule.into(), severity.into());
    erc
}

/// A library with "Company" and the machine's rules, listed into the app.
fn listed_app(lib: &std::path::Path) -> crate::app::Signex {
    rule_profiles::save_named_profile(lib, "Company", "1.0", rules("unused_pin", "error"), false)
        .unwrap();
    rule_profiles::write_user_default(lib, rules("dangling_wire", "off")).unwrap();
    let mut app = app();
    let listing = rule_profiles::list_library(Some(lib));
    let _ = app.handle_profiles_message(ProfilesMsg::Listed(Arc::new(listing)));
    app
}

fn select(app: &mut crate::app::Signex, name: &str) {
    let _ = app.handle_profiles_message(ProfilesMsg::Selected(name.into()));
}

fn manager(app: &crate::app::Signex) -> &crate::app::rule_profile_ui::ProfileManager {
    &app.ui_state.profile_manager
}

#[test]
fn the_listing_selects_the_first_profile_and_keeps_a_selection_that_survives() {
    let lib = tempfile::tempdir().unwrap();
    let mut app = listed_app(lib.path());
    assert!(manager(&app).loaded);
    assert_eq!(manager(&app).selected.as_deref(), Some("Signex Default"));
    assert_eq!(manager(&app).listing.profiles.len(), 3);

    select(&mut app, "Company");
    assert_eq!(
        manager(&app).name_input,
        "Company",
        "the name field follows the pick"
    );
    let again = rule_profiles::list_library(Some(lib.path()));
    let _ = app.handle_profiles_message(ProfilesMsg::Listed(Arc::new(again)));
    assert_eq!(manager(&app).selected.as_deref(), Some("Company"));

    rule_profiles::delete_profile(lib.path(), "Company").unwrap();
    let gone = rule_profiles::list_library(Some(lib.path()));
    let _ = app.handle_profiles_message(ProfilesMsg::Listed(Arc::new(gone)));
    assert_eq!(manager(&app).selected.as_deref(), Some("Signex Default"));
}

#[test]
fn the_shipped_profile_cannot_be_edited_from_the_editor() {
    let lib = tempfile::tempdir().unwrap();
    let mut app = listed_app(lib.path());
    select(&mut app, "Signex Default");
    let before = manager(&app).listing.clone();
    let task = app.handle_profiles_message(ProfilesMsg::SeverityChanged(
        RuleKind::UnusedPin,
        Severity::Off,
    ));
    assert_eq!(task.units(), 0);
    assert_eq!(manager(&app).listing, before);
    assert!(app.ui_state.profile_writes.is_idle());
}

#[test]
fn editing_a_profile_writes_its_own_file_and_round_trips() {
    let lib = tempfile::tempdir().unwrap();
    let mut app = listed_app(lib.path());
    select(&mut app, "Company");

    let task = app.handle_profiles_message(ProfilesMsg::SeverityChanged(
        RuleKind::DanglingWire,
        Severity::Info,
    ));
    assert_eq!(task.units(), 1, "one write starts");
    let _ = app.handle_profiles_message(ProfilesMsg::PinCellCycled { row: 1, col: 1 });

    // The first snapshot, then the waiting one, as the queue would run them.
    let first = app.ui_state.profile_writes.running().cloned().unwrap();
    execute_profile_write(lib.path(), first).unwrap();
    assert_eq!(app.handle_profile_write_finished(&Ok(())).units(), 1);
    let second = app.ui_state.profile_writes.running().cloned().unwrap();
    execute_profile_write(lib.path(), second).unwrap();
    let _ = app.handle_profile_write_finished(&Ok(()));

    let reread = rule_profiles::list_library(Some(lib.path()));
    let company = reread
        .profiles
        .iter()
        .find(|l| l.profile.name == "Company")
        .unwrap();
    assert_eq!(company.profile.erc.severity["dangling_wire"], "info");
    assert_eq!(
        company.profile.erc.severity["unused_pin"], "error",
        "other rules kept"
    );
    assert_eq!(company.profile.erc.pin_matrix.len(), 1);
    assert_eq!(
        manager(&app).selected_profile().unwrap().profile,
        company.profile,
        "the editor shows what the file holds"
    );
    // The other profiles were not touched.
    let machine = reread
        .profiles
        .iter()
        .find(|l| l.kind == ProfileKind::UserDefault)
        .unwrap();
    assert_eq!(machine.profile.erc, rules("dangling_wire", "off"));
}

#[test]
fn editing_the_machine_profile_uses_the_same_store_as_the_erc_dialog() {
    let lib = tempfile::tempdir().unwrap();
    let mut app = listed_app(lib.path());
    select(&mut app, "User Default");

    let _ = app.handle_profiles_message(ProfilesMsg::SeverityChanged(
        RuleKind::UnusedPin,
        Severity::Off,
    ));
    assert_eq!(
        app.ui_state.erc_severity_override[&RuleKind::UnusedPin],
        Severity::Off
    );
    assert!(matches!(
        app.ui_state.profile_writes.running(),
        Some(QueuedWrite::UserDefault(_))
    ));
    // The open editor shows it at once.
    let shown = &manager(&app).selected_profile().unwrap().profile.erc;
    assert_eq!(shown.severity["unused_pin"], "off");
}

#[test]
fn choosing_the_default_profile_is_remembered_and_can_go_back_to_automatic() {
    let lib = tempfile::tempdir().unwrap();
    let mut app = listed_app(lib.path());
    assert_eq!(app.ui_state.default_rule_profile, None);

    let task = app.handle_profiles_message(ProfilesMsg::DefaultChosen(Some("Company".into())));
    assert_eq!(
        app.ui_state.default_rule_profile.as_deref(),
        Some("Company")
    );
    assert_eq!(
        task.units(),
        1,
        "the choice is written to prefs.json off the update loop"
    );

    let _ = app.handle_profiles_message(ProfilesMsg::DefaultChosen(None));
    assert_eq!(app.ui_state.default_rule_profile, None);
}

#[test]
fn the_default_follows_a_rename_and_falls_back_after_a_delete() {
    let lib = tempfile::tempdir().unwrap();
    let mut app = listed_app(lib.path());
    app.ui_state.default_rule_profile = Some("Company".into());

    let renamed = ProfileAction::Renamed {
        old: "Company".into(),
        new: "Acme".into(),
    };
    let _ = app.handle_profiles_message(ProfilesMsg::ActionFinished(Arc::new(Ok(renamed))));
    assert_eq!(app.ui_state.default_rule_profile.as_deref(), Some("Acme"));
    assert_eq!(manager(&app).selected.as_deref(), Some("Acme"));

    // Another profile's rename leaves the default alone.
    let other = ProfileAction::Renamed {
        old: "Zed".into(),
        new: "Yak".into(),
    };
    let _ = app.handle_profiles_message(ProfilesMsg::ActionFinished(Arc::new(Ok(other))));
    assert_eq!(app.ui_state.default_rule_profile.as_deref(), Some("Acme"));

    let _ = app.handle_profiles_message(ProfilesMsg::ActionFinished(Arc::new(Ok(
        ProfileAction::Deleted("Acme".into()),
    ))));
    assert_eq!(app.ui_state.default_rule_profile, None);
    let (is_error, text) = manager(&app).status.clone().unwrap();
    assert!(!is_error && text.contains("automatic default"), "{text}");
}

#[test]
fn a_failed_action_is_shown_and_logged() {
    let mut app = app();
    let _ = app.handle_profiles_message(ProfilesMsg::ActionFinished(Arc::new(Err(
        "A profile named \"X\" already exists.".into(),
    ))));
    let (is_error, text) = manager(&app).status.clone().unwrap();
    assert!(is_error && text.contains("already exists"), "{text}");
}

#[test]
fn delete_asks_first_and_only_for_your_own_profiles() {
    let lib = tempfile::tempdir().unwrap();
    let mut app = listed_app(lib.path());

    for protected in ["Signex Default", "User Default"] {
        select(&mut app, protected);
        let _ = app.handle_profiles_message(ProfilesMsg::DeleteRequested);
        assert_eq!(
            manager(&app).confirm_delete,
            None,
            "{protected} is protected"
        );
    }

    select(&mut app, "Company");
    let _ = app.handle_profiles_message(ProfilesMsg::DeleteRequested);
    assert_eq!(manager(&app).confirm_delete.as_deref(), Some("Company"));
    assert!(
        lib.path().join("company.snxrules").exists(),
        "asking deletes nothing"
    );

    let _ = app.handle_profiles_message(ProfilesMsg::DeleteCancelled);
    assert_eq!(manager(&app).confirm_delete, None);
    assert_eq!(
        app.handle_profiles_message(ProfilesMsg::DeleteConfirmed)
            .units(),
        0,
        "a stray confirm with no question deletes nothing"
    );

    let _ = app.handle_profiles_message(ProfilesMsg::DeleteRequested);
    assert_eq!(
        app.handle_profiles_message(ProfilesMsg::DeleteConfirmed)
            .units(),
        1
    );
    assert_eq!(manager(&app).confirm_delete, None);
}

#[test]
fn library_actions_wait_for_a_pending_edit_so_they_never_miss_it() {
    let lib = tempfile::tempdir().unwrap();
    let mut app = listed_app(lib.path());
    select(&mut app, "Company");
    let _ = app.handle_profiles_message(ProfilesMsg::SeverityChanged(
        RuleKind::DanglingWire,
        Severity::Info,
    ));
    assert!(!app.ui_state.profile_writes.is_idle());

    let _ = app.handle_profiles_message(ProfilesMsg::NameInputChanged("Copy".into()));
    let task = app.handle_profiles_message(ProfilesMsg::DuplicateRequested);
    assert_eq!(task.units(), 0);
    let (is_error, text) = manager(&app).status.clone().unwrap();
    assert!(is_error && text.contains("still being saved"), "{text}");
}

#[test]
fn new_duplicate_and_rename_start_one_write_when_the_library_is_idle() {
    let lib = tempfile::tempdir().unwrap();
    let mut app = listed_app(lib.path());
    select(&mut app, "Company");
    let _ = app.handle_profiles_message(ProfilesMsg::NameInputChanged("Copy".into()));
    assert_eq!(
        app.handle_profiles_message(ProfilesMsg::CreateRequested)
            .units(),
        1
    );
    assert_eq!(
        app.handle_profiles_message(ProfilesMsg::DuplicateRequested)
            .units(),
        1
    );
    assert_eq!(
        app.handle_profiles_message(ProfilesMsg::RenameRequested)
            .units(),
        1
    );

    // The shipped profile and the machine's rules cannot be renamed.
    select(&mut app, "User Default");
    assert_eq!(
        app.handle_profiles_message(ProfilesMsg::RenameRequested)
            .units(),
        0
    );
}
