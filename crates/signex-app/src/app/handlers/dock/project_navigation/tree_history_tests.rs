//! Show History on a Projects tree leaf (mockup row 20): the History panel is
//! shown filtered only once it follows the clicked file.

use std::path::Path;

use signex_widgets::tree_view::{TreeKey, key_of};

use super::*;
use crate::app::ProjectTreeAction;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::handlers::menu::export::write::settle as run_to_end;
use crate::app::runtime::sheet_tree_tests::{settle, write_sheet};
use crate::panels::PanelKind;

/// A project in `tmp` listing `listed`, its tree built, History closed.
fn project_app(tmp: &Path, listed: &[&str]) -> Signex {
    let dir = tmp.to_string_lossy().into_owned();
    let mut app = app_workspace(&dir, listed);
    settle(&mut app, false);
    let _ = app.document_state.dock.close_panel(PanelKind::History);
    assert!(!app.is_panel_open(PanelKind::History));
    app
}

/// The first Source Documents row.
fn first_document(app: &Signex) -> TreeKey {
    key_of(&app.document_state.panel_ctx.project_tree, &[0, 0, 0]).expect("a document row")
}

fn show_history(app: &mut Signex, key: TreeKey) -> Task<Message> {
    app.handle_project_tree_action(ProjectTreeAction::ShowHistory(key))
}

fn history_untouched(app: &Signex) {
    assert!(!app.is_panel_open(PanelKind::History));
    assert!(!app.document_state.history.ui.filter.this_file_only);
    assert!(app.ui_state.pending_tree_history.is_none());
}

#[test]
fn show_history_on_a_closed_sheet_waits_for_its_read() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_sheet(tmp.path(), "top.snxsch", &[]);
    let mut app = project_app(tmp.path(), &["top.snxsch"]);
    let top = app.resolve_held_spelling(tmp.path().join("top.snxsch"));
    let key = first_document(&app);

    let opening = show_history(&mut app, key);

    assert!(opening.units() > 0, "the sheet is read on a worker");
    let waiting = app.ui_state.pending_tree_history.clone();
    assert!(!app.is_panel_open(PanelKind::History), "not yet");
    assert!(!app.document_state.history.ui.filter.this_file_only);
    assert_eq!(waiting, Some(top.clone()));
    run_to_end(&mut app, opening);
    assert_eq!(app.history_target_path(), Some(top));
    assert!(app.is_panel_open(PanelKind::History));
    assert!(app.document_state.history.ui.filter.this_file_only);
    let shown = &app.document_state.panel_ctx.history;
    assert!(shown.ui.filter.this_file_only, "the panel reads it");
    assert!(app.ui_state.pending_tree_history.is_none());
}

#[test]
fn show_history_on_a_file_that_fails_to_open_leaves_the_panel_as_it_was() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut app = project_app(tmp.path(), &["gone.snxsch"]);
    let key = first_document(&app);
    let tree = app.document_state.panel_ctx.project_tree.as_slice();
    let row = signex_widgets::tree_view::get_node(tree, &[0, 0, 0]).expect("the row");
    assert!(row.label.starts_with("gone.snxsch"));

    let opening = show_history(&mut app, key);
    run_to_end(&mut app, opening);

    assert!(app.document_state.tabs.is_empty(), "nothing opened");
    history_untouched(&app);
}

#[test]
fn show_history_on_a_missing_child_sheet_only_offers_to_create_it() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_sheet(tmp.path(), "top.snxsch", &["ghost.snxsch"]);
    let mut app = project_app(tmp.path(), &["top.snxsch"]);
    assert!(app.open_or_focus_sheet_path(tmp.path().join("top.snxsch")));
    settle(&mut app, false);
    let key = vec![
        app.document_state.projects[0].id.to_string(),
        "Source Documents".to_string(),
        "top.snxsch".to_string(),
        "ghost.snxsch".to_string(),
    ];

    let offered = show_history(&mut app, key);

    assert_eq!(offered.units(), 0, "no read of a file that is not there");
    assert!(app.ui_state.create_child_sheet_confirm.is_some());
    history_untouched(&app);
}
