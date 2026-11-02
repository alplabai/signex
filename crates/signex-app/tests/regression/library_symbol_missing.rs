//! #1845 — deleting a library symbol that component rows and placed parts
//! use warns first, and a placed part whose library symbol is gone says so.

use crate::library_place_symbol::{app_with_library, doc, fixture, place};
use crate::library_symbol_drift::{check_drift, click_update, info, pin_count, select_first};
use signex_app::app::{
    ContextMenuMsg, Message, Signex, SymbolDocument, TabContextAction, TabInfo, TabKind,
};
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
use signex_library::{Symbol, SymbolFile};
use std::path::PathBuf;

fn container_path(f: &crate::library_place_symbol::Fixture) -> PathBuf {
    f.snxlib
        .parent()
        .unwrap()
        .join("symbols")
        .join("two-unit.snxsym")
}

/// Rewrite the library's container so it also holds a sibling symbol, and
/// open it in a symbol editor tab (the SCH Library panel's Delete target).
fn open_container_editor(app: &mut Signex, f: &crate::library_place_symbol::Fixture) -> PathBuf {
    let path = container_path(f);
    let mut file = SymbolFile::from_toml_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    file.symbols.push(Symbol::empty("SIBLING"));
    std::fs::write(&path, file.to_toml_string().unwrap()).unwrap();
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "two-unit".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    path
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn symbol_count(app: &Signex, path: &PathBuf) -> usize {
    app.document_state.symbol_editors[path].file.symbols.len()
}

#[test]
fn deleting_a_used_symbol_asks_first_and_names_the_row_and_sheet() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let reference = doc(&app).symbols[0].reference.clone();
    let path = open_container_editor(&mut app, &f);

    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));

    assert_eq!(
        symbol_count(&app, &path),
        2,
        "nothing deleted before the answer"
    );
    let ctx = app.document_state.panel_ctx.symbol_editor.as_ref().unwrap();
    let warning = ctx.delete_warning.as_ref().expect("a warning is shown");
    let text = warning.lines.join("\n");
    assert!(text.contains("Component rows"), "{text}");
    assert!(text.contains("place-364.snxsch"), "{text}");
    assert!(text.contains(&reference), "{text}");

    panel(&mut app, PanelMsg::SchLibraryDeleteConfirmed(0));
    assert_eq!(symbol_count(&app, &path), 1, "the user can still delete");
    assert!(
        app.document_state
            .panel_ctx
            .symbol_editor
            .as_ref()
            .unwrap()
            .delete_warning
            .is_none()
    );
}

#[test]
fn cancelling_the_warning_keeps_the_symbol() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let path = open_container_editor(&mut app, &f);

    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));
    panel(&mut app, PanelMsg::SchLibraryDeleteCancelled);

    assert_eq!(symbol_count(&app, &path), 2);
    assert!(
        app.document_state
            .panel_ctx
            .symbol_editor
            .as_ref()
            .unwrap()
            .delete_warning
            .is_none()
    );
}

#[test]
fn an_unused_symbol_is_deleted_at_once() {
    let f = fixture();
    let mut app = app_with_library(&f);
    let path = open_container_editor(&mut app, &f);

    // The sibling is bound by no row and placed nowhere.
    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(1));

    assert_eq!(symbol_count(&app, &path), 1);
}

#[test]
fn a_placed_part_whose_library_symbol_was_deleted_is_reported_missing() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let id = select_first(&mut app);
    let pins = pin_count(&app);

    // The library no longer holds the symbol.
    std::fs::write(
        container_path(&f),
        SymbolFile::from_symbol(Symbol::empty("OTHER"))
            .to_toml_string()
            .unwrap(),
    )
    .unwrap();
    check_drift(&mut app);

    assert_eq!(
        info(&app, "Library Status").as_deref(),
        Some("Symbol missing")
    );
    let sheet = app.document_state.active_path.clone().unwrap();
    let lib_id = doc(&app).symbols[0].lib_id.clone();
    assert!(app.library.drift.is_missing(&sheet, &lib_id));

    // The sheet keeps its embedded copy, and Update says why it does nothing.
    let _ = signex_app::diagnostics::init_logging();
    click_update(&mut app, id);
    assert!(
        logged(&[&label(&app), "is missing from its library"]) >= 1,
        "Update from library names the missing symbol"
    );
    assert_eq!(pin_count(&app), pins);
    assert_eq!(doc(&app).symbols.len(), 1);
}

