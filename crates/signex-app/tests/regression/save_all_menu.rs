//! #1765 — File ▸ Save All writes every dirty document through the shared
//! Save All flow and then does nothing else: no close, no quit.

use signex_app::app::{FileMsg, Message, Signex, TabInfo, TabKind};
use signex_app::diagnostics;
use signex_app::menu_bar::MenuMessage;
use tempfile::TempDir;

use crate::primitive_dirty_quit::{edit_description_via_panel, open_footprint_editor};

fn logged(needle: &str) -> bool {
    diagnostics::recent_entries()
        .iter()
        .any(|entry| entry.message.contains(needle))
}

#[test]
fn save_all_writes_every_dirty_document_and_closes_nothing() {
    let _ = diagnostics::init_logging();
    let tmp = TempDir::new().unwrap();
    let (a, b) = (tmp.path().join("a.snxfpt"), tmp.path().join("b.snxfpt"));
    let (mut app, _t) = Signex::new();
    for path in [&a, &b] {
        open_footprint_editor(&mut app, path);
        edit_description_via_panel(&mut app);
    }
    assert!(app.document_state.has_dirty_documents());

    crate::settle_support::settle(&mut app, Message::Menu(MenuMessage::SaveAll));

    for path in [&a, &b] {
        let written = std::fs::read_to_string(path).expect("Save All must write the file");
        assert!(written.contains("edited"), "{}", path.display());
    }
    assert!(!app.document_state.has_dirty_documents());
    assert_eq!(app.document_state.tabs.len(), 2, "no tab is closed");
    assert!(
        app.ui_state.app_quit_confirm.is_none(),
        "no quit is started"
    );
    assert!(logged("Save All: saved 2 item(s)"), "the outcome is logged");
}

#[test]
fn save_all_with_nothing_dirty_writes_nothing() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("clean.snxfpt");
    let (mut app, _t) = Signex::new();
    open_footprint_editor(&mut app, &path);

    crate::settle_support::settle(&mut app, Message::Menu(MenuMessage::SaveAll));

    assert!(!path.exists());
    assert_eq!(app.document_state.tabs.len(), 1);
}

#[test]
fn a_failed_write_is_reported_and_leaves_the_document_dirty() {
    let _ = diagnostics::init_logging();
    let tmp = TempDir::new().unwrap();
    let blocker = tmp.path().join("not-a-dir");
    std::fs::write(&blocker, b"x").unwrap();
    let path = blocker.join("unwritable.snxfpt");
    let (mut app, _t) = Signex::new();
    open_footprint_editor(&mut app, &path);
    edit_description_via_panel(&mut app);

    crate::settle_support::settle(&mut app, Message::Menu(MenuMessage::SaveAll));

    assert!(app.document_state.is_document_dirty(&path));
    assert!(logged("Save All: could not save"), "the failure is logged");
    assert!(
        app.document_state.error_notice.is_some(),
        "the notice shows"
    );
}

#[test]
fn save_all_also_writes_a_dirty_schematic_and_project() {
    let tmp = TempDir::new().unwrap();
    let (mut app, _t) = Signex::new();
    let prj = tmp.path().join("All.snxprj");
    crate::settle_support::settle(&mut app, Message::File(FileMsg::NewProject(Some(prj))));
    let sch = tmp.path().join("one.snxsch");
    let sheet = signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let mut engine = signex_engine::Engine::new(sheet).unwrap();
    engine.set_path(Some(sch.clone()));
    app.document_state.engines.insert(sch.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "one".into(),
        path: sch.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    let prj = app.document_state.projects[0].path.clone();
    app.document_state.mark_document_dirty(&sch);
    app.document_state.mark_document_dirty(&prj);

    crate::settle_support::settle(&mut app, Message::Menu(MenuMessage::SaveAll));

    assert!(sch.exists(), "the schematic is written");
    assert!(!app.document_state.is_document_dirty(&sch));
    assert!(!app.document_state.is_document_dirty(&prj));
}
