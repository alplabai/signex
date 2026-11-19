//! The Graphite ERC dialog (#2413): the severity tile counts, the "Profile"
//! menu rows and their messages, and the machine-rules notice text (#2413).

use signex_erc::{RuleKind, Severity};

use super::rule_profiles::{ProfileMenuFacts, profile_actions};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{ErcMsg, ErcProfileAction, Signex};

fn workspace(tag: &str) -> Signex {
    let dir = std::env::temp_dir().join(format!("signex-ercdlg-{tag}-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a_root.snxsch"]);
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    app
}

fn no_project() -> Signex {
    let (mut app, _) = Signex::new();
    assert!(app.document_state.active_document_project().is_none());
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    app
}

fn count_of(counts: [(Severity, usize); 4], severity: Severity) -> usize {
    counts
        .iter()
        .find(|(level, _)| *level == severity)
        .map_or(0, |(_, n)| *n)
}

#[test]
fn tile_counts_follow_the_machine_rules_and_cover_every_rule() {
    let mut app = no_project();
    let before = app.erc_dialog_severity_counts();
    assert_eq!(
        before.iter().map(|(_, n)| n).sum::<usize>(),
        RuleKind::ALL.len()
    );
    let errors = count_of(before, Severity::Error);
    let off = count_of(before, Severity::Off);
    let rule = RuleKind::ALL
        .into_iter()
        .find(|r| r.default_severity() == Severity::Error && !r.is_on_off_only())
        .expect("a rule that defaults to Error");
    app.ui_state
        .erc_severity_override
        .insert(rule, Severity::Off);
    let after = app.erc_dialog_severity_counts();
    assert_eq!(count_of(after, Severity::Error), errors - 1);
    assert_eq!(count_of(after, Severity::Off), off + 1);
    // The order is the tiles' order.
    let order: Vec<Severity> = after.iter().map(|(s, _)| *s).collect();
    assert_eq!(
        order,
        [
            Severity::Error,
            Severity::Warning,
            Severity::Info,
            Severity::Off
        ]
    );
}

#[test]
fn tile_counts_read_the_project_rules_once_it_stores_them() {
    let mut app = workspace("counts");
    let _ = app.handle_store_erc_rules_in_project();
    let rule = RuleKind::ALL
        .into_iter()
        .find(|r| r.default_severity() == Severity::Error && !r.is_on_off_only())
        .expect("a rule that defaults to Error");
    let errors = count_of(app.erc_dialog_severity_counts(), Severity::Error);
    let _ = app.handle_erc_severity_changed(rule, Severity::Warning);
    // The edit went to the project, not the machine.
    assert!(app.ui_state.erc_severity_override.is_empty());
    assert_eq!(
        count_of(app.erc_dialog_severity_counts(), Severity::Error),
        errors - 1
    );
}

#[test]
fn every_profile_menu_row_runs_the_message_of_its_old_button() {
    assert!(matches!(
        ErcProfileAction::StoreInProject.message(),
        ErcMsg::StoreRulesInProject
    ));
    assert!(matches!(
        ErcProfileAction::SaveAsProfile.message(),
        ErcMsg::ProfileSaveDialogOpened
    ));
    assert!(matches!(
        ErcProfileAction::UpdateFromProfile.message(),
        ErcMsg::ProfileUpdateRequested
    ));
    assert_eq!(
        ErcProfileAction::StoreInProject.label(),
        signex_i18n::ids::DIALOG_ERC_STORE_RULES
    );
    assert_eq!(
        ErcProfileAction::SaveAsProfile.label(),
        signex_i18n::ids::DIALOG_ERC_SAVE_AS_PROFILE
    );
    assert_eq!(
        ErcProfileAction::UpdateFromProfile.label(),
        signex_i18n::ids::DIALOG_ERC_UPDATE_FROM_PROFILE
    );
}

