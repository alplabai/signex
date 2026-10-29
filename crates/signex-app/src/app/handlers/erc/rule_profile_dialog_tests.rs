//! The Save-as-profile dialog and the machine's rules as the `User Default`
//! profile (#1608): validation, the overwrite question (confirm and cancel),
//! the write queue, and that nothing reaches `prefs.json`.

use signex_erc::{RuleKind, Severity};

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::rule_profile_ui::{QueuedWrite, WriteOutcome};
use crate::rule_profiles::{self, SaveOutcome};

fn workspace(tag: &str) -> crate::app::Signex {
    let dir = std::env::temp_dir().join(format!("signex-dlg-{tag}-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a_root.snxsch"]);
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    app
}

fn no_project() -> crate::app::Signex {
    let (mut app, _) = crate::app::Signex::new();
    assert!(app.document_state.active_document_project().is_none());
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    app
}

fn project_rules(app: &crate::app::Signex) -> Option<signex_types::project::ProjectRules> {
    app.document_state.projects[0].data.rules.clone()
}

fn is_dirty(app: &crate::app::Signex) -> bool {
    app.document_state
        .dirty_paths
        .contains(&app.document_state.projects[0].path)
}

/// A project that stores rules, with the Save-as dialog open.
fn with_dialog(tag: &str) -> crate::app::Signex {
    let mut app = workspace(tag);
    let _ = app.handle_store_erc_rules_in_project();
    app.document_state.dirty_paths.clear();
    let _ = app.handle_profile_save_dialog_opened();
    app
}

fn set_inputs(app: &mut crate::app::Signex, name: &str, version: &str) {
    let _ = app.handle_profile_save_name_changed(name.into());
    let _ = app.handle_profile_save_version_changed(version.into());
}

fn dialog(app: &crate::app::Signex) -> &crate::app::rule_profile_ui::SaveProfileDialog {
    app.ui_state
        .save_profile_dialog
        .as_ref()
        .expect("dialog open")
}

// ---- Save as profile ----

#[test]
fn the_dialog_opens_with_the_project_name_and_version_one_and_writes_nothing() {
    let app = with_dialog("open");
    let d = dialog(&app);
    assert_eq!(d.name, "ProjectA rules");
    assert_eq!(d.version, "1.0");
    assert!(!d.confirm_overwrite && !d.busy && d.error.is_none());
    assert!(app.ui_state.profile_writes.is_idle());
    assert!(!is_dirty(&app));
}

#[test]
fn the_dialog_does_not_open_for_a_project_without_rules() {
    let mut app = workspace("norules");
    let _ = app.handle_profile_save_dialog_opened();
    assert!(app.ui_state.save_profile_dialog.is_none());
}

#[test]
fn invalid_input_stays_in_the_dialog_with_a_reason_and_starts_no_write() {
    let mut app = with_dialog("invalid");
    for (name, version, needle) in [
        ("", "1.0", "name"),
        ("Mine", "", "version"),
        ("Signex Default", "1.0", "reserved"),
        ("User Default", "1.0", "reserved"),
    ] {
        set_inputs(&mut app, name, version);
        let task = app.handle_profile_save_submitted();
        assert_eq!(task.units(), 0, "{name:?}/{version:?} starts no write");
        let d = dialog(&app);
        assert!(!d.busy);
        let error = d
            .error
            .as_deref()
            .expect("a reason is shown")
            .to_lowercase();
        assert!(error.contains(needle), "{name:?}/{version:?}: {error}");
    }
    // Typing clears the stale reason.
    let _ = app.handle_profile_save_name_changed("Mine".into());
    assert!(dialog(&app).error.is_none());
}

#[test]
fn a_valid_save_starts_one_write_and_blocks_a_second() {
    let mut app = with_dialog("valid");
    set_inputs(&mut app, "Mine", "2.0");
    assert_eq!(app.handle_profile_save_submitted().units(), 1);
    assert!(dialog(&app).busy);
    assert_eq!(
        app.handle_profile_save_submitted().units(),
        0,
        "no double write"
    );
    // Typing is ignored while the write runs.
    let _ = app.handle_profile_save_name_changed("Changed".into());
    assert_eq!(dialog(&app).name, "Mine");
}

