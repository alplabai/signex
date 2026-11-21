//! #110 item 7 — a failed Enable Version Control is shown to the user and
//! lands in the Messages panel instead of only being logged.

use super::*;
use signex_i18n::RenderTranslatable;

fn open_dialog_on(project_dir: &std::path::Path) -> Signex {
    let (mut app, _task) = Signex::new();
    app.ui_state.enable_version_control = Some(crate::app::EnableVersionControlState {
        scope: crate::app::VersionControlScope::Project,
        project_path: project_dir.join("demo.snxprj"),
        project_dir: project_dir.to_path_buf(),
        project_name: "demo".to_string(),
        items: Vec::new(),
        use_lfs: false,
        commit_message: "Initial version".to_string(),
        commit_every_save: true,
        error: None,
        working: false,
        scanning: false,
    });
    app
}

/// Confirm and run the off-thread work to completion (#1122).
fn confirm(app: &mut Signex) {
    let task = app.handle_enable_version_control_confirm();
    crate::app::handlers::menu::export::settle(app, task);
}

/// Load a project "demo" at `dir` (no sheets, `enable_git` off).
fn push_demo_project(app: &mut Signex, dir: &std::path::Path) -> crate::app::state::ProjectId {
    let id = app.document_state.mint_project_id();
    app.document_state
        .projects
        .push(crate::app::state::LoadedProject {
            id,
            path: dir.join("demo.snxprj"),
            data: signex_types::project::ProjectData {
                document_id: None,
                description: None,
                name: "demo".to_string(),
                dir: dir.to_string_lossy().into_owned(),
                schematic_root: None,
                pcb_file: None,
                sheets: Vec::new(),
                variant_definitions: Vec::new(),
                active_variant: None,
                libraries: Vec::new(),
                enable_git: false,
                net_classes: Vec::new(),
                rules: None,
                annotation: None,
                schematic_defaults: Default::default(),
                project_defaults: Default::default(),
                agent_journal: Default::default(),
                schematic_design: Default::default(),
                fab_outputs: None,
                requirements: Default::default(),
                parameters: Default::default(),
                unknown: Default::default(),
            },
            pending_libraries: Default::default(),
            case_fold: Default::default(),
            case_probed: false,
            case_decided: false,
        });
    id
}

fn send(app: &mut Signex, msg: crate::app::EnableVersionControlMsg) {
    let _ = app.update(Message::EnableVersionControl(msg));
}

fn dialog(app: &Signex) -> &crate::app::EnableVersionControlState {
    app.ui_state
        .enable_version_control
        .as_ref()
        .expect("dialog")
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
    push_demo_project(&mut app, tmp.path());

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
    let notice = crate::app::state::ErrorNotice::version_control(crate::app::state::verbatim(
        "auto-commit is off",
    ));
    assert_eq!(notice.title.render_en_us(), "Version Control Setup Failed");
    assert!(notice.detail.render_en_us().contains("auto-commit is off"));
}

/// #1098: a successful Enable records `.git` in the Model at once, so the
/// menu and the open guards do not re-offer it before the re-probe lands.
/// Exercised on the step the success path runs, because a real init needs a
/// git identity that CI runners do not have.
#[test]
fn a_successful_enable_marks_the_dir_as_having_git_immediately() {
    let (mut app, _task) = Signex::new();
    let dir = std::path::Path::new("/nonexistent/demo-project");
    let marker = crate::app::state::project_fs::git_marker(dir);
    app.document_state
        .project_fs
        .facts
        .set_exists(marker, false);
    assert!(app.document_state.project_fs.facts.lacks_git(dir));

    app.record_git_present(dir);

    assert!(!app.document_state.project_fs.facts.lacks_git(dir));
}

/// A run in flight cannot be dismissed, so a second concurrent repository
/// init cannot start on the same directory.
#[test]
fn close_is_refused_while_a_run_is_in_flight() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut app = open_dialog_on(tmp.path());
    app.ui_state
        .enable_version_control
        .as_mut()
        .unwrap()
        .working = true;

    app.close_enable_version_control_dialog();
    assert!(app.ui_state.enable_version_control.is_some());

    app.ui_state
        .enable_version_control
        .as_mut()
        .unwrap()
        .working = false;
    app.close_enable_version_control_dialog();
    assert!(app.ui_state.enable_version_control.is_none());
}

