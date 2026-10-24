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
    });
    app
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

    app.handle_enable_version_control_confirm();

    let state = app
        .ui_state
        .enable_version_control
        .as_ref()
        .expect("the dialog must stay open after a failure");
    assert!(state.error.is_some(), "the failure reason is shown");
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

    app.handle_enable_version_control_confirm();

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

#[test]
fn the_setup_failure_notice_names_version_control() {
    let notice = crate::app::state::ErrorNotice::version_control("auto-commit is off");
    assert_eq!(notice.title, "Version Control Setup Failed");
    assert!(notice.detail.contains("auto-commit is off"));
}