#[test]
fn a_taken_name_asks_before_replacing_and_changes_nothing_yet() {
    let mut app = with_dialog("ask");
    set_inputs(&mut app, "Mine", "1.0");
    let _ = app.handle_profile_save_submitted();
    let id = app.document_state.projects[0].id;
    let before = project_rules(&app);

    let _ = app.handle_profile_saved(id, &Ok(SaveOutcome::Exists));
    let d = dialog(&app);
    assert!(d.confirm_overwrite && !d.busy);
    assert_eq!(
        project_rules(&app),
        before,
        "the project does not record anything yet"
    );
    assert!(!is_dirty(&app));
    assert!(app.ui_state.profile_writes.is_idle());
}

#[test]
fn cancelling_the_overwrite_question_goes_back_and_never_writes() {
    let mut app = with_dialog("cancel");
    set_inputs(&mut app, "Mine", "1.0");
    let id = app.document_state.projects[0].id;
    let _ = app.handle_profile_saved(id, &Ok(SaveOutcome::Exists));
    let before = project_rules(&app);

    let _ = app.handle_profile_save_cancelled();
    assert!(!dialog(&app).confirm_overwrite, "back to the inputs");
    assert_eq!(app.handle_profile_save_overwrite_confirmed().units(), 0);

    let _ = app.handle_profile_save_cancelled();
    assert!(
        app.ui_state.save_profile_dialog.is_none(),
        "second cancel closes"
    );
    assert_eq!(project_rules(&app), before);
    assert!(!is_dirty(&app));
}

#[test]
fn confirming_the_overwrite_replaces_the_file_and_records_the_profile() {
    let lib = tempfile::tempdir().unwrap();
    let mut app = with_dialog("confirm");
    let id = app.document_state.projects[0].id;
    let erc = project_rules(&app).unwrap().erc;

    // The first save, as the worker would run it.
    let first =
        rule_profiles::save_named_profile(lib.path(), "Mine", "1.0", erc.clone(), false).unwrap();
    assert!(matches!(first, SaveOutcome::Saved(_)));
    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Off);
    let edited = project_rules(&app).unwrap().erc;
    assert_ne!(edited, erc);

    // Saving the edited rules under the same name finds the name taken.
    set_inputs(&mut app, "Mine", "2.0");
    let _ = app.handle_profile_save_submitted();
    let taken = rule_profiles::save_named_profile(lib.path(), "Mine", "2.0", edited.clone(), false)
        .unwrap();
    assert_eq!(taken, SaveOutcome::Exists);
    let _ = app.handle_profile_saved(id, &Ok(taken));
    let on_disk = std::fs::read(lib.path().join("mine.snxrules")).unwrap();

    // Only the confirmation replaces it.
    assert_eq!(app.handle_profile_save_overwrite_confirmed().units(), 1);
    assert_eq!(
        std::fs::read(lib.path().join("mine.snxrules")).unwrap(),
        on_disk
    );
    let done =
        rule_profiles::save_named_profile(lib.path(), "Mine", "2.0", edited.clone(), true).unwrap();
    let _ = app.handle_profile_saved(id, &Ok(done));

    assert!(app.ui_state.save_profile_dialog.is_none());
    let rules = project_rules(&app).unwrap();
    let recorded = rules.profile.expect("provenance recorded");
    assert_eq!(
        (recorded.name.as_str(), recorded.version.as_str()),
        ("Mine", "2.0")
    );
    assert_eq!(recorded.sha256, edited.sha256_hex());
    assert!(is_dirty(&app));
    let file: signex_types::project::RuleProfile =
        signex_types::project::RuleProfile::from_json_bytes(
            &std::fs::read(lib.path().join("mine.snxrules")).unwrap(),
        )
        .unwrap();
    assert_eq!(file.erc, edited);
    assert_eq!(file.version, "2.0");
}

#[test]
fn an_overwrite_message_without_the_question_is_ignored() {
    let mut app = with_dialog("stray");
    set_inputs(&mut app, "Mine", "1.0");
    assert_eq!(app.handle_profile_save_overwrite_confirmed().units(), 0);
    assert!(!dialog(&app).busy);
}

#[test]
fn a_failed_write_stays_in_the_dialog_with_the_reason() {
    let mut app = with_dialog("fail");
    let id = app.document_state.projects[0].id;
    let before = project_rules(&app);
    let _ = app.handle_profile_saved(id, &Err("disk full".into()));
    assert_eq!(dialog(&app).error.as_deref(), Some("disk full"));
    assert!(!dialog(&app).busy);
    assert_eq!(project_rules(&app), before);
}

