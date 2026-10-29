//! #271 — the `open_project_options` command opens the active project's
//! Project tab through the same message the palette and the canvas menu use.

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, TabKind};
use crate::menu_bar::MenuMessage;

#[test]
fn open_project_options_opens_the_active_projects_tab() {
    let mut app = app_workspace("/p", &["a_root.snxsch"]);
    let id = app.document_state.projects[0].id;
    assert!(
        app.document_state
            .tabs
            .iter()
            .all(|tab| tab.kind.as_project().is_none()),
        "no Project tab before the command"
    );

    let _ = app.update(Message::Menu(MenuMessage::OpenProjectOptions));

    let tab = app
        .document_state
        .tabs
        .get(app.document_state.active_tab)
        .expect("an active tab");
    assert!(matches!(tab.kind, TabKind::Project(p) if p == id));
}

#[test]
fn open_project_options_with_no_project_opens_nothing() {
    let (mut app, _boot) = crate::app::Signex::new();
    let tabs = app.document_state.tabs.len();
    let _ = app.update(Message::Menu(MenuMessage::OpenProjectOptions));
    assert_eq!(app.document_state.tabs.len(), tabs);
}
