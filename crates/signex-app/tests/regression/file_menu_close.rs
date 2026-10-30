//! #1765 — File ▸ Close Document, Close Project and Print reach the same
//! handlers the tab context menu, the project tree and the Ctrl+P command
//! already use.

use signex_app::app::{FileMsg, Message, Signex, TabInfo, TabKind};
use signex_app::menu_bar::MenuMessage;
use std::path::PathBuf;

fn push_footprint_tab(app: &mut Signex, name: &str) {
    let path = PathBuf::from(name);
    app.document_state.tabs.push(TabInfo {
        title: name.into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
}

#[test]
fn close_document_closes_only_the_active_tab() {
    let (mut app, _t) = Signex::new();
    push_footprint_tab(&mut app, "one.snxfpt");
    push_footprint_tab(&mut app, "two.snxfpt");

    let _ = app.update(Message::Menu(MenuMessage::CloseDocument));

    assert_eq!(app.document_state.tabs.len(), 1);
    assert_eq!(app.document_state.tabs[0].title, "one.snxfpt");
}

#[test]
fn close_document_with_nothing_open_does_nothing() {
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Menu(MenuMessage::CloseDocument));
    assert!(app.document_state.tabs.is_empty());
}

#[test]
fn close_project_closes_the_active_project() {
    let (mut app, _t) = Signex::new();
    let dir = tempfile::tempdir().expect("tempdir");
    let prj = dir.path().join("Close.snxprj");
    crate::settle_support::settle(&mut app, Message::File(FileMsg::NewProject(Some(prj))));
    assert!(app.document_state.active_project.is_some());

    crate::settle_support::settle(&mut app, Message::Menu(MenuMessage::CloseProject));

    assert!(app.document_state.active_project.is_none());
    assert!(app.document_state.projects.is_empty());
}

#[test]
fn close_project_with_no_project_does_nothing() {
    let (mut app, _t) = Signex::new();
    let _ = app.update(Message::Menu(MenuMessage::CloseProject));
    assert!(app.document_state.projects.is_empty());
}