#[test]
fn closing_the_erc_dialog_or_switching_project_drops_the_save_form() {
    let mut app = with_dialog("close");
    let _ = app.handle_close_erc_dialog();
    assert!(app.ui_state.save_profile_dialog.is_none());

    let mut app = with_dialog("switch");
    app.document_state.active_project = None;
    app.drop_pending_profile_update_for_other_project();
    assert!(app.ui_state.save_profile_dialog.is_none());
}

#[test]
fn escape_closes_the_save_form_first_then_the_erc_dialog() {
    let mut app = with_dialog("esc");
    app.ui_state.first_run_tour_open = false;
    app.ui_state.erc_dialog_open = true;
    let first = format!("{:?}", app.escape_overlay_message().expect("a rung"));
    assert!(first.contains("ProfileSaveCancelled"), "{first}");

    let id = app.document_state.projects[0].id;
    let _ = app.handle_profile_saved(id, &Ok(SaveOutcome::Exists));
    let _ = app.handle_profile_save_cancelled(); // back from the question
    assert!(
        format!("{:?}", app.escape_overlay_message().unwrap()).contains("ProfileSaveCancelled")
    );
    let _ = app.handle_profile_save_cancelled(); // closes the form

    let then = format!("{:?}", app.escape_overlay_message().expect("a rung"));
    assert!(then.contains("CloseDialog"), "{then}");
}

// ---- the machine's rules are the User Default profile ----

fn queued_user_default(app: &crate::app::Signex) -> signex_types::project::ErcRules {
    match app.ui_state.profile_writes.running().map(|d| &d.write) {
        Some(QueuedWrite::UserDefault(erc)) => erc.clone(),
        other => panic!("expected a queued User Default write, got {other:?}"),
    }
}

#[test]
fn the_no_project_dialog_writes_the_user_default_profile_not_prefs() {
    let lib = tempfile::tempdir().unwrap();
    let mut app = no_project();
    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Off);
    assert_eq!(
        app.ui_state.erc_severity_override[&RuleKind::UnusedPin],
        Severity::Off
    );

    // Run the queued write as the worker would, against a temp library.
    let write = app
        .ui_state
        .profile_writes
        .running()
        .cloned()
        .expect("queued");
    write.run(lib.path()).unwrap();
    let stored = rule_profiles::load_user_default(lib.path())
        .unwrap()
        .expect("profile created");
    assert_eq!(stored.name, rule_profiles::USER_DEFAULT_PROFILE_NAME);
    assert_eq!(stored.erc.severity["unused_pin"], "off");
    assert_eq!(
        app.handle_profile_write_finished(&Ok(WriteOutcome::Done(None)))
            .units(),
        0
    );
    assert!(app.ui_state.profile_writes.is_idle());

    // Loading it back gives the dialog the same rules (a restart).
    let mut fresh = no_project();
    fresh.ui_state.machine_rules_loading = true;
    let _ = fresh.handle_user_default_loaded(&Ok(Some(stored)));
    assert!(!fresh.ui_state.machine_rules_loading);
    assert_eq!(
        fresh.ui_state.erc_severity_override[&RuleKind::UnusedPin],
        Severity::Off
    );

    let prefs = std::fs::read_to_string(crate::fonts::prefs_file_path()).unwrap_or_default();
    assert!(
        !prefs.contains("erc_severity") && !prefs.contains("pin_matrix"),
        "the dialog no longer writes ERC rules to prefs.json"
    );
}

#[test]
fn a_pin_matrix_click_with_no_project_is_stored_in_the_profile_too() {
    let mut app = no_project();
    let _ = app.handle_pin_matrix_cell_cycled(1, 1);
    let erc = queued_user_default(&app);
    assert_eq!(
        erc.pin_matrix.len(),
        1,
        "one cell differs from the factory matrix"
    );
    assert!(!app.ui_state.pin_matrix_overrides.is_empty());
}

#[test]
fn quick_edits_are_written_in_order_and_the_last_snapshot_wins() {
    let lib = tempfile::tempdir().unwrap();
    let mut app = no_project();
    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Off);
    let first = app.ui_state.profile_writes.running().cloned().unwrap();
    // Two more edits while the first write is still running.
    assert_eq!(
        app.handle_erc_severity_changed(RuleKind::DanglingWire, Severity::Info)
            .units(),
        0
    );
    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Error);

    first.run(lib.path()).unwrap();
    // Each waiting write starts when the one before it ends.
    let mut started = 0;
    while let Some(write) = {
        let next = app.handle_profile_write_finished(&Ok(WriteOutcome::Done(None)));
        (next.units() == 1).then(|| app.ui_state.profile_writes.running().cloned().unwrap())
    } {
        started += 1;
        write.run(lib.path()).unwrap();
    }
    assert_eq!(started, 2, "both waiting snapshots ran, in order");

    let stored = rule_profiles::load_user_default(lib.path())
        .unwrap()
        .unwrap();
    assert_eq!(
        stored.erc.severity["unused_pin"], "error",
        "the newest edit is on disk"
    );
    assert_eq!(stored.erc.severity["dangling_wire"], "info");
    assert!(app.ui_state.profile_writes.is_idle());
}

