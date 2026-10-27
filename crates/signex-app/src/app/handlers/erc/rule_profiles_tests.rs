//! Rule profiles in the ERC flow (#1085): confirmed updates, the drift note,
//! the report header and the old-project guarantee.

use std::sync::Arc;

use signex_erc::project_rules::profile::{builtin_profile, profile_ref};
use signex_erc::{RuleKind, Severity};
use signex_output::{ErcReportExporter, ErcReportFormat, Exporter};
use signex_types::project::{RuleProfile, RuleProfileRef};

use crate::app::handlers::menu::export::tests::app_workspace;

fn workspace(tag: &str) -> crate::app::Signex {
    let dir = std::env::temp_dir().join(format!("signex-prof-{tag}-{}", uuid::Uuid::new_v4()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a_root.snxsch"]);
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    app
}

fn rules(app: &crate::app::Signex) -> Option<signex_types::project::ProjectRules> {
    app.document_state.projects[0].data.rules.clone()
}

fn is_dirty(app: &crate::app::Signex) -> bool {
    let path = &app.document_state.projects[0].path;
    app.document_state.dirty_paths.contains(path)
}

fn found(profile: RuleProfile) -> Arc<Result<Option<(RuleProfile, &'static str)>, String>> {
    Arc::new(Ok(Some((profile, "user"))))
}

/// A project whose rules were edited away from a stored profile.
fn edited_project(tag: &str) -> crate::app::Signex {
    let mut app = workspace(tag);
    let _ = app.handle_store_erc_rules_in_project();
    let _ = app.handle_erc_severity_changed(RuleKind::UnusedPin, Severity::Off);
    let _ = app.handle_erc_severity_changed(RuleKind::DanglingWire, Severity::Info);
    app
}

fn company_profile() -> RuleProfile {
    let mut p = builtin_profile();
    p.name = "Company".into();
    p.erc.severity.insert("unused_pin".into(), "error".into());
    p
}

#[test]
fn update_applies_only_after_confirm_and_is_one_undo_step() {
    let mut app = edited_project("confirm");
    let id = app.document_state.projects[0].id;
    let before = rules(&app);
    let _ = app.handle_profile_loaded_for_update(id, &found(company_profile()));

    let pending = app
        .ui_state
        .pending_profile_update
        .as_ref()
        .expect("diff shown");
    assert!(!pending.changes.is_empty());
    assert_eq!(rules(&app), before, "the diff alone changes nothing");

    let _ = app.handle_profile_update_confirmed();
    let after = rules(&app).unwrap();
    assert_eq!(after.erc.severity["unused_pin"], "error");
    assert_eq!(after.profile.as_ref().unwrap().name, "Company");
    assert_eq!(
        after.profile.as_ref().unwrap().sha256,
        company_profile().content_sha256()
    );
    assert!(app.ui_state.pending_profile_update.is_none());
    assert!(is_dirty(&app));

    assert!(app.undo_project_rules());
    assert_eq!(
        rules(&app),
        before,
        "one undo returns to the pre-update rules"
    );
}

#[test]
fn cancelling_the_diff_leaves_the_project_alone() {
    let mut app = edited_project("cancel");
    let id = app.document_state.projects[0].id;
    let before = rules(&app);
    let _ = app.handle_profile_loaded_for_update(id, &found(company_profile()));
    let _ = app.handle_profile_update_cancelled();
    let _ = app.handle_profile_update_confirmed();
    assert_eq!(rules(&app), before);
    assert!(app.ui_state.pending_profile_update.is_none());
}

#[test]
fn an_update_keeps_rule_ids_from_a_newer_build() {
    let mut app = edited_project("foreign");
    app.edit_project_erc_rules(|erc| {
        erc.severity.insert("future_rule".into(), "warning".into());
    });
    let id = app.document_state.projects[0].id;
    let _ = app.handle_profile_loaded_for_update(id, &found(company_profile()));
    let _ = app.handle_profile_update_confirmed();
    assert_eq!(rules(&app).unwrap().erc.severity["future_rule"], "warning");
}

#[test]
fn a_profile_missing_from_the_library_shows_no_diff() {
    let mut app = edited_project("missing");
    let id = app.document_state.projects[0].id;
    let before = rules(&app);
    let _ = app.handle_profile_loaded_for_update(id, &Arc::new(Ok(None)));
    assert!(app.ui_state.pending_profile_update.is_none());
    assert_eq!(rules(&app), before);
}

#[test]
fn matching_rules_and_hash_show_no_diff() {
    let mut app = workspace("uptodate");
    let _ = app.handle_store_erc_rules_in_project();
    let profile = builtin_profile();
    let id = app.document_state.projects[0].id;
    app.document_state.projects[0]
        .data
        .rules
        .as_mut()
        .unwrap()
        .profile = Some(profile_ref(&profile, "builtin"));
    let _ = app.handle_profile_loaded_for_update(id, &found(profile));
    assert!(app.ui_state.pending_profile_update.is_none());
}

#[test]
fn drift_is_noted_once_and_never_overwrites() {
    let mut app = workspace("drift");
    let _ = app.handle_store_erc_rules_in_project();
    let id = app.document_state.projects[0].id;
    let mut local = company_profile();
    local.name = format!("Drift-{}", uuid::Uuid::new_v4());
    let stale = RuleProfileRef {
        name: local.name.clone(),
        source: "user".into(),
        sha256: "stale".into(),
        ..Default::default()
    };
    app.document_state.projects[0]
        .data
        .rules
        .as_mut()
        .unwrap()
        .profile = Some(stale);
    let before = rules(&app);
    let dirty_before = is_dirty(&app);

    let needle = local.name.clone();
    let logged = crate::diagnostics::logged_count(&needle);
    let _ = app.handle_profile_drift_checked(id, &found(local.clone()));
    let _ = app.handle_profile_drift_checked(id, &found(local));
    assert_eq!(crate::diagnostics::logged_count(&needle), logged + 1);
    assert!(app.document_state.profile_drift_noted.contains(&id));
    assert_eq!(
        rules(&app),
        before,
        "the project's rules are never overwritten"
    );
    assert_eq!(is_dirty(&app), dirty_before);
    assert!(app.ui_state.pending_profile_update.is_none());
}

#[test]
fn no_drift_note_when_the_hash_still_matches() {
    let mut app = workspace("nodrift");
    let _ = app.handle_store_erc_rules_in_project();
    let id = app.document_state.projects[0].id;
    let mut local = company_profile();
    local.name = format!("Same-{}", uuid::Uuid::new_v4());
    app.document_state.projects[0]
        .data
        .rules
        .as_mut()
        .unwrap()
        .profile = Some(profile_ref(&local, "user"));
    let logged = crate::diagnostics::logged_count(&local.name);
    let _ = app.handle_profile_drift_checked(id, &found(local.clone()));
    assert_eq!(crate::diagnostics::logged_count(&local.name), logged);
    assert!(app.document_state.profile_drift_noted.is_empty());
}

#[test]
fn storing_the_untouched_defaults_records_the_shipped_profile() {
    let mut app = workspace("seedref");
    let _ = app.handle_store_erc_rules_in_project();
    let reference = rules(&app).unwrap().profile.expect("provenance");
    assert_eq!(reference.name, "Signex Default");
    assert_eq!(reference.source, "builtin");
    assert_eq!(reference.sha256, builtin_profile().content_sha256());

    let mut custom = workspace("seedref-custom");
    custom
        .ui_state
        .erc_severity_override
        .insert(RuleKind::DanglingWire, Severity::Off);
    let _ = custom.handle_store_erc_rules_in_project();
    assert!(rules(&custom).unwrap().profile.is_none());
}

#[test]
fn the_report_header_carries_the_profile_name_and_rule_set_hash() {
    let mut app = workspace("report");
    let _ = app.handle_store_erc_rules_in_project();
    app.ui_state
        .erc_violations_by_path
        .insert(std::path::PathBuf::from("a_root.snxsch"), Vec::new());
    let _ = app.begin_erc_run();
    let stored = rules(&app).unwrap();

    let opts = app.erc_report_options(ErcReportFormat::Text);
    let ctx = signex_output::ExportContext {
        sheets: Vec::new(),
        metadata: signex_output::ProjectMetadata::default(),
        netlist: None,
        netlist_sheets: Vec::new(),
        current_sheet: None,
        netlist_placements: Vec::new(),
    };
    let text = String::from_utf8(ErcReportExporter.export(&ctx, &opts).unwrap().bytes).unwrap();
    let want = format!(
        "Rule set: Signex Default (sha256 {})",
        stored.erc.sha256_hex()
    );
    assert!(text.lines().nth(1) == Some(want.as_str()), "{text}");
}

#[test]
fn an_old_project_stays_without_rules_and_clean_through_every_profile_path() {
    let mut app = workspace("old");
    let id = app.document_state.projects[0].id;
    let _ = app.begin_erc_run();
    let _ = app.handle_profile_drift_checked(id, &Arc::new(Ok(None)));
    let _ = app.handle_profile_update_requested();
    let _ = app.handle_profile_update_confirmed();
    let _ = app.handle_profile_save_requested("Mine".into());
    assert!(rules(&app).is_none());
    assert!(!is_dirty(&app));
    assert!(app.ui_state.pending_profile_update.is_none());
    let report_set = app.ui_state.erc_run_rule_set.clone().expect("identity");
    assert_eq!(report_set.profile_name, None);
}
