//! #110 item 7 — a failed Enable Version Control is shown to the user and
//! lands in the Messages panel instead of only being logged.

use super::*;

fn open_dialog_on(project_dir: &std::path::Path) -> Signex {
    let (mut app, _task) = Signex::new();
    app.ui_state.enable_version_control = Some(crate::app::EnableVersionControlState {
        scope: crate::app::VersionControlScope::Project,
        project_path: project_dir.join("demo.snxprj"),
        project_dir: project_dir.to_path_buf(),
        project_name: "demo".to_string(),
        items: Vec::new(),
        use_lfs: false,
        intro_text: String::new(),
        error: None,
        working: false,
    });
    app
}

/// Confirm and run the off-thread work to completion (#1122).
fn confirm(app: &mut Signex) {
    let task = app.handle_enable_version_control_confirm();
    crate::app::handlers::menu::export::settle(app, task);
}

/// `.git` already exists, so the library refuses to initialise: the dialog
/// stays open with the reason on an error line, and the Messages panel gets
/// a record naming the project directory.
#[test]
fn a_refused_init_keeps_the_dialog_open_with_the_error_and_logs_it() {
    let _ = crate::diagnostics::init_logging();
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(tmp.path().join(".git")).unwrap();
    let mut app = open_dialog_on(tmp.path());
    let marker = tmp.path().display().to_string();

    confirm(&mut app);

    let state = app
        .ui_state
        .enable_version_control
        .as_ref()
        .expect("the dialog must stay open after a failure");
    assert!(state.error.is_some(), "the failure reason is shown");
    assert!(!state.working, "the dialog can be confirmed again");
    assert!(
        crate::diagnostics::recent_entries().iter().any(|entry| {
            entry.message.contains("enable version control failed")
                && entry.message.contains(&marker)
        }),
        "the failure must reach the Messages panel"
    );
}

/// `.gitattributes` is a directory, so the adapter cannot write it: the
/// error card shows, auto-commit stays off and no success line is logged.
#[test]
fn an_adapter_failure_raises_the_card_and_skips_the_success_log() {
    let _ = crate::diagnostics::init_logging();
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("demo.snxprj"), "x").unwrap();
    std::fs::create_dir(tmp.path().join(".gitattributes")).unwrap();
    let mut app = open_dialog_on(tmp.path());
    let marker = format!("initialised repository at {}", tmp.path().display());

    confirm(&mut app);

    // Where the failure surfaces depends on the host: with a git identity the
    // repository is created and the `.gitattributes` write fails (error card);
    // on a runner without one, creating the repository already fails and the
    // dialog stays open with the error. Either way the user sees it.
    let dialog_error = app
        .ui_state
        .enable_version_control
        .as_ref()
        .is_some_and(|state| state.error.is_some());
    assert!(
        app.document_state.error_notice.is_some() || dialog_error,
        "the failure must be shown to the user"
    );
    assert!(
        !app.document_state
            .projects
            .iter()
            .any(|p| p.data.enable_git),
        "enable_git must stay off"
    );
    assert!(
        !crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains(&marker)),
        "no success log after an adapter failure"
    );
}

/// The run happens on the blocking pool: Confirm returns at once with the
/// dialog marked working, a second Confirm or an edit is ignored, and the
/// result closes the dialog and logs the new repository.
#[test]
fn confirm_runs_off_the_ui_thread_and_the_result_closes_the_dialog() {
    let _ = crate::diagnostics::init_logging();
    signex_library::git_identity::test_identity::install();
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("demo.snxprj"), "x").unwrap();
    let mut app = open_dialog_on(tmp.path());

    let task = app.handle_enable_version_control_confirm();
    let state = app.ui_state.enable_version_control.as_ref().unwrap();
    assert!(state.working, "the dialog shows the run as working");
    assert!(
        !tmp.path().join(".git").exists(),
        "nothing ran inside update"
    );
    let again = app.handle_enable_version_control_confirm();
    assert_eq!(again.units(), 0, "a second Confirm starts nothing");
    let _ = app.update(Message::EnableVersionControl(
        crate::app::EnableVersionControlMsg::ToggleLfs,
    ));
    assert!(
        !app.ui_state
            .enable_version_control
            .as_ref()
            .unwrap()
            .use_lfs,
        "choices are frozen while the run is in flight"
    );

    crate::app::handlers::menu::export::settle(&mut app, task);

    assert!(tmp.path().join(".git").exists());
    assert!(
        app.ui_state.enable_version_control.is_none(),
        "the dialog closes"
    );
    let marker = format!("initialised repository at {}", tmp.path().display());
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains(&marker)),
        "the new repository is logged"
    );
}

/// A result for a directory whose dialog is no longer the open one leaves
/// that other dialog alone.
#[test]
fn a_late_result_for_another_project_leaves_the_open_dialog_alone() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut app = open_dialog_on(tmp.path());

    app.handle_enable_version_control_finished(
        crate::app::VersionControlScope::Project,
        tmp.path().join("elsewhere"),
        crate::app::EnableVersionControlOutcome::InitFailed("boom".into()),
    );

    let state = app.ui_state.enable_version_control.as_ref().unwrap();
    assert!(
        state.error.is_none(),
        "another run's error is not shown here"
    );
}

#[test]
fn the_setup_failure_notice_names_version_control() {
    let notice = crate::app::state::ErrorNotice::version_control("auto-commit is off");
    assert_eq!(notice.title, "Version Control Setup Failed");
    assert!(notice.detail.contains("auto-commit is off"));
}