#[test]
fn a_project_with_its_own_rules_leaves_the_machine_rules_alone() {
    let mut app = workspace("own");
    let _ = app.handle_store_erc_rules_in_project();
    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Off);
    let _ = app.handle_pin_matrix_cell_cycled(1, 1);
    assert!(app.ui_state.erc_severity_override.is_empty());
    assert!(app.ui_state.pin_matrix_overrides.is_empty());
    assert!(app.ui_state.profile_writes.is_idle());
}

#[test]
fn machine_rules_are_not_editable_until_the_profile_has_loaded() {
    let mut app = no_project();
    app.ui_state.machine_rules_loading = true;
    let task = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Off);
    assert_eq!(task.units(), 0);
    assert!(app.ui_state.erc_severity_override.is_empty());
    assert!(
        app.ui_state.profile_writes.is_idle(),
        "no snapshot of defaults is written"
    );
    assert_eq!(app.handle_pin_matrix_cell_cycled(1, 1).units(), 0);
    assert!(app.ui_state.pin_matrix_overrides.is_empty());
}

#[test]
fn an_unreadable_user_default_is_reported_and_never_replaced_by_an_edit() {
    let lib = tempfile::tempdir().unwrap();
    let file = lib.path().join("user-default.snxrules");
    std::fs::write(&file, b"{not json").unwrap();

    let mut app = no_project();
    app.ui_state.machine_rules_loading = true;
    let why = rule_profiles::load_user_default(lib.path()).unwrap_err();
    let _ = app.handle_user_default_loaded(&Err(why));
    assert!(!app.ui_state.machine_rules_loading);

    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Off);
    let write = app.ui_state.profile_writes.running().cloned().unwrap();
    assert!(write.run(lib.path()).is_err());
    assert_eq!(std::fs::read(&file).unwrap(), b"{not json");
}

#[test]
fn the_migration_is_followed_by_loading_the_user_default() {
    let mut app = no_project();
    let task =
        app.handle_default_profile_migrated(&Ok(rule_profiles::ProfileMigration::AlreadyDone));
    assert_eq!(task.units(), 1);
}

#[test]
fn loading_a_profile_with_rules_from_a_newer_build_keeps_the_known_ones() {
    let mut app = no_project();
    let mut profile = signex_erc::project_rules::profile::builtin_profile();
    profile.name = rule_profiles::USER_DEFAULT_PROFILE_NAME.into();
    profile
        .erc
        .severity
        .insert("unused_pin".into(), "off".into());
    profile
        .erc
        .severity
        .insert("future_rule".into(), "error".into());
    let _ = app.handle_user_default_loaded(&Ok(Some(profile)));
    assert_eq!(
        app.ui_state.erc_severity_override[&RuleKind::UnusedPin],
        Severity::Off
    );
}

// ---- review round ----

#[test]
fn the_dialog_says_when_the_machine_rules_are_loading_or_not_saving() {
    let mut app = no_project();
    assert_eq!(app.machine_rules_notice(), None);

    app.ui_state.machine_rules_loading = true;
    assert!(app.machine_rules_notice().unwrap().contains("Loading"));
    let _ = app.handle_user_default_loaded(&Err("bad json".into()));
    let notice = app.machine_rules_notice().unwrap();
    assert!(
        notice.contains("bad json") && notice.contains("not saving"),
        "{notice}"
    );

    // A write that fails keeps the notice; one that succeeds clears it.
    let _ = app.handle_user_default_loaded(&Ok(None));
    assert_eq!(app.machine_rules_notice(), None);
    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Off);
    let _ = app.handle_profile_write_finished(&Err("disk full".into()));
    assert!(app.machine_rules_notice().unwrap().contains("disk full"));
    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Info);
    let _ = app.handle_profile_write_finished(&Ok(WriteOutcome::Done(None)));
    assert_eq!(app.machine_rules_notice(), None);
}