#[test]
fn profile_menu_rows_keep_the_conditions_of_the_old_buttons() {
    let facts = |has_project, stores_rules, unreadable_rules, has_profile| ProfileMenuFacts {
        has_project,
        stores_rules,
        unreadable_rules,
        has_profile,
    };
    // No project: nothing to store into, nothing stored.
    assert!(profile_actions(facts(false, false, false, false)).is_empty());
    // A project without rules: Store only.
    assert_eq!(
        profile_actions(facts(true, false, false, false)),
        [ErcProfileAction::StoreInProject]
    );
    // A newer rule set sits unread in the file: storing would replace it.
    assert!(profile_actions(facts(true, false, true, false)).is_empty());
    // Stored rules: Save as, and Update when they record their profile.
    assert_eq!(
        profile_actions(facts(true, true, false, false)),
        [ErcProfileAction::SaveAsProfile]
    );
    assert_eq!(
        profile_actions(facts(true, true, false, true)),
        [
            ErcProfileAction::SaveAsProfile,
            ErcProfileAction::UpdateFromProfile
        ]
    );
}

#[test]
fn the_profile_menu_toggles_and_a_row_closes_it_and_runs_its_action() {
    let mut app = workspace("menu");
    let _ = app.dispatch_erc_message(ErcMsg::OpenDialog);
    assert!(!app.ui_state.erc_profile_menu_open);
    let _ = app.dispatch_erc_message(ErcMsg::ProfileMenuToggled);
    assert!(app.ui_state.erc_profile_menu_open);
    let _ = app.dispatch_erc_message(ErcMsg::ProfileMenuToggled);
    assert!(!app.ui_state.erc_profile_menu_open);

    let _ = app.dispatch_erc_message(ErcMsg::ProfileMenuToggled);
    assert_eq!(
        app.erc_profile_actions(),
        [ErcProfileAction::StoreInProject]
    );
    let _ = app.dispatch_erc_message(ErcMsg::ProfileMenuRowChosen(
        ErcProfileAction::StoreInProject,
    ));
    assert!(!app.ui_state.erc_profile_menu_open);
    assert!(app.active_project_stores_rules());
    assert_eq!(app.erc_profile_actions(), [ErcProfileAction::SaveAsProfile]);
}

#[test]
fn a_profile_menu_with_no_rows_does_not_open() {
    let mut app = no_project();
    assert!(app.erc_profile_actions().is_empty());
    let _ = app.dispatch_erc_message(ErcMsg::ProfileMenuToggled);
    assert!(!app.ui_state.erc_profile_menu_open);
}

#[test]
fn closing_the_dialog_closes_the_profile_menu() {
    let mut app = workspace("close");
    let _ = app.dispatch_erc_message(ErcMsg::OpenDialog);
    let _ = app.dispatch_erc_message(ErcMsg::ProfileMenuToggled);
    assert!(app.ui_state.erc_profile_menu_open);
    let _ = app.dispatch_erc_message(ErcMsg::CloseDialog);
    assert!(!app.ui_state.erc_profile_menu_open);
}

/// #2413: the read-only notice is one sentence with single spaces; the
/// source line break no longer leaves a run of spaces inside it.
#[test]
fn the_read_only_notice_has_no_run_of_spaces() {
    let mut app = no_project();
    app.ui_state.machine_rules_locked = Some("the old file is unreadable.".into());
    let notice = app.machine_rules_notice().expect("a notice while locked");
    assert!(!notice.contains("  "), "double space in {notice:?}");
    assert!(
        notice.contains("old ERC preferences could not be merged"),
        "{notice:?}"
    );
}

#[test]
fn every_machine_rules_notice_has_single_spaces() {
    let mut app = no_project();
    app.ui_state.machine_rules_locked = None;
    app.ui_state.machine_rules_loading = true;
    let loading = app.machine_rules_notice().expect("a notice while loading");
    assert!(!loading.contains("  "), "{loading:?}");
    app.ui_state.machine_rules_loading = false;
    app.ui_state.machine_rules_problem = Some("disk full.".into());
    let problem = app
        .machine_rules_notice()
        .expect("a notice on a write problem");
    assert!(!problem.contains("  "), "{problem:?}");
}
