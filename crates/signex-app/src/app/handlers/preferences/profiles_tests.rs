//! Preferences ▸ Rule Profiles (#1608): list and select, edit with the shared
//! rule editor, the default picker, and the delete question.

use std::sync::Arc;

use signex_erc::{RuleKind, Severity};
use signex_types::project::ErcRules;

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
    first.run(lib.path()).unwrap();
    assert_eq!(app.handle_profile_write_finished(&Ok(None)).units(), 1);
    let second = app.ui_state.profile_writes.running().cloned().unwrap();
    second.run(lib.path()).unwrap();
    let _ = app.handle_profile_write_finished(&Ok(None));

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
        app.ui_state.profile_writes.running().map(|d| &d.write),
        Some(QueuedWrite::UserDefault(_))
    ));
    // The open editor shows it at once.
    let shown = manager(&app)
        .selected_profile()
        .unwrap()
        .profile
        .erc
        .clone();
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
    for request in [
        ProfilesMsg::CreateRequested,
        ProfilesMsg::DuplicateRequested,
        ProfilesMsg::RenameRequested,
    ] {
        let name = format!("{request:?}");
        assert_eq!(app.handle_profiles_message(request).units(), 1, "{name}");
        assert!(manager(&app).busy);
        // The worker reports (here: a failure, which leaves the selection).
        let _ = app
            .handle_profiles_message(ProfilesMsg::ActionFinished(Arc::new(Err("stopped".into()))));
        assert!(!manager(&app).busy);
    }

    // The shipped profile and the machine's rules cannot be renamed.
    select(&mut app, "User Default");
    assert_eq!(
        app.handle_profiles_message(ProfilesMsg::RenameRequested)
            .units(),
        0
    );
}

// ---- review round: stale listings, busy actions, reset, notices ----

fn queued_edit(app: &crate::app::Signex) -> (String, crate::rule_profiles::RuleEdit) {
    match app.ui_state.profile_writes.running().map(|d| &d.write) {
        Some(QueuedWrite::ProfileEdit { name, edit }) => (name.clone(), *edit),
        other => panic!("expected a queued profile edit, got {other:?}"),
    }
}

#[test]
fn a_pane_edit_queues_the_one_change_not_the_whole_profile() {
    let lib = tempfile::tempdir().unwrap();
    let mut app = listed_app(lib.path());
    select(&mut app, "Company");
    let _ = app.handle_profiles_message(ProfilesMsg::SeverityChanged(
        RuleKind::DanglingWire,
        Severity::Info,
    ));
    let (name, edit) = queued_edit(&app);
    assert_eq!(name, "Company");
    assert_eq!(
        edit,
        crate::rule_profiles::RuleEdit::Severity(RuleKind::DanglingWire, Severity::Info)
    );
}

#[test]
fn a_stale_listing_does_not_hide_a_pending_edit_and_is_read_again_afterwards() {
    let lib = tempfile::tempdir().unwrap();
    let mut app = listed_app(lib.path());
    select(&mut app, "Company");
    let _ = app.handle_profiles_message(ProfilesMsg::SeverityChanged(
        RuleKind::DanglingWire,
        Severity::Info,
    ));
    let shown = manager(&app).listing.clone();

    // A listing that was read before the write landed.
    let stale = rule_profiles::list_library(Some(lib.path()));
    let _ = app.handle_profiles_message(ProfilesMsg::Listed(Arc::new(stale)));
    assert_eq!(
        manager(&app).listing,
        shown,
        "the older listing is not applied"
    );
    assert!(manager(&app).relist_pending);

    // The write ends: the pane shows the file's truth and reads the library.
    let running = app.ui_state.profile_writes.running().cloned().unwrap();
    let done = running.run(lib.path());
    let next = app.handle_profile_write_finished(&done);
    assert_eq!(next.units(), 1, "the library is read again");
    assert!(!manager(&app).relist_pending);
    let on_disk = rule_profiles::list_library(Some(lib.path()));
    let company = |l: &rule_profiles::ProfileListing| {
        l.profiles
            .iter()
            .find(|p| p.profile.name == "Company")
            .unwrap()
            .profile
            .clone()
    };
    assert_eq!(company(&manager(&app).listing), company(&on_disk));
}

#[test]
fn a_failed_pane_write_is_shown_in_the_pane_and_the_library_is_read_again() {
    let lib = tempfile::tempdir().unwrap();
    let mut app = listed_app(lib.path());
    select(&mut app, "Company");
    let _ = app.handle_profiles_message(ProfilesMsg::PinCellCycled { row: 1, col: 1 });
    let next = app.handle_profile_write_finished(&Err("disk full".into()));
    let (is_error, text) = manager(&app).status.clone().unwrap();
    assert!(is_error && text.contains("disk full"), "{text}");
    assert_eq!(next.units(), 1, "the screen is rebuilt from the files");
}