#[test]
fn drift_is_checked_for_the_named_project_not_the_active_tab() {
    let mut app = workspace("drift-id");
    // A second project that recorded a profile; the first (active) one did not.
    let mut second = app.document_state.projects[0].clone();
    second.id = app.document_state.mint_project_id();
    second.path = second.path.with_file_name("Other.snxprj");
    second.data.rules = Some(signex_types::project::ProjectRules {
        profile: Some(signex_types::project::RuleProfileRef {
            name: "Company".into(),
            source: "user".into(),
            sha256: "recorded".into(),
            ..Default::default()
        }),
        ..Default::default()
    });
    let second_id = second.id;
    app.document_state.projects.push(second);
    let active = app.document_state.projects[0].id;
    app.document_state.active_project = Some(active);

    assert_eq!(
        app.check_profile_drift_task().units(),
        0,
        "the active project records none"
    );
    assert_eq!(app.check_profile_drift_task_for(second_id).units(), 1);
    assert_eq!(app.check_profile_drift_task_for(active).units(), 0);
}

#[test]
fn a_save_as_overwrite_asks_an_open_profile_pane_to_read_the_library_again() {
    let mut app = with_dialog("relist");
    let id = app.document_state.projects[0].id;
    let done = SaveOutcome::Saved(rule_profiles::SavedProfile {
        reference: signex_types::project::RuleProfileRef::default(),
        summary: "\"Mine\"".into(),
    });
    let closed = app.handle_profile_saved(id, &Ok(done.clone()));
    assert_eq!(closed.units(), 0, "no pane open, nothing to refresh");

    let mut app = with_dialog("relist2");
    app.ui_state.profile_manager.loaded = true;
    let id = app.document_state.projects[0].id;
    assert_eq!(app.handle_profile_saved(id, &Ok(done)).units(), 1);
}

// ---- re-review round ----

#[test]
fn a_refused_migration_keeps_the_machine_rules_read_only_for_the_session() {
    let mut app = no_project();
    let _ = app.handle_default_profile_migrated(&Err("prefs.json is not writable".into()));
    let notice = app.machine_rules_notice().expect("the dialog says so");
    assert!(
        notice.contains("read-only") && notice.contains("not writable"),
        "{notice}"
    );

    // The load that follows does not lift it, and no edit is taken, so the
    // fold of a later launch has nothing of this session to overwrite.
    let _ = app.handle_user_default_loaded(&Ok(None));
    assert!(app.machine_rules_notice().unwrap().contains("read-only"));
    assert_eq!(
        app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Off)
            .units(),
        0
    );
    assert_eq!(app.handle_pin_matrix_cell_cycled(1, 1).units(), 0);
    assert!(app.ui_state.erc_severity_override.is_empty());
    assert!(app.ui_state.pin_matrix_overrides.is_empty());
    assert!(
        app.ui_state.profile_writes.is_idle(),
        "nothing queued, nothing written"
    );
}

#[test]
fn a_write_the_new_project_already_ran_does_not_clear_a_reported_problem() {
    use crate::app::rule_profile_ui::WriteOutcome;
    let mut app = no_project();
    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Off);
    app.ui_state.machine_rules_problem = Some("disk full".into());
    let _ = app.handle_profile_write_finished(&Ok(WriteOutcome::Skipped));
    assert_eq!(
        app.ui_state.machine_rules_problem.as_deref(),
        Some("disk full"),
        "a skipped run says nothing about the file"
    );
    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Info);
    let _ = app.handle_profile_write_finished(&Ok(WriteOutcome::Done(None)));
    assert_eq!(app.ui_state.machine_rules_problem, None);
}

#[test]
fn the_first_listing_held_back_by_a_pending_write_is_read_again() {
    use crate::app::rule_profile_ui::WriteOutcome;
    use crate::preferences::ProfilesMsg;
    let mut app = no_project();
    assert!(
        !app.ui_state.profile_manager.loaded,
        "the pane has not loaded yet"
    );
    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Off);

    let listing = rule_profiles::list_library(None);
    let _ = app.handle_profiles_message(ProfilesMsg::Listed(std::sync::Arc::new(listing)));
    assert!(
        !app.ui_state.profile_manager.loaded,
        "held back, not applied"
    );
    assert!(app.ui_state.profile_manager.relist_pending);

    let next = app.handle_profile_write_finished(&Ok(WriteOutcome::Done(None)));
    assert_eq!(next.units(), 1, "the library is requested again");
}
