//! Removing a sheet that sheet symbols place names those symbols before the
//! delete and in Messages after it (#1837).

use signex_widgets::tree_view::TreeKey;

use crate::app::handlers::menu::export::tests::{app_workspace, sheet_with_net};
use crate::app::runtime::sheet_tree_tests::{settle, write_sheet};
use crate::app::{Message, ProjectTreeAction, RemoveChoice, RemoveMsg};

fn child_key(app: &super::Signex) -> TreeKey {
    vec![
        app.document_state.projects[0].id.to_string(),
        "Source Documents".to_string(),
        "top.snxsch".to_string(),
        "child.snxsch".to_string(),
    ]
}

/// `top.snxsch` placing `child.snxsch` twice, as "Driver A" and "Driver B".
fn top_with_two_symbols() -> signex_types::schematic::SchematicSheet {
    let mut top = sheet_with_net("R_TOP", "N", &["child.snxsch", "child.snxsch"]);
    for (symbol, name) in top.child_sheets.iter_mut().zip(["Driver A", "Driver B"]) {
        symbol.name = name.to_string();
        symbol.uuid = uuid::Uuid::now_v7();
    }
    top
}

fn write_top_with_two_symbols(dir: &std::path::Path) -> signex_types::schematic::SchematicSheet {
    let top = top_with_two_symbols();
    let text = signex_types::format::SnxSchematic::new(top.clone())
        .write_string()
        .expect("encode");
    std::fs::write(dir.join("top.snxsch"), text).expect("write top");
    top
}

#[test]
fn the_dialog_names_the_symbols_and_messages_names_the_parents_left_dangling() {
    let _ = crate::diagnostics::init_logging();
    let tmp = tempfile::tempdir().expect("tempdir");
    let _ = write_top_with_two_symbols(tmp.path());
    write_sheet(tmp.path(), "child.snxsch", &[]);
    let dir = tmp.path().to_string_lossy().into_owned();
    let mut app = app_workspace(&dir, &["top.snxsch", "child.snxsch"]);
    settle(&mut app, false);

    let key = child_key(&app);
    let opened = app.handle_project_tree_action(ProjectTreeAction::OpenRemoveDialog(key));
    assert_eq!(opened.units(), 0);
    let dialog = app.ui_state.remove_dialog.clone().expect("dialog opens");
    let blockers = app.remove_blockers(&dialog);
    assert!(!blockers.blocks_delete(false), "the row's own parent only");
    assert_eq!(blockers.symbols.len(), 2, "{:?}", blockers.symbols);
    assert!(
        blockers
            .symbols
            .iter()
            .all(|s| s.parent == tmp.path().join("top.snxsch")),
        "{:?}",
        blockers.symbols
    );
    let names: Vec<&str> = blockers.symbols.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Driver A", "Driver B"]);

    crate::app::path_ops::settle(
        &mut app,
        Message::Remove(RemoveMsg::Confirm(RemoveChoice::DeleteFile)),
    );

    assert!(!tmp.path().join("child.snxsch").exists());
    let said = app
        .document_state
        .panel_ctx
        .diagnostics
        .iter()
        .find(|e| e.message.contains("still point at it"))
        .expect("Messages names the dangling symbols");
    assert!(
        said.message.contains("2 sheet symbol(s)"),
        "{}",
        said.message
    );
    assert!(
        said.message.contains("\"Driver A\" on top.snxsch")
            && said.message.contains("\"Driver B\" on top.snxsch"),
        "{}",
        said.message
    );
}

#[test]
fn deleting_a_sheet_nothing_places_adds_no_dangling_warning() {
    let _ = crate::diagnostics::init_logging();
    let tmp = tempfile::tempdir().expect("tempdir");
    write_sheet(tmp.path(), "top.snxsch", &[]);
    write_sheet(tmp.path(), "loose.snxsch", &[]);
    let dir = tmp.path().to_string_lossy().into_owned();
    let mut app = app_workspace(&dir, &["top.snxsch", "loose.snxsch"]);
    settle(&mut app, false);
    let key: TreeKey = vec![
        app.document_state.projects[0].id.to_string(),
        "Source Documents".to_string(),
        "loose.snxsch".to_string(),
    ];
    let _ = app.handle_project_tree_action(ProjectTreeAction::OpenRemoveDialog(key));
    let dialog = app.ui_state.remove_dialog.clone().expect("dialog opens");
    assert!(app.remove_blockers(&dialog).symbols.is_empty());
}

#[test]
fn a_symbol_seen_by_the_walk_and_the_open_document_is_listed_once() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let top = write_top_with_two_symbols(tmp.path());
    write_sheet(tmp.path(), "child.snxsch", &[]);
    let dir = tmp.path().to_string_lossy().into_owned();
    let mut app = app_workspace(&dir, &["top.snxsch", "child.snxsch"]);
    // The same two symbols, now also read from the open document.
    crate::app::handlers::menu::export::tests::open_with(
        &mut app.document_state,
        &tmp.path().join("top.snxsch"),
        top,
    );
    settle(&mut app, false);

    let _ = app.handle_project_tree_action(ProjectTreeAction::OpenRemoveDialog(child_key(&app)));
    let dialog = app.ui_state.remove_dialog.clone().expect("dialog opens");
    assert_eq!(app.remove_blockers(&dialog).symbols.len(), 2);
}