/// The project was closed mid-run: the repository exists but no loaded
/// project can record it, so the user gets the error card, not a success.
#[test]
fn a_run_finishing_after_its_project_closed_raises_the_notice() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut app = open_dialog_on(tmp.path());

    app.handle_enable_version_control_finished(
        crate::app::VersionControlScope::Project,
        tmp.path().to_path_buf(),
        crate::app::EnableVersionControlOutcome::Created {
            adapter_error: None,
            commit_warning: None,
        },
    );

    let notice = app
        .document_state
        .error_notice
        .as_ref()
        .expect("a closed project must raise the version-control notice");
    assert_eq!(notice.title.render_en_us(), "Version Control Setup Failed");
    assert_eq!(
        notice.detail.render_en_us(),
        format!(
            "The repository at {} was created, but its project was closed \
             before auto-commit could be enabled. Delete the project's \
             .git folder, reopen the project, then enable version \
             control again.",
            tmp.path().display()
        )
    );
}

/// #1205 — a folder that holds other projects is refused before any
/// `git init`, with the reason on the dialog and in the Messages panel.
#[test]
fn a_folder_that_holds_other_projects_is_refused_before_git_init() {
    let _ = crate::diagnostics::init_logging();
    let tmp = tempfile::TempDir::new().unwrap();
    let nested = tmp.path().join("A");
    std::fs::create_dir(&nested).unwrap();
    std::fs::write(nested.join("A.snxprj"), b"").unwrap();
    std::fs::write(tmp.path().join("diary.txt"), b"private").unwrap();
    let mut app = open_dialog_on(tmp.path());

    confirm(&mut app);

    assert!(!tmp.path().join(".git").exists(), "nothing was initialised");
    let state = app.ui_state.enable_version_control.as_ref().expect("open");
    let error = state.error.as_deref().expect("the reason is shown");
    assert!(error.contains("another project"), "{error}");
    assert!(
        crate::diagnostics::recent_entries().iter().any(|entry| {
            entry.message.contains("enable version control failed")
                && entry.message.contains(&tmp.path().display().to_string())
        }),
        "the refusal must reach the Messages panel"
    );
}

/// #1205 — the old shared layout (several `.snxprj` side by side with
/// personal files) is refused: `git init` would stage the other projects.
#[test]
fn sibling_projects_in_the_folder_are_refused_before_git_init() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("demo.snxprj"), b"").unwrap();
    std::fs::write(tmp.path().join("other.snxprj"), b"").unwrap();
    std::fs::write(tmp.path().join("diary.txt"), b"private").unwrap();
    let mut app = open_dialog_on(tmp.path());

    confirm(&mut app);

    assert!(!tmp.path().join(".git").exists(), "nothing was initialised");
    let state = app.ui_state.enable_version_control.as_ref().expect("open");
    let error = state.error.as_deref().expect("the reason is shown");
    assert!(error.contains("more than this project"), "{error}");
}

/// Mockup row 26: the dialog opens with the catalog's first commit message for the
/// project, plain text (no isolation marks), and Commit every save on.
#[test]
fn the_dialog_opens_with_a_first_commit_message_and_commit_every_save_on() {
    let tmp = tempfile::TempDir::new().unwrap();
    let (mut app, _task) = Signex::new();
    push_demo_project(&mut app, tmp.path());
    app.document_state
        .project_fs
        .facts
        .set_exists(crate::app::state::project_fs::git_marker(tmp.path()), false);
    app.open_enable_version_control_dialog(vec![0]);
    let st = dialog(&app);
    assert_eq!(st.commit_message, "Initial version of demo");
    assert!(st.commit_every_save);
    assert_eq!(
        st.items.first().map(|i| i.kind),
        Some(crate::app::contracts::TrackKind::Project)
    );
}

