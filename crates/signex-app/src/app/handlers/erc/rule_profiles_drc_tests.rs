//! Rule profiles and the `drc` section (#2307 step 2b): the rule-set hash,
//! drift, Update from profile and Save as profile cover DRC rules.

use std::sync::Arc;

use signex_erc::project_rules::profile::{builtin_profile, profile_ref};
use signex_types::project::{DrcRuleSet, ProjectRules, RuleProfile, rule_set_sha256};

use crate::app::handlers::menu::export::tests::app_workspace;

fn workspace(tag: &str) -> crate::app::Signex {
    let dir = std::env::temp_dir().join(format!("signex-drcprof-{tag}-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a_root.snxsch"]);
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    app.ui_state.machine_drc_rules = DrcRuleSet::default();
    app
}

fn rules(app: &crate::app::Signex) -> ProjectRules {
    app.document_state.projects[0]
        .data
        .rules
        .clone()
        .expect("rules stored")
}

fn rules_mut(app: &mut crate::app::Signex) -> &mut ProjectRules {
    app.document_state.projects[0]
        .data
        .rules
        .as_mut()
        .expect("rules stored")
}

fn found(profile: RuleProfile) -> Arc<Result<Option<(RuleProfile, &'static str)>, String>> {
    Arc::new(Ok(Some((profile, "user"))))
}

/// The shipped ERC rules plus a DRC section, under a name unique to the test
/// so log counts do not cross tests.
fn drc_profile(tag: &str) -> RuleProfile {
    let mut p = builtin_profile();
    p.name = format!("Board-{tag}-{}", uuid::Uuid::now_v7());
    p.drc
        .severity
        .insert("builtin::unconnected".into(), "error".into());
    p.drc.dsl = Some("rule wide on track then error".into());
    p
}

/// A project storing the shipped ERC rules, recorded against `profile`.
fn project_recorded_against(tag: &str, profile: &RuleProfile) -> crate::app::Signex {
    let mut app = workspace(tag);
    let _ = app.handle_store_erc_rules_in_project();
    rules_mut(&mut app).profile = Some(profile_ref(profile, "user"));
    app
}

#[test]
fn a_drc_only_difference_is_listed_and_applied_as_one_undo_step() {
    let profile = drc_profile("update");
    let mut app = project_recorded_against("update", &profile);
    rules_mut(&mut app)
        .drc
        .severity
        .insert("builtin::clearance".into(), "warning".into());
    let before = rules(&app);
    let id = app.document_state.projects[0].id;

    let _ = app.handle_profile_loaded_for_update(id, &found(profile.clone()));
    let (_, lines) = app.pending_profile_update_lines().expect("diff shown");
    assert_eq!(
        lines,
        [
            "DRC severity builtin::clearance: warning -> default",
            "DRC severity builtin::unconnected: default -> error",
            "DRC rule text: added (1 line)",
        ]
    );
    assert_eq!(rules(&app), before, "the diff alone changes nothing");

    let _ = app.handle_profile_update_confirmed();
    let after = rules(&app);
    assert_eq!(after.drc, profile.drc);
    assert_eq!(after.erc, before.erc);
    assert_eq!(after.profile.unwrap().sha256, profile.content_sha256());
    assert!(app.undo_project_rules());
    assert_eq!(rules(&app), before);
}

#[test]
fn a_drc_edit_after_the_diff_was_shown_blocks_apply() {
    let profile = drc_profile("stale");
    let mut app = project_recorded_against("stale", &profile);
    let id = app.document_state.projects[0].id;
    let _ = app.handle_profile_loaded_for_update(id, &found(profile));
    assert!(app.ui_state.pending_profile_update.is_some());
    rules_mut(&mut app).drc.dsl = Some("rule mine on via then info".into());
    let before = rules(&app);
    let _ = app.handle_profile_update_confirmed();
    assert_eq!(rules(&app), before, "what was not shown is never applied");
}

#[test]
fn matching_drc_and_hash_show_no_diff() {
    let profile = drc_profile("same");
    let mut app = project_recorded_against("same", &profile);
    rules_mut(&mut app).drc = profile.drc.clone();
    let id = app.document_state.projects[0].id;
    let _ = app.handle_profile_loaded_for_update(id, &found(profile));
    assert!(app.ui_state.pending_profile_update.is_none());
}

