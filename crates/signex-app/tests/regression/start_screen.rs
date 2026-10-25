//! #331 — the empty centre region is an actionable start screen when
//! nothing is loaded, and a quieter hint when a project is loaded but
//! no document is selected. Every start-screen button must dispatch an
//! existing, working message.

use signex_app::app::{FileMsg, Message, Signex};
use signex_app::menu_bar::MenuMessage;
use signex_app::start_screen::{self, Action, Surface};

fn surface_of(app: &Signex) -> Surface {
    Surface::select(app.document_state.active_project.is_some())
}

#[test]
fn fresh_app_shows_the_start_screen() {
    let (app, _t) = Signex::new();
    assert_eq!(surface_of(&app), Surface::Start);
}

#[test]
fn project_without_document_shows_the_quiet_hint() {
    let (mut app, _t) = Signex::new();
    let dir = tempfile::tempdir().expect("tempdir");
    let prj = dir.path().join("Start.snxprj");
    crate::settle_support::settle(&mut app, Message::File(FileMsg::NewProject(Some(prj))));
    assert!(app.document_state.active_project.is_some());
    assert_eq!(surface_of(&app), Surface::ProjectHint);
}

#[test]
fn every_action_dispatches_the_existing_menu_message() {
    for action in Action::ALL {
        let ok = match action {
            Action::NewProject => {
                matches!(action.message(), Message::Menu(MenuMessage::NewProject))
            }
            Action::OpenProject => {
                matches!(action.message(), Message::Menu(MenuMessage::OpenProject))
            }
            Action::OpenSampleProject => matches!(
                action.message(),
                Message::Menu(MenuMessage::OpenSampleProject)
            ),
            Action::KeyboardShortcuts => matches!(
                action.message(),
                Message::Menu(MenuMessage::OpenKeyboardShortcuts)
            ),
        };
        assert!(ok, "{action:?} dispatched the wrong message");
    }
}

#[test]
fn keyboard_shortcuts_action_opens_the_modal_through_real_update() {
    let (mut app, _t) = Signex::new();
    assert!(!app.ui_state.keyboard_shortcuts_open);
    let _ = app.update(Action::KeyboardShortcuts.message());
    assert!(app.ui_state.keyboard_shortcuts_open);
}

#[test]
fn new_and_open_actions_reach_their_handlers_without_panicking() {
    // Both open a native file dialog as a Task (not runnable headless),
    // so the observable contract is that update accepts the message and
    // leaves the app on the start screen.
    let (mut app, _t) = Signex::new();
    for action in [Action::NewProject, Action::OpenProject] {
        let _ = app.update(action.message());
    }
    assert_eq!(surface_of(&app), Surface::Start);
}

#[test]
fn open_sample_action_schedules_the_unpack_through_real_update() {
    let (mut app, _t) = Signex::new();
    let task = app.update(Action::OpenSampleProject.message());
    assert_eq!(task.units(), 1, "the sample unpack must be scheduled");
}

#[test]
fn a_recent_entry_opens_its_project_through_real_update() {
    let (mut app, _t) = Signex::new();
    let dir = tempfile::tempdir().expect("tempdir");
    let prj = dir.path().join("Recent.snxprj");
    std::fs::write(&prj, b"").expect("create project");
    assert!(app.document_state.active_project.is_none());

    let _ = app.update(start_screen::recent_message(&prj));

    assert!(
        app.document_state.projects.iter().any(|p| p.path == prj),
        "the recent entry must open the project"
    );
}

#[test]
fn a_vanished_recent_entry_is_handled_like_open_recent() {
    let (mut app, _t) = Signex::new();
    let dir = tempfile::tempdir().expect("tempdir");
    let gone = dir.path().join("Gone.snxprj");
    app.ui_state.recent_projects =
        signex_app::recent_projects::RecentProjects::from_paths([gone.clone()]);
    assert_eq!(
        start_screen::visible_recent(app.ui_state.recent_projects.entries()).len(),
        1
    );

    let _ = app.update(start_screen::recent_message(&gone));

    assert!(app.ui_state.recent_projects.entries().is_empty());
    assert!(app.document_state.error_notice.is_some());
    assert!(app.document_state.projects.is_empty());
}
