//! #1765 — File ▸ Save All writes every dirty document through the shared
//! Save All flow and then does nothing else: no close, no quit.

use signex_app::app::{Message, Signex};
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