#[test]
fn edits_wait_while_a_create_rename_or_delete_runs() {
    let lib = tempfile::tempdir().unwrap();
    let mut app = listed_app(lib.path());
    select(&mut app, "Company");
    let _ = app.handle_profiles_message(ProfilesMsg::NameInputChanged("Copy".into()));
    assert_eq!(
        app.handle_profiles_message(ProfilesMsg::DuplicateRequested)
            .units(),
        1
    );
    assert!(manager(&app).busy);

    let before = manager(&app).listing.clone();
    let task = app.handle_profiles_message(ProfilesMsg::SeverityChanged(
        RuleKind::DanglingWire,
        Severity::Info,
    ));
    assert_eq!(task.units(), 0);
    assert_eq!(
        manager(&app).listing,
        before,
        "the edit was refused, not half applied"
    );
    assert!(app.ui_state.profile_writes.is_idle());
    assert!(manager(&app).status.clone().unwrap().0);
    // A second library action is refused too.
    assert_eq!(
        app.handle_profiles_message(ProfilesMsg::CreateRequested)
            .units(),
        0
    );

    let _ = app.handle_profiles_message(ProfilesMsg::ActionFinished(Arc::new(Ok(
        ProfileAction::Created("Copy".into()),
    ))));
    assert!(!manager(&app).busy);
    select(&mut app, "Company");
    let task = app.handle_profiles_message(ProfilesMsg::SeverityChanged(
        RuleKind::DanglingWire,
        Severity::Info,
    ));
    assert_eq!(task.units(), 1, "edits work again");
}

#[test]
fn reset_to_shipped_asks_first_and_covers_library_machine_and_shipped_profiles() {
    let lib = tempfile::tempdir().unwrap();
    let mut app = listed_app(lib.path());

    select(&mut app, "Signex Default");
    let _ = app.handle_profiles_message(ProfilesMsg::ResetRequested);
    assert_eq!(
        manager(&app).confirm_reset,
        None,
        "nothing to reset on the shipped profile"
    );

    select(&mut app, "Company");
    let _ = app.handle_profiles_message(ProfilesMsg::ResetRequested);
    assert_eq!(manager(&app).confirm_reset.as_deref(), Some("Company"));
    assert!(
        app.ui_state.profile_writes.is_idle(),
        "asking changes nothing"
    );
    let _ = app.handle_profiles_message(ProfilesMsg::ResetCancelled);
    assert_eq!(manager(&app).confirm_reset, None);
    assert_eq!(
        app.handle_profiles_message(ProfilesMsg::ResetConfirmed)
            .units(),
        0,
        "a confirm without the question does nothing"
    );

    let _ = app.handle_profiles_message(ProfilesMsg::ResetRequested);
    assert_eq!(
        app.handle_profiles_message(ProfilesMsg::ResetConfirmed)
            .units(),
        1
    );
    assert_eq!(
        queued_edit(&app).1,
        crate::rule_profiles::RuleEdit::ResetToShipped
    );
    let shown = manager(&app)
        .selected_profile()
        .unwrap()
        .profile
        .erc
        .clone();
    assert_eq!(
        shown.severity["unused_pin"],
        signex_erc::project_rules::profile::builtin_profile()
            .erc
            .severity["unused_pin"]
    );
    let running = app.ui_state.profile_writes.running().cloned().unwrap();
    let _ = app.handle_profile_write_finished(&running.run(lib.path()));
    let stored = rule_profiles::list_library(Some(lib.path()));
    let company = stored
        .profiles
        .iter()
        .find(|p| p.profile.name == "Company")
        .unwrap();
    assert_eq!(
        company.profile.erc.severity["unused_pin"],
        shown.severity["unused_pin"]
    );

    // The machine's rules go back to the shipped ones through the dialog's store.
    app.ui_state
        .erc_severity_override
        .insert(RuleKind::UnusedPin, Severity::Off);
    select(&mut app, "User Default");
    let _ = app.handle_profiles_message(ProfilesMsg::ResetRequested);
    let _ = app.handle_profiles_message(ProfilesMsg::ResetConfirmed);
    assert!(app.ui_state.erc_severity_override.is_empty());
    assert!(matches!(
        app.ui_state.profile_writes.running().map(|d| &d.write),
        Some(QueuedWrite::UserDefault(_))
    ));
}

#[test]
fn rename_and_delete_say_what_happens_to_projects_that_recorded_the_profile() {
    let mut app = app();
    for action in [
        ProfileAction::Renamed {
            old: "A".into(),
            new: "B".into(),
        },
        ProfileAction::Deleted("A".into()),
    ] {
        let _ = app.handle_profiles_message(ProfilesMsg::ActionFinished(Arc::new(Ok(action))));
        let (_, text) = manager(&app).status.clone().unwrap();
        assert!(text.contains("keep their rules"), "{text}");
    }
}

#[test]
fn opening_preferences_forgets_the_last_sessions_questions_and_result() {
    let lib = tempfile::tempdir().unwrap();
    let mut app = listed_app(lib.path());
    app.ui_state.profile_manager.status = Some((true, "old result".into()));
    app.ui_state.profile_manager.confirm_delete = Some("Company".into());
    app.ui_state.profile_manager.confirm_reset = Some("Company".into());
    let _ = app.handle_preferences_open_requested();
    let m = manager(&app);
    assert!(m.status.is_none() && m.confirm_delete.is_none() && m.confirm_reset.is_none());
}
