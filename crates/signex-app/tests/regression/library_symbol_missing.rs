//! #1845 — deleting a library symbol that component rows and placed parts
//! use warns first, and a placed part whose library symbol is gone says so.

use crate::library_place_symbol::{app_with_library, doc, fixture, place};
use crate::library_symbol_drift::{check_drift, click_update, pin_count, select_first};
use signex_app::app::{Message, Signex, SymbolDocument, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
use signex_library::{Symbol, SymbolFile};
use std::path::PathBuf;

fn info(app: &Signex, key: &str) -> Option<String> {
    app.document_state
        .panel_ctx
        .selection_info
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
}

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

    // The sheet keeps its embedded copy, and Update has nothing to apply.
    click_update(&mut app, id);
    assert_eq!(pin_count(&app), pins);
    assert_eq!(doc(&app).symbols.len(), 1);
}
