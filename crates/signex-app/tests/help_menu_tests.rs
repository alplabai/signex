// This integration-test binary discards fallible test-setup calls with
// `let _ = ...` routinely; see `passive_calculator_tests.rs`.
#![expect(
    clippy::let_underscore_must_use,
    reason = "integration tests discard Results with `let _ = ...` routinely; this is test scaffolding, not a production `iced::Task` being dropped, and `[lints]` in Cargo.toml is package-scoped so each integration-test crate root needs its own"
)]

use signex_app::app::{Message, OverlayMsg, Signex};
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
fn copy_version_info_schedules_a_clipboard_write() {
    let (mut app, _startup) = Signex::new();
    let task = app.update(Message::Overlay(OverlayMsg::CopyAboutInfo));
    assert_eq!(task.units(), 1);
}
