// This integration-test binary discards fallible test-setup calls with
// `let _ = ...` routinely; see `passive_calculator_tests.rs`.
#![expect(
    clippy::let_underscore_must_use,
    reason = "integration tests discard Results with `let _ = ...` routinely; this is test scaffolding, not a production `iced::Task` being dropped, and `[lints]` in Cargo.toml is package-scoped so each integration-test crate root needs its own"
)]

use signex_app::app::{FileMsg, Message, OverlayMsg, Signex};
use signex_app::menu_bar::MenuMessage;

#[test]
fn about_menu_item_opens_the_dialog_and_close_dismisses_it() {
    let (mut app, _startup) = Signex::new();
    assert!(!app.ui_state.about_open);

    let _ = app.update(Message::Menu(MenuMessage::OpenAbout));
    assert!(app.ui_state.about_open, "Help > About must open the dialog");

    let _ = app.update(Message::Overlay(OverlayMsg::CloseAbout));
    assert!(!app.ui_state.about_open);
}

#[test]
fn escape_closes_the_about_dialog() {
    let (mut app, _startup) = Signex::new();
    // Keep the first-run tour out of the ladder so About is the only rung.
    app.ui_state.first_run_tour_open = false;
    let _ = app.update(Message::Menu(MenuMessage::OpenAbout));

    let _ = app.update(Message::EscapePressed { window: None });

    assert!(!app.ui_state.about_open, "Esc must dismiss About");
}

#[test]
fn documentation_and_issue_items_return_a_browser_task() {
    let (mut app, _startup) = Signex::new();
    for item in [MenuMessage::OpenDocumentation, MenuMessage::ReportIssue] {
        let task = app.update(Message::Menu(item));
        assert_eq!(task.units(), 1, "a link item must schedule one task");
    }
}

#[test]
fn open_sample_project_unpacks_opens_and_never_clobbers_edits() {
    let (mut app, _startup) = Signex::new();
    let root = signex_app::sample_project::sample_root().expect("test config root");
    let _ = std::fs::remove_dir_all(&root);

    let task = app.update(Message::Menu(MenuMessage::OpenSampleProject));
    assert_eq!(task.units(), 1, "the unpack must be scheduled as a task");
    assert!(!root.exists(), "update must do no IO before the task runs");

    // Run what the task would run, then feed its completion message.
    signex_app::sample_project::ensure_extracted(&root).expect("unpack");
    let _ = app.update(Message::File(FileMsg::SampleProjectUnpacked(Ok(
        root.clone()
    ))));

    let project = root.join("linear-regulator.snxprj");
    let sheet = root.join("linear-regulator.snxsch");
    assert!(
        project.exists() && sheet.exists(),
        "sample must be unpacked"
    );
    assert!(
        app.document_state
            .projects
            .iter()
            .any(|p| p.path == project),
        "sample project must be loaded"
    );
    assert!(
        app.document_state.pending_opens.contains(&sheet),
        "sample sheet must be opening"
    );

    // A second invocation keeps the tester's edits.
    std::fs::write(&sheet, "edited").expect("edit sample");
    signex_app::sample_project::ensure_extracted(&root).expect("re-unpack");
    assert_eq!(std::fs::read_to_string(&sheet).expect("read"), "edited");
}

#[test]
fn copy_version_info_schedules_a_clipboard_write() {
    let (mut app, _startup) = Signex::new();
    let task = app.update(Message::Overlay(OverlayMsg::CopyAboutInfo));
    assert_eq!(task.units(), 1);
}