fn label(app: &Signex) -> String {
    let lib_id = doc(app).symbols[0].lib_id.clone();
    app.library
        .drift
        .display_name(&lib_id, &app.library.open_libraries)
}

fn logged(needles: &[&str]) -> usize {
    signex_app::diagnostics::recent_entries()
        .iter()
        .filter(|e| needles.iter().all(|n| e.message.contains(n)))
        .count()
}

fn remove_symbol_from_library(f: &crate::library_place_symbol::Fixture) {
    std::fs::write(
        container_path(f),
        SymbolFile::from_symbol(Symbol::empty("OTHER"))
            .to_toml_string()
            .unwrap(),
    )
    .unwrap();
}

#[test]
fn repeated_passes_over_a_missing_symbol_log_it_once() {
    let _ = signex_app::diagnostics::init_logging();
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    remove_symbol_from_library(&f);
    let lib_id = doc(&app).symbols[0].lib_id.clone();

    check_drift(&mut app);
    check_drift(&mut app);
    check_drift(&mut app);

    assert_eq!(
        logged(&[&lib_id, "library symbol missing"]),
        1,
        "the removal is reported once per change"
    );
    assert_eq!(
        logged(&[&lib_id, "Could not check library symbol"]),
        0,
        "a removal is not a failed read"
    );
}

#[test]
fn a_symbol_that_comes_back_is_no_longer_missing() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    select_first(&mut app);
    let original = std::fs::read_to_string(container_path(&f)).unwrap();
    remove_symbol_from_library(&f);
    check_drift(&mut app);
    assert!(info(&app, "Library Status").is_some());

    std::fs::write(container_path(&f), original).unwrap();
    check_drift(&mut app);

    let sheet = app.document_state.active_path.clone().unwrap();
    let lib_id = doc(&app).symbols[0].lib_id.clone();
    assert!(!app.library.drift.is_missing(&sheet, &lib_id));
    assert!(info(&app, "Library Status").is_none());
}

#[test]
fn an_unreadable_lookup_keeps_the_symbol_missing() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    select_first(&mut app);
    remove_symbol_from_library(&f);
    check_drift(&mut app);
    assert!(info(&app, "Library Status").is_some());

    std::fs::write(container_path(&f), "not a symbol").unwrap();
    check_drift(&mut app);

    assert_eq!(
        info(&app, "Library Status").as_deref(),
        Some("Symbol missing"),
        "a failed read says nothing about the removal"
    );
}

fn warning_shown(app: &Signex) -> bool {
    app.document_state
        .panel_ctx
        .symbol_editor
        .as_ref()
        .is_some_and(|c| c.delete_warning.is_some())
}

#[test]
fn an_edit_after_the_warning_makes_it_stale_and_confirm_says_so() {
    let _ = signex_app::diagnostics::init_logging();
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let path = open_container_editor(&mut app, &f);
    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));
    assert!(warning_shown(&app));

    // The container's history moves under the warning.
    let editor = app.document_state.symbol_editors.get_mut(&path).unwrap();
    let snapshot = editor.snapshot();
    editor.commit_history(snapshot);
    panel(&mut app, PanelMsg::SchLibrarySelectSymbol(1));
    assert!(!warning_shown(&app), "a stale warning is not shown");

    panel(&mut app, PanelMsg::SchLibraryDeleteConfirmed(0));
    assert_eq!(
        symbol_count(&app, &path),
        2,
        "a stale warning deletes nothing"
    );
    assert!(
        logged(&["TWO-UNIT", "changed since the warning; press Delete again"]) >= 1,
        "the refusal reaches the Messages panel"
    );
}

#[test]
fn closing_the_tab_or_the_library_drops_the_warning() {
    let f = fixture();
    let mut app = app_with_library(&f);
    place(&mut app, &f);
    let path = open_container_editor(&mut app, &f);
    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));
    assert!(app.library.pending_symbol_delete.is_some());

    let tab = app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == path)
        .unwrap();
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::Close(tab),
    )));
    assert!(app.library.pending_symbol_delete.is_none(), "tab closed");

    open_container_editor(&mut app, &f);
    panel(&mut app, PanelMsg::SchLibraryDeleteSymbol(0));
    assert!(app.library.pending_symbol_delete.is_some());
    app.library.close_library(&f.snxlib);
    assert!(
        app.library.pending_symbol_delete.is_none(),
        "library closed"
    );
}
