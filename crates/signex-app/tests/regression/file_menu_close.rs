//! #1765 — File ▸ Close Document, Close Project and Print reach the same
//! handlers the tab context menu, the project tree and the Ctrl+P command
//! already use.

use signex_app::app::{FileMsg, Message, Signex, TabInfo, TabKind, UiMsg};
use signex_app::keymap::ShortcutTrigger;
use signex_app::menu_bar::MenuMessage;
use std::path::PathBuf;

use crate::primitive_dirty_quit::{edit_description_via_panel, open_footprint_editor};

fn press(app: &mut Signex, keys: &str) {
    let ShortcutTrigger::KeySequence(mut strokes) = ShortcutTrigger::parse(keys).unwrap() else {
        panic!("{keys} is a key, not a gesture");
    };
    let _ = app.update(Message::Ui(UiMsg::KeymapStroke {
        window: None,
        stroke: strokes.remove(0),
    }));
}

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

/// #1765: the catalog command the menu row now shows (Ctrl+F4) closes the
/// active tab the same way the row does, parking a dirty draft.
#[test]
fn close_active_document_command_closes_the_tab_and_parks_a_dirty_draft() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("draft.snxfpt");
    let (mut app, _t) = Signex::new();
    open_footprint_editor(&mut app, &path);
    edit_description_via_panel(&mut app);

    press(&mut app, "Ctrl+F4");

    assert!(app.document_state.tabs.is_empty(), "the tab closes");
    assert!(
        app.document_state.is_document_dirty(&path),
        "the dirty draft is parked, not dropped"
    );
}

#[test]
fn close_project_with_a_dirty_document_asks_first_and_stays_open() {
    let (mut app, _t) = Signex::new();
    let dir = tempfile::tempdir().expect("tempdir");
    let prj = dir.path().join("Dirty.snxprj");
    crate::settle_support::settle(&mut app, Message::File(FileMsg::NewProject(Some(prj))));
    let project_path = app.document_state.projects[0].path.clone();
    app.document_state.mark_document_dirty(&project_path);

    crate::settle_support::settle(&mut app, Message::Menu(MenuMessage::CloseProject));

    assert!(app.ui_state.project_close_confirm.is_some(), "asks first");
    assert!(app.document_state.active_project.is_some(), "stays open");
}