/// Mockup row 26: the edited first commit message is the subject of the repository's
/// first commit.
#[test]
fn the_first_commit_carries_the_dialog_message() {
    let _ = crate::diagnostics::init_logging();
    signex_library::git_identity::test_identity::install();
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("demo.snxprj"), "x").unwrap();
    let mut app = open_dialog_on(tmp.path());
    push_demo_project(&mut app, tmp.path());
    send(
        &mut app,
        crate::app::EnableVersionControlMsg::CommitMessageChanged(
            "  First board of demo  ".to_string(),
        ),
    );

    confirm(&mut app);

    let repo = git2::Repository::open(tmp.path()).expect("a repository");
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(
        head.message().expect("utf-8 message"),
        "First board of demo",
        "trimmed"
    );
}

/// Mockup row 26: a blank first commit message withholds Enable: Confirm starts
/// nothing until the message has text again.
#[test]
fn a_blank_commit_message_refuses_confirm() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut app = open_dialog_on(tmp.path());
    send(
        &mut app,
        crate::app::EnableVersionControlMsg::CommitMessageChanged("   ".to_string()),
    );
    let task = app.handle_enable_version_control_confirm();
    assert_eq!(task.units(), 0, "no run starts");
    assert!(!dialog(&app).working);
    assert!(!tmp.path().join(".git").exists());
}

/// Mockup row 26: Select all ticks every row; edits wait while a run is in flight.
#[test]
fn select_all_ticks_every_row() {
    use crate::app::contracts::TrackKind;
    let tmp = tempfile::TempDir::new().unwrap();
    let mut app = open_dialog_on(tmp.path());
    let row = |name: &str, tracked: bool| crate::app::TrackItem {
        absolute: tmp.path().join(name),
        relative: name.to_string(),
        kind: TrackKind::Folder,
        is_directory: true,
        tracked,
    };
    app.ui_state.enable_version_control.as_mut().unwrap().items = vec![
        row("assets", true),
        row("outputs", false),
        row("docs", false),
    ];

    send(&mut app, crate::app::EnableVersionControlMsg::AllTicked);
    assert!(dialog(&app).items.iter().all(|i| i.tracked));

    send(&mut app, crate::app::EnableVersionControlMsg::ToggleItem(1));
    app.ui_state
        .enable_version_control
        .as_mut()
        .unwrap()
        .working = true;
    send(&mut app, crate::app::EnableVersionControlMsg::AllTicked);
    send(
        &mut app,
        crate::app::EnableVersionControlMsg::CommitEverySaveToggled,
    );
    let st = dialog(&app);
    assert!(!st.items[1].tracked, "frozen while the run is in flight");
    assert!(st.commit_every_save, "frozen while the run is in flight");
}

fn created() -> crate::app::EnableVersionControlOutcome {
    crate::app::EnableVersionControlOutcome::Created {
        adapter_error: None,
        commit_warning: None,
    }
}

/// Mockup row 26: with Commit every save turned off the repository is recorded but
/// `enable_git` stays off: no General step, nothing dirty.
#[test]
fn commit_every_save_off_leaves_enable_git_off() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut app = open_dialog_on(tmp.path());
    push_demo_project(&mut app, tmp.path());
    send(
        &mut app,
        crate::app::EnableVersionControlMsg::CommitEverySaveToggled,
    );
    assert!(!dialog(&app).commit_every_save);

    app.handle_enable_version_control_finished(
        crate::app::VersionControlScope::Project,
        tmp.path().to_path_buf(),
        created(),
    );

    assert!(!app.document_state.projects[0].data.enable_git);
    assert!(app.document_state.error_notice.is_none(), "no failure card");
    assert!(
        app.ui_state.enable_version_control.is_none(),
        "the dialog closes"
    );
    assert!(
        !app.document_state.project_fs.facts.lacks_git(tmp.path()),
        "the repository is recorded"
    );
}

/// Mockup row 26: with Commit every save on (as the dialog opens) a success turns
/// `enable_git` on, as before the switch existed.
#[test]
fn commit_every_save_on_turns_enable_git_on() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut app = open_dialog_on(tmp.path());
    push_demo_project(&mut app, tmp.path());
    assert!(dialog(&app).commit_every_save);

    app.handle_enable_version_control_finished(
        crate::app::VersionControlScope::Project,
        tmp.path().to_path_buf(),
        created(),
    );

    assert!(app.document_state.projects[0].data.enable_git);
}