#[test]
fn a_drc_only_profile_change_is_noted_as_drift() {
    let profile = drc_profile("drift");
    let mut app = project_recorded_against("drift", &profile);
    rules_mut(&mut app).drc = profile.drc.clone();
    let id = app.document_state.projects[0].id;
    let mut local = profile.clone();
    local
        .drc
        .severity
        .insert("builtin::short".into(), "warning".into());
    let before = rules(&app);
    let logged = crate::diagnostics::logged_count(&profile.name);
    let _ = app.handle_profile_drift_checked(id, &found(local));
    assert_eq!(crate::diagnostics::logged_count(&profile.name), logged + 1);
    assert_eq!(rules(&app), before, "drift never overwrites");
}

/// A project recorded against a profile with `drc` before the section
/// entered the hash holds the ERC-only hash. It reads as drifted and as
/// modified once; Update from profile lists nothing and refreshes the record.
#[test]
fn a_record_from_before_drc_entered_the_hash_is_refreshed_by_one_update() {
    let profile = drc_profile("legacy");
    let mut app = project_recorded_against("legacy", &profile);
    let stored = rules_mut(&mut app);
    stored.drc = profile.drc.clone();
    stored.profile.as_mut().unwrap().sha256 = profile.erc.sha256_hex();
    let before = rules(&app);
    let id = app.document_state.projects[0].id;

    let logged = crate::diagnostics::logged_count(&profile.name);
    let _ = app.handle_profile_drift_checked(id, &found(profile.clone()));
    assert_eq!(crate::diagnostics::logged_count(&profile.name), logged + 1);
    let modified = format!("{} (modified)", profile.name);
    let identity = app.current_rule_set_identity();
    assert_eq!(identity.profile_name.as_deref(), Some(modified.as_str()));
    assert_eq!(identity.sha256, before.erc.sha256_hex());

    let _ = app.handle_profile_loaded_for_update(id, &found(profile.clone()));
    let (_, lines) = app.pending_profile_update_lines().expect("record refresh");
    assert!(lines.is_empty(), "{lines:?}");
    let _ = app.handle_profile_update_confirmed();
    let after = rules(&app);
    assert_eq!((&after.erc, &after.drc), (&before.erc, &before.drc));
    assert_eq!(after.profile.unwrap().sha256, profile.content_sha256());
    assert_eq!(
        app.current_rule_set_identity().profile_name,
        Some(profile.name)
    );
}

#[test]
fn a_drc_only_edit_marks_the_erc_report_rule_set_modified() {
    let mut app = workspace("modified");
    let _ = app.handle_store_erc_rules_in_project();
    rules_mut(&mut app).drc.dsl = Some("rule a on track then error".into());
    let identity = app.current_rule_set_identity();
    assert_eq!(
        identity.profile_name.as_deref(),
        Some("Signex Default (modified)")
    );
    assert_eq!(identity.sha256, rules(&app).erc.sha256_hex());
}

/// The shipped profile has no `drc`, so rules stored with the machine's DRC
/// section are not that profile and record no provenance.
#[test]
fn storing_rules_with_machine_drc_records_no_shipped_profile() {
    let mut app = workspace("machine-drc");
    app.ui_state.machine_drc_rules.dsl = Some("rule a on via then info".into());
    let _ = app.handle_store_erc_rules_in_project();
    let stored = rules(&app);
    assert_eq!(stored.drc, app.ui_state.machine_drc_rules);
    assert!(stored.profile.is_none());
}

#[test]
fn save_as_profile_carries_the_drc_section_and_records_its_hash() {
    let mut app = workspace("save");
    let _ = app.handle_store_erc_rules_in_project();
    rules_mut(&mut app).drc = drc_profile("save").drc;
    let lib = tempfile::tempdir().unwrap();
    let project = app.document_state.projects[0].id;
    let profile = crate::rule_profiles::profile_of_project_rules(&rules(&app));
    let result = crate::rule_profiles::save_profile_as(lib.path(), profile, "Mine", "1.0", false);
    let _ = app.handle_profile_saved(project, &result);

    let on_disk =
        RuleProfile::from_json_bytes(&std::fs::read(lib.path().join("mine.snxrules")).unwrap())
            .unwrap();
    let stored = rules(&app);
    assert_eq!(on_disk.drc, stored.drc);
    assert_eq!(on_disk.erc, stored.erc);
    let recorded = stored.profile.unwrap().sha256;
    assert_eq!(recorded, on_disk.content_sha256());
    assert_eq!(recorded, rule_set_sha256(&stored.erc, &stored.drc));
    assert_eq!(
        app.current_rule_set_identity().profile_name.as_deref(),
        Some("Mine")
    );
}
