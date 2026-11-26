//! #2413 — the Graphite ERC dialog: the severity tiles count the rules the
//! dialog shows, and the "Profile" menu rows run the actions of the footer
//! buttons they replace.
//!
//! The shared rule editor has no row filter yet, so the tiles are read-only
//! counts; a click on the Errors tile listing only error rules waits on it.

use signex_app::app::{ErcMsg, ErcProfileAction, FileMsg, Message, Signex};
use signex_erc::{RuleKind, Severity};
use tempfile::TempDir;

use crate::preferences_prefs_recovery::{PrefsPathGuard, serial};
use crate::rule_profiles_ui::ProfilesDirGuard;
use crate::settle_support::settle;

fn count_of(app: &Signex, severity: Severity) -> usize {
    app.erc_dialog_severity_counts()
        .iter()
        .find(|(level, _)| *level == severity)
        .map_or(0, |(_, n)| *n)
}

#[test]
fn the_open_erc_dialog_counts_its_rules_per_severity() {
    let (mut app, _t) = Signex::new();
    assert!(app.document_state.active_document_project().is_none());
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    let _ = app.update(Message::Erc(ErcMsg::OpenDialog));
    assert!(app.ui_state.erc_dialog_open);

    let defaults = |severity: Severity| {
        RuleKind::ALL
            .into_iter()
            .filter(|r| r.default_severity() == severity)
            .count()
    };
    for severity in [
        Severity::Error,
        Severity::Warning,
        Severity::Info,
        Severity::Off,
    ] {
        assert_eq!(count_of(&app, severity), defaults(severity), "{severity:?}");
    }

    // Two error rules turned off move two from the Errors tile to Off.
    let demoted: Vec<RuleKind> = RuleKind::ALL
        .into_iter()
        .filter(|r| r.default_severity() == Severity::Error && !r.is_on_off_only())
        .take(2)
        .collect();
    assert_eq!(demoted.len(), 2, "two rules default to Error");
    for rule in &demoted {
        app.ui_state
            .erc_severity_override
            .insert(*rule, Severity::Off);
    }
    assert_eq!(
        count_of(&app, Severity::Error),
        defaults(Severity::Error) - 2
    );
    assert_eq!(count_of(&app, Severity::Off), defaults(Severity::Off) + 2);
}

#[test]
fn the_profile_menu_opens_save_as_profile_and_closes() {
    let _serial = serial();
    let _prefs = PrefsPathGuard::capture();
    let _lib = ProfilesDirGuard::empty();
    let tmp = TempDir::new().unwrap();
    let (mut app, _t) = Signex::new();
    settle(
        &mut app,
        Message::File(FileMsg::NewProject(Some(tmp.path().join("Demo.snxprj")))),
    );
    assert!(app.document_state.error_notice.is_none());
    let _ = app.update(Message::Erc(ErcMsg::OpenDialog));

    // A new project stores the rules it was seeded with.
    let actions = app.erc_profile_actions();
    assert_eq!(actions.first(), Some(&ErcProfileAction::SaveAsProfile));
    assert!(!actions.contains(&ErcProfileAction::StoreInProject));

    let _ = app.update(Message::Erc(ErcMsg::ProfileMenuToggled));
    assert!(app.ui_state.erc_profile_menu_open);
    let _ = app.update(Message::Erc(ErcMsg::ProfileMenuRowChosen(
        ErcProfileAction::SaveAsProfile,
    )));
    assert!(!app.ui_state.erc_profile_menu_open);
    assert!(
        app.ui_state.save_profile_dialog.is_some(),
        "the row opens the Save as profile form, as the old button did"
    );
}
